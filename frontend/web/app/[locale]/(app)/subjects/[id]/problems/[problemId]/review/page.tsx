import { notFound } from 'next/navigation';
import { getTranslations } from 'next-intl/server';
import ProblemReview from './problem-review';
import { serverApi } from '@/lib/api/server';
import {
  ProblemListResponseSchema,
  ProblemResponseSchema,
  SubjectResponseSchema,
} from '@/lib/api/response-schemas';

export async function generateMetadata({
  params,
}: {
  params: Promise<{ id: string; problemId: string }>;
}) {
  const { id: subjectId, problemId } = await params;
  const { problem } = await loadData(subjectId, problemId);
  const t = await getTranslations('Metadata');
  return {
    title: t('reviewItemMetaTitle', { name: problem?.title ?? '' }),
  };
}

async function loadData(subjectId: string, problemId: string) {
  try {
    const [problem, subject, allProblems] = await Promise.all([
      serverApi(`/api/problems/${problemId}`, {}, ProblemResponseSchema),
      serverApi(`/api/subjects/${subjectId}`, {}, SubjectResponseSchema),
      serverApi(
        `/api/problems?subject_id=${subjectId}`,
        {},
        ProblemListResponseSchema
      ),
    ]);
    return {
      problem,
      subject,
      allProblems:
        allProblems?.map(({ id, title, problem_type, status }) => ({
          id,
          title,
          problem_type,
          status,
        })) || [],
    };
  } catch {
    return { problem: null, subject: null, allProblems: [] };
  }
}

export default async function ProblemReviewPage({
  params,
}: {
  params: Promise<{ id: string; problemId: string }>;
}) {
  const { id: subjectId, problemId } = await params;
  const { problem, subject, allProblems } = await loadData(
    subjectId,
    problemId
  );

  if (!problem || !subject) {
    notFound();
  }

  return (
    <ProblemReview
      problem={problem}
      subject={subject}
      allProblems={allProblems}
    />
  );
}
