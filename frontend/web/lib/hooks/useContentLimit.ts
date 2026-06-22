'use client';

import { useCallback } from 'react';
import { useQuery } from '@tanstack/react-query';
import { CONTENT_LIMIT_CONSTANTS } from '@/lib/constants';
import type { ContentLimitResult } from '@/lib/api/content-limits';
import { clientApi } from '@/lib/api/client';
import { queryKeys } from '@/lib/api/query-keys';

const { WARNING_THRESHOLD } = CONTENT_LIMIT_CONSTANTS;

export function useContentLimit(resourceType: string, subjectId?: string) {
  const query = useQuery({
    queryKey: queryKeys.contentLimit(resourceType, subjectId),
    queryFn: async () => {
      const params = new URLSearchParams();
      if (subjectId) params.set('subject_id', subjectId);
      const qs = params.toString();
      const url = `/api/usage/${resourceType}${qs ? `?${qs}` : ''}`;
      return clientApi<ContentLimitResult>(url);
    },
  });
  const { refetch } = query;

  const refresh = useCallback(async () => {
    await refetch();
  }, [refetch]);

  const data = query.data ?? null;
  const ratio = data && data.limit > 0 ? data.current / data.limit : 0;
  const isWarning = ratio >= WARNING_THRESHOLD && ratio < 1;
  const isExhausted = data ? data.current >= data.limit : false;

  return {
    loading: query.isLoading,
    data,
    refresh,
    isWarning,
    isExhausted,
  };
}
