import { serverApi } from '@/lib/api/server';
import { CONTENT_LIMIT_CONSTANTS } from '../constants';

const { DEFAULTS } = CONTENT_LIMIT_CONSTANTS;

export interface ContentLimitResult {
  allowed: boolean;
  current: number;
  limit: number;
  remaining: number;
  resource_type: string;
  per_subject?: Array<{
    subject_id: string;
    subject_name: string;
    current: number;
  }>;
}

type ContentLimitApiValue = Partial<ContentLimitResult> & {
  daily_limit?: number;
};

type ContentLimitsPayload =
  | ContentLimitApiValue[]
  | Record<string, ContentLimitApiValue>;

interface UsageResponse {
  content_limits?: ContentLimitsPayload;
  daily_quotas?: Record<string, unknown>;
}

function fallback(resourceType: string): ContentLimitResult {
  const limit = DEFAULTS[resourceType] ?? 0;
  return {
    allowed: true,
    current: 0,
    limit,
    remaining: limit,
    resource_type: resourceType,
  };
}

function normalizeContentLimit(
  resourceType: string,
  value: ContentLimitApiValue
): ContentLimitResult {
  const actualResourceType = value.resource_type ?? resourceType;
  const limit =
    value.limit ?? value.daily_limit ?? DEFAULTS[actualResourceType] ?? 0;
  return {
    resource_type: actualResourceType,
    allowed: value.allowed ?? true,
    current: value.current ?? 0,
    limit,
    remaining: value.remaining ?? 0,
    per_subject: value.per_subject,
  };
}

function normalizeContentLimits(
  payload: ContentLimitsPayload | null | undefined
): ContentLimitResult[] {
  if (!payload) return [];
  if (Array.isArray(payload)) {
    return payload.map((value, index) =>
      normalizeContentLimit(value.resource_type ?? String(index), value)
    );
  }
  return Object.entries(payload).map(([resourceType, value]) =>
    normalizeContentLimit(resourceType, value)
  );
}

export async function getContentLimit(
  _userId: string,
  resourceType: string
): Promise<number> {
  return fallback(resourceType).limit;
}

export async function checkContentLimit(
  _userId: string,
  resourceType: string,
  context?: { subjectId?: string }
): Promise<ContentLimitResult> {
  const params = new URLSearchParams();
  if (context?.subjectId) params.set('subject_id', context.subjectId);
  return serverApi<ContentLimitResult>(
    `/api/usage/${resourceType}${params.size ? `?${params}` : ''}`
  ).catch(() => fallback(resourceType));
}

export async function getAllContentLimits(
  userId: string
): Promise<ContentLimitResult[]> {
  const adminData = await serverApi<ContentLimitsPayload>(
    `/api/admin/users/${encodeURIComponent(userId)}/content-limits`
  ).catch(() => null);
  const adminLimits = normalizeContentLimits(adminData);
  if (adminLimits.length > 0) return adminLimits;

  const usage = await serverApi<UsageResponse>('/api/usage').catch(() => null);
  return normalizeContentLimits(usage?.content_limits);
}

export async function setContentLimitOverride(): Promise<void> {
  return;
}
