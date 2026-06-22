'use client';

import { useState, useCallback, useEffect, useMemo } from 'react';
import ProblemForm from './problem-form';
import EnhancedProblemsTable from './enhanced-problems-table';
import StatsStrip from './stats-strip';
import EmptyState from './empty-state';
import ProblemFab from './problem-fab';
import { ProblemsPageClientProps, Problem, Tag } from '@/lib/types';
import { useOnboarding } from '@/components/onboarding/onboarding-provider';
import { ConfirmationDialog } from '@/components/ui/confirmation-dialog';
import { useContentLimit } from '@/lib/hooks/useContentLimit';
import { CONTENT_LIMIT_CONSTANTS } from '@/lib/constants';
import { toast } from 'sonner';
import { useTranslations } from 'next-intl';
import { clientApi } from '@/lib/api/client';
import { useQueryClient } from '@tanstack/react-query';
import { useProblems } from './use-problems';
import { queryKeys } from '@/lib/api/query-keys';

type FormMode = 'closed' | 'create-manual' | 'create-scan' | 'edit';

export default function ProblemsPageClient({
  initialProblems,
  subjectId,
  availableTags,
}: ProblemsPageClientProps) {
  const t = useTranslations('Problems');
  const tCommon = useTranslations('Common');
  const queryClient = useQueryClient();
  const { data: problemsData } = useProblems(subjectId, {
    initialData: initialProblems,
  });
  const problems = useMemo(() => problemsData ?? [], [problemsData]);
  const tagsByProblem = useMemo(() => {
    const map: Record<string, Tag[]> = {};
    for (const problem of problems) {
      map[problem.id] = problem.tags || [];
    }
    return map;
  }, [problems]);
  const { refreshChecklistStatus } = useOnboarding();

  const [formMode, setFormMode] = useState<FormMode>('closed');
  const [editingProblem, setEditingProblem] = useState<Problem | null>(null);
  const [hasActiveFilters, setHasActiveFilters] = useState(false);

  // Content limit checks
  const {
    data: problemLimit,
    isExhausted: problemLimitExhausted,
    refresh: refreshProblemLimit,
  } = useContentLimit(
    CONTENT_LIMIT_CONSTANTS.RESOURCE_TYPES.PROBLEMS_PER_SUBJECT,
    subjectId
  );

  // Confirmation dialog for switching from create to edit
  const [switchDialog, setSwitchDialog] = useState<{
    open: boolean;
    pendingProblem: Problem | null;
  }>({ open: false, pendingProblem: null });

  const handleAddManually = useCallback(() => {
    if (problemLimitExhausted) {
      toast.error(
        `You've reached the maximum of ${problemLimit?.limit ?? 300} problems in this notebook.`
      );
      return;
    }
    setFormMode('create-manual');
    setEditingProblem(null);
    window.scrollTo({ top: 0, behavior: 'smooth' });
  }, [problemLimitExhausted, problemLimit?.limit]);

  const handleAddScan = useCallback(() => {
    if (problemLimitExhausted) {
      toast.error(
        `You've reached the maximum of ${problemLimit?.limit ?? 300} problems in this notebook.`
      );
      return;
    }
    setFormMode('create-scan');
    setEditingProblem(null);
    window.scrollTo({ top: 0, behavior: 'smooth' });
  }, [problemLimitExhausted, problemLimit?.limit]);

  const fetchAndOpenEdit = useCallback(async (problem: Problem) => {
    try {
      setEditingProblem(
        await clientApi<Problem>(`/api/problems/${problem.id}`)
      );
    } catch (err) {
      toast.error(
        err instanceof Error ? err.message : 'Failed to load problem details'
      );
      setEditingProblem(problem);
    }
    setFormMode('edit');
    window.scrollTo({ top: 0, behavior: 'smooth' });
  }, []);

  const handleEditProblem = useCallback(
    async (problem: Problem) => {
      if (formMode === 'create-manual' || formMode === 'create-scan') {
        setSwitchDialog({ open: true, pendingProblem: problem });
        return;
      }
      fetchAndOpenEdit(problem);
    },
    [formMode, fetchAndOpenEdit]
  );

  const handleConfirmSwitch = useCallback(async () => {
    const problem = switchDialog.pendingProblem;
    setSwitchDialog({ open: false, pendingProblem: null });
    if (!problem) return;
    fetchAndOpenEdit(problem);
  }, [switchDialog.pendingProblem, fetchAndOpenEdit]);

  const handleCloseForm = useCallback(() => {
    setFormMode('closed');
    setEditingProblem(null);
  }, []);

  const handleProblemCreated = useCallback(() => {
    // 列表数据由 useProblems 维护；create mutation 的 onSuccess 已 invalidate 重取。
    setFormMode('closed');
    setEditingProblem(null);
    refreshChecklistStatus();
    refreshProblemLimit();
  }, [refreshChecklistStatus, refreshProblemLimit]);

  const handleProblemDeleted = useCallback(() => {
    // enhanced-problems-table 已乐观隐藏并调 DELETE API；这里让缓存重取同步。
    queryClient.invalidateQueries({
      queryKey: queryKeys.problems(subjectId),
    });
  }, [queryClient, subjectId]);

  const handleProblemUpdated = useCallback(() => {
    // 列表数据由 useProblems 维护；update mutation 的 onSuccess 已 invalidate 重取。
    setFormMode('closed');
    setEditingProblem(null);
  }, []);

  const handleFiltersActiveChange = useCallback((active: boolean) => {
    setHasActiveFilters(active);
  }, []);

  const showEmptyState = problems.length === 0 && !hasActiveFilters;

  // Keyboard shortcuts: Enter = write, Shift+Enter = scan
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (formMode !== 'closed') return;

      const target = e.target as HTMLElement;
      const tag = target?.tagName?.toLowerCase();
      if (
        tag === 'input' ||
        tag === 'textarea' ||
        tag === 'select' ||
        target?.getAttribute('contenteditable') === 'true' ||
        target?.closest('[role="dialog"]')
      )
        return;

      if (e.key === 'Enter' && !e.metaKey && !e.ctrlKey && !e.altKey) {
        e.preventDefault();
        if (e.shiftKey) {
          handleAddScan();
        } else {
          handleAddManually();
        }
      }
    };
    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, [formMode, handleAddManually, handleAddScan]);

  return (
    <>
      {/* Stats strip */}
      <StatsStrip problems={problems} />

      {/* Form slot */}
      {formMode !== 'closed' && (
        <div className="form-slot-enter rounded-lg border bg-card p-4 md:p-6">
          <ProblemForm
            key={editingProblem?.id ?? formMode}
            subjectId={subjectId}
            availableTags={availableTags}
            problem={editingProblem}
            alwaysExpanded
            initialShowImageScan={formMode === 'create-scan'}
            onCancel={handleCloseForm}
            onProblemCreated={handleProblemCreated}
            onProblemUpdated={handleProblemUpdated}
          />
        </div>
      )}

      {/* Empty state or table */}
      {showEmptyState ? (
        <EmptyState
          onAddManually={handleAddManually}
          onAddScan={handleAddScan}
        />
      ) : (
        <EnhancedProblemsTable
          initialProblems={problems}
          initialTagsByProblem={tagsByProblem}
          subjectId={subjectId}
          availableTags={availableTags}
          onProblemDeleted={handleProblemDeleted}
          onEditProblem={handleEditProblem}
          onFiltersActiveChange={handleFiltersActiveChange}
        />
      )}

      {/* FAB */}
      <ProblemFab
        hidden={formMode !== 'closed'}
        onAddManually={handleAddManually}
        onAddScan={handleAddScan}
      />

      {/* Switch from create to edit confirmation */}
      <ConfirmationDialog
        isOpen={switchDialog.open}
        title={t('discardUnsavedTitle')}
        message={t('discardUnsavedDesc')}
        confirmText={tCommon('discardAndEdit')}
        cancelText={tCommon('keepEditing')}
        onConfirm={handleConfirmSwitch}
        onCancel={() => setSwitchDialog({ open: false, pendingProblem: null })}
        variant="destructive"
      />
    </>
  );
}
