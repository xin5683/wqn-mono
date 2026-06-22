import { describe, it, expect, vi, beforeEach } from 'vitest';
import { USAGE_QUOTA_CONSTANTS } from '../constants';
import {
  checkAndIncrementQuota,
  getQuotaUsage,
  getUserQuotaLimit,
} from '../api/usage-quota';

const mockServerApi = vi.hoisted(() => vi.fn());

vi.mock('@/lib/api/server', () => ({
  serverApi: mockServerApi,
}));

const defaultLimit =
  USAGE_QUOTA_CONSTANTS.DEFAULTS.AI_EXTRACTION_DAILY_LIMIT ?? 0;

beforeEach(() => {
  vi.clearAllMocks();
});

// ---------------------------------------------------------------------------
// checkAndIncrementQuota
// ---------------------------------------------------------------------------

describe('checkAndIncrementQuota', () => {
  it('returns the local fallback quota result', async () => {
    const result = await checkAndIncrementQuota('user-1');

    expect(result).toEqual({
      allowed: true,
      current_usage: 0,
      daily_limit: defaultLimit,
      remaining: defaultLimit,
    });
    expect(mockServerApi).not.toHaveBeenCalled();
  });
});

// ---------------------------------------------------------------------------
// getUserQuotaLimit
// ---------------------------------------------------------------------------

describe('getUserQuotaLimit', () => {
  it('returns the local default quota limit', async () => {
    const limit = await getUserQuotaLimit('user-1');

    expect(limit).toBe(defaultLimit);
    expect(mockServerApi).not.toHaveBeenCalled();
  });
});

// ---------------------------------------------------------------------------
// getQuotaUsage
// ---------------------------------------------------------------------------

describe('getQuotaUsage', () => {
  it('returns quota usage from the Rust admin API', async () => {
    mockServerApi.mockResolvedValue({
      ai_extraction: {
        allowed: false,
        current_usage: 10,
        daily_limit: 10,
        remaining: 0,
      },
    });

    const result = await getQuotaUsage('user-1');

    expect(result).toEqual({
      allowed: false,
      current_usage: 10,
      daily_limit: 10,
      remaining: 0,
    });
    expect(mockServerApi).toHaveBeenCalledWith('/api/admin/users/user-1/quota');
  });

  it('returns the requested resource quota when available', async () => {
    mockServerApi.mockResolvedValue({
      ai_extraction: {
        allowed: true,
        current_usage: 1,
        daily_limit: 10,
        remaining: 9,
      },
      ai_categorisation: {
        allowed: true,
        current_usage: 12,
        daily_limit: 50,
        remaining: 38,
      },
    });

    const result = await getQuotaUsage(
      'user-1',
      USAGE_QUOTA_CONSTANTS.RESOURCE_TYPES.AI_CATEGORISATION
    );

    expect(result).toEqual({
      allowed: true,
      current_usage: 12,
      daily_limit: 50,
      remaining: 38,
    });
  });

  it('returns the local fallback when the API is unavailable', async () => {
    mockServerApi.mockRejectedValue(new Error('offline'));

    const result = await getQuotaUsage('user-1');

    expect(result).toEqual({
      allowed: true,
      current_usage: 0,
      daily_limit: defaultLimit,
      remaining: defaultLimit,
    });
  });
});
