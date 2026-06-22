import ProblemSetsPageClient from './problem-sets-page-client';
import type { Metadata } from 'next';
import { getTranslations } from 'next-intl/server';
import { getCurrentUser, serverApi } from '@/lib/api/server';
import { ProblemSetWithDetails } from '@/lib/types';

export async function generateMetadata(): Promise<Metadata> {
  const t = await getTranslations('Metadata');
  return {
    title: t('allProblemSetsMetaTitle'),
    description: t('allProblemSetsMetaDescription'),
  };
}

async function loadProblemSets() {
  const user = await getCurrentUser();

  if (!user) {
    return {
      data: [] as ProblemSetWithDetails[],
      statsMap: {},
      hasUsername: false,
    };
  }

  const problemSets = await serverApi<ProblemSetWithDetails[]>(
    '/api/problem-sets'
  ).catch(() => []);
  return {
    data: problemSets,
    statsMap: {},
    hasUsername: Boolean(user.profile?.username),
  };
}

export default async function ProblemSetsPage() {
  const { data, statsMap, hasUsername } = await loadProblemSets();

  return (
    <ProblemSetsPageClient
      initialProblemSets={data}
      statsMap={statsMap}
      hasUsername={hasUsername}
    />
  );
}
