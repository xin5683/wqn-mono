import { notFound } from 'next/navigation';
import { stripHtml } from '@/lib/security/html-sanitizer';
import { getTranslations } from 'next-intl/server';
import ProblemSetPageClient from './problem-set-page-client';
import { getCurrentUser, serverApi } from '@/lib/api/server';
import { absoluteSiteUrl } from '@/lib/api/url';
import {
  ensureProblemSetDetails,
  normalizeProblemSetDetail,
} from '@/lib/api/problem-sets';
import { ProblemSetDetailResponseSchema } from '@/lib/api/response-schemas';
import type {
  ProblemInSet,
  ProblemSetDetail,
  ProblemSetStats,
  ProblemSetStatsResponse,
  ProblemSetWithDetails,
  UserSocialState,
} from '@/lib/types';

type ProblemSetPageData = ProblemSetDetail &
  ProblemSetWithDetails & {
    problems: ProblemInSet[];
  };

export async function generateMetadata({
  params,
}: {
  params: Promise<{ id: string }>;
}) {
  const { id } = await params;
  const problemSet = await loadProblemSet(id);
  const t = await getTranslations('Metadata');

  if (!problemSet) {
    return { title: t('problemSetNotFoundMetaTitle') };
  }

  const stripped = problemSet.description
    ? stripHtml(problemSet.description)
    : '';
  const description =
    stripped.length > 160
      ? stripped.substring(0, 160) + '...'
      : stripped ||
        `${problemSet.problem_count} problems in ${problemSet.subject_name}`;

  return {
    title: problemSet.name,
    description,
    openGraph: {
      title: problemSet.name,
      description,
      type: 'article' as const,
      url: absoluteSiteUrl(`/problem-sets/${id}`),
      siteName: t('siteName'),
    },
    twitter: {
      card: 'summary' as const,
      title: problemSet.name,
      description,
    },
    alternates: {
      canonical: absoluteSiteUrl(`/problem-sets/${id}`),
    },
  };
}

async function loadProblemSet(id: string): Promise<ProblemSetPageData | null> {
  const problemSet = await serverApi(
    `/api/problem-sets/${encodeURIComponent(id)}`,
    {},
    ProblemSetDetailResponseSchema
  ).catch(() => null);
  return problemSet
    ? ensureProblemSetDetails(normalizeProblemSetDetail(problemSet))
    : null;
}

async function loadSocialData(id: string, userId: string | null) {
  const stats = await serverApi<ProblemSetStatsResponse>(
    `/api/problem-sets/${encodeURIComponent(id)}/stats`
  ).catch(() => null);
  const initialStats: ProblemSetStats | null = stats
    ? {
        view_count: stats.view_count,
        unique_view_count: stats.unique_view_count ?? stats.view_count,
        like_count: stats.like_count,
        copy_count: stats.copy_count,
        problem_count: stats.problem_count ?? 0,
        ranking_score: stats.ranking_score ?? 0,
      }
    : null;
  const socialState: UserSocialState | null = userId
    ? {
        liked: Boolean(stats?.liked),
        favourited: Boolean(stats?.favourited),
      }
    : null;
  return { stats: initialStats, socialState };
}

export default async function ProblemSetPage({
  params,
  searchParams,
}: {
  params: Promise<{ id: string }>;
  searchParams: Promise<{ from?: string }>;
}) {
  const { id } = await params;
  const { from } = await searchParams;
  const problemSet = await loadProblemSet(id);

  if (!problemSet) {
    notFound();
  }

  const user = await getCurrentUser();

  // Fetch social data for non-private sets
  const isShared = problemSet.sharing_level !== 'private';
  const { stats, socialState } = isShared
    ? await loadSocialData(id, user?.id ?? null)
    : { stats: null, socialState: null };

  const hasUsername = user ? Boolean(user.profile?.username) : true;

  // Sanitize `from` param to prevent open-redirect via external URLs
  const backHref =
    from && from.startsWith('/') && !from.startsWith('//')
      ? from
      : '/problem-sets';

  return (
    <ProblemSetPageClient
      initialProblemSet={problemSet}
      isAuthenticated={!!user}
      ownerProfile={problemSet.ownerProfile ?? problemSet.owner_profile ?? null}
      initialStats={stats}
      initialSocialState={socialState}
      hasUsername={hasUsername}
      backHref={backHref}
    />
  );
}
