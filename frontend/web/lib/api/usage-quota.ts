import { serverApi } from '@/lib/api/server';
import { USAGE_QUOTA_CONSTANTS } from '../constants';

export interface QuotaCheckResult {
  allowed: boolean;
  current_usage: number;
  daily_limit: number;
  remaining: number;
}

type QuotaUsageResponse = Partial<Record<string, QuotaCheckResult>>;

function emptyQuota(): QuotaCheckResult {
  return {
    allowed: true,
    current_usage: 0,
    daily_limit: USAGE_QUOTA_CONSTANTS.DEFAULTS.AI_EXTRACTION_DAILY_LIMIT ?? 0,
    remaining: USAGE_QUOTA_CONSTANTS.DEFAULTS.AI_EXTRACTION_DAILY_LIMIT ?? 0,
  };
}

export async function getUserQuotaLimit(
  _userId?: string,
  _resourceType?: string
): Promise<number> {
  void _userId;
  void _resourceType;
  return emptyQuota().daily_limit;
}

export async function checkAndIncrementQuota(
  _userId?: string,
  _resourceType?: string,
  _userTimezone?: string
): Promise<QuotaCheckResult> {
  void _userId;
  void _resourceType;
  void _userTimezone;
  return emptyQuota();
}

export async function getQuotaUsage(
  userId: string,
  resourceType: string = USAGE_QUOTA_CONSTANTS.RESOURCE_TYPES.AI_EXTRACTION
): Promise<QuotaCheckResult> {
  const data = await serverApi<QuotaUsageResponse>(
    `/api/admin/users/${encodeURIComponent(userId)}/quota`
  ).catch(() => null);
  return data?.[resourceType] ?? data?.ai_extraction ?? emptyQuota();
}
