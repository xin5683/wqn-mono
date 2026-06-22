import { getCurrentUser, serverApi } from '@/lib/api/server';
import { notFound, redirect } from 'next/navigation';
import { getTranslations } from 'next-intl/server';
import SpacedReviewClient from './spaced-review-client';
import { SubjectResponseSchema } from '@/lib/api/response-schemas';
import type { ReviewSessionResponse } from '@/lib/types';

export async function generateMetadata() {
  const t = await getTranslations('Metadata');
  return { title: t('spacedReviewMetaTitle') };
}

export default async function SpacedReviewPage({
  params,
  searchParams,
}: {
  params: Promise<{ id: string }>;
  searchParams: Promise<{ sessionId?: string }>;
}) {
  const { id: subjectId } = await params;
  const { sessionId } = await searchParams;

  const user = await getCurrentUser();
  if (!user) {
    redirect(`/auth/login?redirect=/subjects/${subjectId}/review-due`);
  }

  if (!sessionId) {
    notFound();
  }

  const session = await serverApi<ReviewSessionResponse>(
    `/api/review-sessions/${encodeURIComponent(sessionId)}`
  ).catch(() => null);

  if (!session?.session || session.session.subject_id !== subjectId) {
    notFound();
  }

  const subject = await serverApi(
    `/api/subjects/${subjectId}`,
    {},
    SubjectResponseSchema
  ).catch(() => null);

  return (
    <SpacedReviewClient
      subjectId={subjectId}
      subjectName={subject?.name || 'Unknown'}
      sessionId={sessionId}
    />
  );
}
