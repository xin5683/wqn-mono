import { getCurrentUser, serverApi } from '@/lib/api/server';
import { redirect } from 'next/navigation';
import { getTranslations } from 'next-intl/server';
import { INSIGHT_CONSTANTS } from '@/lib/constants';
import InsightsPageClient from './insights-page-client';
import {
  InsightStatusResponseSchema,
  SubjectListResponseSchema,
} from '@/lib/api/response-schemas';

export async function generateMetadata() {
  const t = await getTranslations('Metadata');
  return { title: t('insightsMetaTitle') };
}

export default async function InsightsPage() {
  const user = await getCurrentUser();
  if (!user) redirect('/auth/login?redirect=/insights');

  const status = await serverApi(
    '/api/insights/status',
    {},
    InsightStatusResponseSchema
  ).catch(() => null);
  const latestRow = status?.latest ?? null;

  // Determine if generation is in progress
  let isGenerating = false;
  let digest = status?.digest ?? null;

  if (latestRow?.status === 'generating') {
    const staleThreshold = new Date(
      Date.now() - INSIGHT_CONSTANTS.GENERATING_STALE_MINUTES * 60 * 1000
    );
    const generatedAt = latestRow.generated_at
      ? new Date(latestRow.generated_at)
      : null;

    if (!generatedAt || generatedAt < staleThreshold) {
      // Stale — treat as no digest
      digest = null;
    } else {
      isGenerating = true;
      digest = null;
    }
  } else if (latestRow?.status === 'failed') {
    // Failed — treat as no digest
    digest = null;
  }

  const subjects = await serverApi(
    '/api/subjects',
    {},
    SubjectListResponseSchema
  ).catch(() => []);

  return (
    <InsightsPageClient
      initialDigest={digest}
      initialIsGenerating={isGenerating}
      subjects={(subjects || []).map(subject => ({
        id: subject.id,
        name: subject.name,
        color: subject.color ?? null,
      }))}
    />
  );
}
