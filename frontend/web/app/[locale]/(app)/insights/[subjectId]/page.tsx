import { getCurrentUser, serverApi } from '@/lib/api/server';
import { redirect, notFound } from 'next/navigation';
import { getTranslations } from 'next-intl/server';
import SubjectInsightsClient from './subject-insights-client';
import {
  InsightStatusResponseSchema,
  SubjectResponseSchema,
} from '@/lib/api/response-schemas';

export async function generateMetadata() {
  const t = await getTranslations('Metadata');
  return { title: t('subjectInsightsMetaTitle') };
}

export default async function SubjectInsightsPage({
  params,
}: {
  params: Promise<{ subjectId: string }>;
}) {
  const { subjectId } = await params;
  const user = await getCurrentUser();
  if (!user) redirect(`/auth/login?redirect=/insights/${subjectId}`);

  const subject = await serverApi(
    `/api/subjects/${subjectId}`,
    {},
    SubjectResponseSchema
  ).catch(() => null);

  if (!subject) notFound();

  const status = await serverApi(
    '/api/insights/status',
    {},
    InsightStatusResponseSchema
  ).catch(() => null);
  const digest = status?.digest ?? null;

  return (
    <SubjectInsightsClient
      subject={{
        id: subject.id,
        name: subject.name,
        color: subject.color ?? null,
      }}
      digest={digest}
      reviewSessions={[]}
    />
  );
}
