import { PROBLEM_SET_CONSTANTS } from '@/lib/constants';
import { getTranslations } from 'next-intl/server';
import DiscoverPageClient from './discover-page-client';
import { serverApi } from '@/lib/api/server';
import type {
  DiscoverProblemSetItem,
  DiscoverResponse,
  ProblemSetCard,
} from '@/lib/types';
import { absoluteSiteUrl } from '@/lib/api/url';

export async function generateMetadata() {
  const t = await getTranslations('Discover');
  const tMeta = await getTranslations('Metadata');
  const siteName = tMeta('siteName');
  return {
    title: t('metaTitle'),
    description: t('metaDescription'),
    openGraph: {
      title: `${t('metaTitle')} – ${siteName}`,
      description: t('metaDescription'),
      url: absoluteSiteUrl('/discover'),
      siteName,
    },
    alternates: {
      canonical: absoluteSiteUrl('/discover'),
    },
  };
}

async function loadDiscoveryData() {
  const data = await serverApi<DiscoverResponse>(
    `/api/discover?limit=${PROBLEM_SET_CONSTANTS.DISCOVERY_PAGE_SIZE}`
  ).catch((): DiscoverResponse => ({ items: [] }));
  const items = data.items ?? data.data ?? [];
  const sets: ProblemSetCard[] = items.map((set: DiscoverProblemSetItem) => ({
    id: set.id,
    name: set.name,
    description: set.description,
    subject_name: set.discovery_subject || set.subject_name || 'Other',
    subject_color: set.subject_color ?? null,
    subject_icon: set.subject_icon ?? null,
    problem_count: set.problem_count ?? 0,
    is_smart: set.is_smart,
    owner: {
      username: set.owner?.username ?? set.owner_username ?? null,
      display_name:
        set.owner?.display_name ?? set.owner_username ?? 'Anonymous',
      avatar_url: set.owner?.avatar_url ?? set.owner_avatar_url ?? null,
    },
    stats: {
      view_count: set.stats?.view_count ?? set.view_count ?? 0,
      unique_view_count:
        set.stats?.unique_view_count ??
        set.unique_view_count ??
        set.view_count ??
        0,
      like_count: set.stats?.like_count ?? set.like_count ?? 0,
      copy_count: set.stats?.copy_count ?? set.copy_count ?? 0,
      problem_count: set.stats?.problem_count ?? set.problem_count ?? 0,
      ranking_score: set.stats?.ranking_score ?? set.ranking_score ?? 0,
    },
    created_at: set.created_at,
  }));
  const subjectMap = new Map<string, number>();
  for (const set of sets) {
    subjectMap.set(
      set.subject_name,
      (subjectMap.get(set.subject_name) || 0) + 1
    );
  }
  const subjects = [...subjectMap.entries()].map(([name, count]) => ({
    name,
    count,
  }));
  return { sets, subjects };
}

export default async function DiscoverPage() {
  const { sets, subjects } = await loadDiscoveryData();

  return <DiscoverPageClient initialSets={sets} initialSubjects={subjects} />;
}
