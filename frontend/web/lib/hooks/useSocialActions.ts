'use client';

import { useCallback, useEffect, useMemo, useRef } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import type {
  ProblemSetStats,
  ProblemSetStatsResponse,
  UserSocialState,
} from '@/lib/types';
import { PROBLEM_SET_CONSTANTS } from '@/lib/constants';
import { clientApi } from '@/lib/api/client';
import { queryKeys } from '@/lib/api/query-keys';

interface UseSocialActionsProps {
  problemSetId: string;
  initialStats?: ProblemSetStats | null;
  initialSocialState?: UserSocialState | null;
  isAuthenticated: boolean;
  trackView?: boolean;
}

interface UseSocialActionsReturn {
  stats: ProblemSetStats;
  liked: boolean;
  favourited: boolean;
  toggleLike: () => Promise<void>;
  toggleFavourite: () => Promise<void>;
  likeLoading: boolean;
  favouriteLoading: boolean;
}

const defaultStats: ProblemSetStats = {
  view_count: 0,
  unique_view_count: 0,
  like_count: 0,
  copy_count: 0,
  problem_count: 0,
  ranking_score: 0,
};

interface LegacyStatsResponse {
  stats?: Partial<ProblemSetStats>;
  social_state?: Partial<UserSocialState>;
}

type StatsResponse = ProblemSetStatsResponse & LegacyStatsResponse;

interface LikeToggleResponse {
  liked: boolean;
  like_count: number;
}

interface FavouriteToggleResponse {
  favourited: boolean;
  favorited?: boolean;
}

interface SocialActionsData {
  stats: ProblemSetStats;
  liked: boolean;
  favourited: boolean;
}

function toProblemSetStats(
  value: Partial<ProblemSetStats> | null | undefined
): ProblemSetStats {
  return { ...defaultStats, ...(value ?? {}) };
}

function normaliseStatsResponse(data: StatsResponse): SocialActionsData {
  return {
    stats: toProblemSetStats(data.stats ?? data),
    liked: Boolean(data.social_state?.liked ?? data.liked),
    favourited: Boolean(
      data.social_state?.favourited ?? data.favourited ?? data.favorited
    ),
  };
}

export function useSocialActions({
  problemSetId,
  initialStats,
  initialSocialState,
  isAuthenticated,
  trackView = false,
}: UseSocialActionsProps): UseSocialActionsReturn {
  const queryClient = useQueryClient();
  const statsQueryKey = useMemo(
    () => queryKeys.problemSetStats(problemSetId),
    [problemSetId]
  );
  const fallbackData = useMemo<SocialActionsData>(
    () => ({
      stats: toProblemSetStats(initialStats),
      liked: initialSocialState?.liked ?? false,
      favourited: initialSocialState?.favourited ?? false,
    }),
    [initialStats, initialSocialState]
  );
  const viewTracked = useRef(false);

  const statsQuery = useQuery({
    queryKey: statsQueryKey,
    queryFn: async () =>
      normaliseStatsResponse(
        await clientApi<StatsResponse>(
          `/api/problem-sets/${problemSetId}/stats`
        )
      ),
    initialData: initialStats ? fallbackData : undefined,
  });

  const data = statsQuery.data ?? fallbackData;

  useEffect(() => {
    viewTracked.current = false;
  }, [problemSetId]);

  const { mutate: trackProblemSetView } = useMutation({
    mutationFn: () =>
      clientApi(`/api/problem-sets/${problemSetId}/view`, {
        method: 'POST',
      }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: statsQueryKey });
    },
  });

  // Track view after the configured delay (filters out bounces).
  useEffect(() => {
    if (!trackView || viewTracked.current) return;

    const timer = setTimeout(() => {
      viewTracked.current = true;
      trackProblemSetView();
    }, PROBLEM_SET_CONSTANTS.VIEW_TRACKING_DELAY_MS);

    return () => clearTimeout(timer);
  }, [trackProblemSetView, trackView]);

  const likeMutation = useMutation<
    LikeToggleResponse,
    Error,
    void,
    { previous: SocialActionsData }
  >({
    mutationFn: () =>
      clientApi<LikeToggleResponse>(`/api/problem-sets/${problemSetId}/like`, {
        method: 'POST',
      }),
    onMutate: async () => {
      await queryClient.cancelQueries({ queryKey: statsQueryKey });
      const previous =
        queryClient.getQueryData<SocialActionsData>(statsQueryKey) ?? data;

      queryClient.setQueryData<SocialActionsData>(statsQueryKey, current => {
        const snapshot = current ?? previous;
        const nextLiked = !snapshot.liked;
        return {
          ...snapshot,
          liked: nextLiked,
          stats: {
            ...snapshot.stats,
            like_count: nextLiked
              ? snapshot.stats.like_count + 1
              : Math.max(0, snapshot.stats.like_count - 1),
          },
        };
      });

      return { previous };
    },
    onError: (_error, _variables, context) => {
      queryClient.setQueryData(statsQueryKey, context?.previous);
    },
    onSuccess: result => {
      queryClient.setQueryData<SocialActionsData>(statsQueryKey, current => ({
        ...(current ?? data),
        liked: result.liked,
        stats: {
          ...(current ?? data).stats,
          like_count: result.like_count,
        },
      }));
    },
    onSettled: () => {
      queryClient.invalidateQueries({ queryKey: statsQueryKey });
    },
  });

  const favouriteMutation = useMutation<
    FavouriteToggleResponse,
    Error,
    void,
    { previous: SocialActionsData }
  >({
    mutationFn: () =>
      clientApi<FavouriteToggleResponse>(
        `/api/problem-sets/${problemSetId}/favourite`,
        {
          method: 'POST',
        }
      ),
    onMutate: async () => {
      await queryClient.cancelQueries({ queryKey: statsQueryKey });
      const previous =
        queryClient.getQueryData<SocialActionsData>(statsQueryKey) ?? data;

      queryClient.setQueryData<SocialActionsData>(statsQueryKey, current => {
        const snapshot = current ?? previous;
        return {
          ...snapshot,
          favourited: !snapshot.favourited,
        };
      });

      return { previous };
    },
    onError: (_error, _variables, context) => {
      queryClient.setQueryData(statsQueryKey, context?.previous);
    },
    onSuccess: result => {
      queryClient.setQueryData<SocialActionsData>(statsQueryKey, current => ({
        ...(current ?? data),
        favourited: result.favourited ?? result.favorited ?? false,
      }));
    },
    onSettled: () => {
      queryClient.invalidateQueries({ queryKey: statsQueryKey });
    },
  });

  const toggleLike = useCallback(async () => {
    if (!isAuthenticated || likeMutation.isPending) return;

    try {
      await likeMutation.mutateAsync();
    } catch {
      // Rollback is handled by the mutation.
    }
  }, [isAuthenticated, likeMutation]);

  const toggleFavourite = useCallback(async () => {
    if (!isAuthenticated || favouriteMutation.isPending) return;

    try {
      await favouriteMutation.mutateAsync();
    } catch {
      // Rollback is handled by the mutation.
    }
  }, [favouriteMutation, isAuthenticated]);

  return {
    stats: data.stats,
    liked: data.liked,
    favourited: data.favourited,
    toggleLike,
    toggleFavourite,
    likeLoading: likeMutation.isPending,
    favouriteLoading: favouriteMutation.isPending,
  };
}
