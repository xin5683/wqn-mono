'use client';

import { useEffect, useState } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { useTranslations } from 'next-intl';
import { toast } from 'sonner';
import { clientApi } from '@/lib/api/client';
import { queryKeys } from '@/lib/api/query-keys';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Gauge, Save, RotateCcw, Zap, HardDrive } from 'lucide-react';
import { formatBytes } from '@/lib/utils/format';

interface LimitDefaultEntry {
  resource_type: string;
  category: 'daily_quota' | 'content_limit';
  hardcoded: number;
  configured: number | null;
  effective: number;
}

interface LimitDefaultsResponse {
  defaults: LimitDefaultEntry[];
}

// Display order: daily quotas first, then content limits.
const RESOURCE_ORDER = [
  'ai_extraction',
  'ai_categorisation',
  'subjects',
  'problems_per_subject',
  'problem_sets',
  'tags_per_subject',
  'storage_bytes',
];

function isStorage(resourceType: string) {
  return resourceType === 'storage_bytes';
}

function formatValue(resourceType: string, n: number): string {
  return isStorage(resourceType) ? formatBytes(n) : String(n);
}

export function LimitDefaultsSection() {
  const t = useTranslations('Admin');
  const tUsage = useTranslations('Usage');
  const queryClient = useQueryClient();
  const [inputs, setInputs] = useState<Record<string, string>>({});
  const [saving, setSaving] = useState(false);

  const { data, isLoading } = useQuery({
    queryKey: queryKeys.adminLimitDefaults(),
    queryFn: () =>
      clientApi<LimitDefaultsResponse>('/api/admin/limit-defaults'),
    staleTime: 0,
  });

  // Seed the local inputs whenever fresh server data arrives.
  useEffect(() => {
    if (!data) return;
    const seeded: Record<string, string> = {};
    for (const entry of data.defaults) {
      seeded[entry.resource_type] =
        entry.configured === null ? '' : String(entry.configured);
    }
    setInputs(seeded);
  }, [data]);

  const entries = (data?.defaults ?? []).slice().sort((a, b) => {
    return (
      RESOURCE_ORDER.indexOf(a.resource_type) -
      RESOURCE_ORDER.indexOf(b.resource_type)
    );
  });

  const dailyQuotas = entries.filter(e => e.category === 'daily_quota');
  const contentLimits = entries.filter(e => e.category === 'content_limit');

  const labelFor = (resourceType: string): string => {
    switch (resourceType) {
      case 'ai_extraction':
        return tUsage('aiExtraction');
      case 'ai_categorisation':
        return tUsage('aiCategorisation');
      case 'subjects':
        return tUsage('limits.subjects');
      case 'problems_per_subject':
        return tUsage('limits.problems_per_subject');
      case 'problem_sets':
        return tUsage('limits.problem_sets');
      case 'tags_per_subject':
        return tUsage('limits.tags_per_subject');
      case 'storage_bytes':
        return tUsage('limits.storage_bytes');
      default:
        return resourceType;
    }
  };

  const isModified = (resourceType: string): boolean => {
    const entry = data?.defaults.find(e => e.resource_type === resourceType);
    if (!entry) return false;
    const inputVal = inputs[resourceType] ?? '';
    const serverVal = entry.configured === null ? '' : String(entry.configured);
    return inputVal !== serverVal;
  };

  const hasModifications = entries.some(e => isModified(e.resource_type));

  const handleSave = async () => {
    setSaving(true);
    try {
      const payload: Record<string, number | null> = {};
      for (const resourceType of RESOURCE_ORDER) {
        const raw = (inputs[resourceType] ?? '').trim();
        payload[resourceType] = raw === '' ? null : Number(raw);
      }
      await clientApi('/api/admin/limit-defaults', {
        method: 'PUT',
        body: payload,
      });
      await queryClient.invalidateQueries({
        queryKey: queryKeys.adminLimitDefaults(),
      });
      // Usage views depend on these defaults too.
      await queryClient.invalidateQueries({ queryKey: queryKeys.usage() });
      toast.success(t('limitDefaultsSaved'));
    } catch {
      toast.error(t('limitDefaultsSaveError'));
    } finally {
      setSaving(false);
    }
  };

  const handleReset = () => {
    if (!data) return;
    const seeded: Record<string, string> = {};
    for (const entry of data.defaults) {
      seeded[entry.resource_type] =
        entry.configured === null ? '' : String(entry.configured);
    }
    setInputs(seeded);
  };

  const renderRow = (entry: LimitDefaultEntry) => {
    const resourceType = entry.resource_type;
    const modified = isModified(resourceType);
    const inputVal = inputs[resourceType] ?? '';
    // Effective = the edited value if modified, else the server-reported effective.
    const effectiveNum = modified
      ? inputVal.trim() === ''
        ? entry.hardcoded
        : Number(inputVal)
      : entry.effective;

    return (
      <div key={resourceType}>
        <div className="flex items-baseline justify-between mb-1">
          <span className="text-sm font-medium text-gray-700 dark:text-gray-300">
            {labelFor(resourceType)}
          </span>
          <span className="text-xs text-gray-500 dark:text-gray-400">
            {t('effectiveLabel')}: {formatValue(resourceType, effectiveNum)}
            {entry.configured !== null && !modified && (
              <span className="ml-1 text-gray-400 dark:text-gray-500">
                ({t('baseline')}: {formatValue(resourceType, entry.hardcoded)})
              </span>
            )}
          </span>
        </div>
        <div className="flex gap-2">
          <Input
            type="number"
            min="0"
            placeholder={`${t('baseline')}: ${formatValue(resourceType, entry.hardcoded)}`}
            value={inputVal}
            onChange={e =>
              setInputs(prev => ({ ...prev, [resourceType]: e.target.value }))
            }
            className="h-8 text-xs rounded-lg"
          />
          {modified && (
            <Button
              size="sm"
              variant="ghost"
              className="h-8 text-xs rounded-lg"
              onClick={() =>
                setInputs(prev => ({
                  ...prev,
                  [resourceType]:
                    entry.configured === null ? '' : String(entry.configured),
                }))
              }
            >
              <RotateCcw className="h-3 w-3" />
            </Button>
          )}
        </div>
      </div>
    );
  };

  return (
    <div className="admin-section-card">
      <div className="flex items-center gap-2 mb-2">
        <Gauge className="h-4 w-4 text-amber-600 dark:text-amber-400" />
        <h3 className="font-semibold text-gray-900 dark:text-white">
          {t('limitDefaultsTitle')}
        </h3>
      </div>
      <p className="text-sm text-gray-500 dark:text-gray-400 mb-4">
        {t('limitDefaultsDesc')}
      </p>

      {isLoading || !data ? (
        <p className="text-sm text-gray-500 dark:text-gray-400">
          {t('noQuotaData')}
        </p>
      ) : (
        <div className="space-y-6">
          <div className="space-y-4">
            <div className="flex items-center gap-2 text-xs font-semibold uppercase tracking-wide text-gray-500 dark:text-gray-400">
              <Zap className="h-3.5 w-3.5" />
              {tUsage('dailyQuotas')}
            </div>
            <div className="space-y-4 pl-5">{dailyQuotas.map(renderRow)}</div>
          </div>

          <div className="space-y-4">
            <div className="flex items-center gap-2 text-xs font-semibold uppercase tracking-wide text-gray-500 dark:text-gray-400">
              <HardDrive className="h-3.5 w-3.5" />
              {tUsage('contentLimits')}
            </div>
            <div className="space-y-4 pl-5">{contentLimits.map(renderRow)}</div>
          </div>
        </div>
      )}

      {hasModifications && (
        <div className="flex justify-end gap-2 pt-4 mt-4 border-t border-amber-200/30 dark:border-stone-800/50">
          <Button
            variant="outline"
            className="rounded-xl gap-2"
            onClick={handleReset}
            disabled={saving}
          >
            <RotateCcw className="h-4 w-4" />
            {t('resetChanges')}
          </Button>
          <Button
            className="rounded-xl gap-2"
            onClick={handleSave}
            disabled={saving}
          >
            <Save className="h-4 w-4" />
            {saving ? t('saving') : t('saveChanges')}
          </Button>
        </div>
      )}
    </div>
  );
}
