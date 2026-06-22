import { notFound } from 'next/navigation';
import { getTranslations } from 'next-intl/server';
import SummaryClient from './summary-client';
import { getCurrentUser, serverApi } from '@/lib/api/server';
import type { ProblemSetDetail } from '@/lib/types';

type ProblemSetSummaryResponse = Pick<
  ProblemSetDetail,
  'name' | 'subject_name'
>;

export async function generateMetadata() {
  const t = await getTranslations('Metadata');
  return { title: t('reviewSummaryMetaTitle') };
}

export default async function SummaryPage({
  params,
  searchParams,
}: {
  params: Promise<{ id: string }>;
  searchParams: Promise<{ sessionId?: string }>;
}) {
  const { id } = await params;
  const { sessionId } = await searchParams;

  if (!sessionId) {
    notFound();
  }

  const user = await getCurrentUser();

  if (!user) {
    notFound();
  }

  const problemSet = await serverApi<ProblemSetSummaryResponse>(
    `/api/problem-sets/${encodeURIComponent(id)}`
  ).catch(() => null);

  if (!problemSet) {
    notFound();
  }

  return (
    <SummaryClient
      problemSetId={id}
      sessionId={sessionId}
      problemSetName={problemSet.name}
      subjectName={problemSet.subject_name || 'Unknown'}
    />
  );
}
