import { notFound, redirect } from 'next/navigation';
import { getTranslations } from 'next-intl/server';
import ProblemReview from '../../../subjects/[id]/problems/[problemId]/review/problem-review';
import SessionReviewClient from './session-review-client';
import { BackLink } from '@/components/back-link';
import { getCurrentUser, serverApi } from '@/lib/api/server';
import {
  ensureProblemSetDetails,
  normalizeProblemSetDetail,
} from '@/lib/api/problem-sets';
import { ProblemSetDetailResponseSchema } from '@/lib/api/response-schemas';
import type {
  ProblemInSet,
  ProblemSetDetail,
  ProblemSetWithDetails,
} from '@/lib/types';

type ReviewProblemSet = ProblemSetDetail &
  ProblemSetWithDetails & {
    problems: ProblemInSet[];
    isOwner: boolean;
  };

export async function generateMetadata({
  params,
}: {
  params: Promise<{ id: string }>;
}) {
  const { id } = await params;
  const problemSet = await loadProblemSet(id);
  const tMeta = await getTranslations('Metadata');
  return {
    title: tMeta('reviewItemMetaTitle', { name: problemSet?.name ?? '' }),
  };
}

async function loadProblemSet(id: string): Promise<ReviewProblemSet | null> {
  const user = await getCurrentUser();
  const problemSet = await serverApi(
    `/api/problem-sets/${encodeURIComponent(id)}`,
    {},
    ProblemSetDetailResponseSchema
  ).catch(() => null);
  if (!problemSet) return null;
  const normalized = ensureProblemSetDetails(
    normalizeProblemSetDetail(problemSet)
  );
  const isOwner = !!user && problemSet.user_id === user.id;

  return {
    ...normalized,
    subject_name: problemSet.subject_name || 'Unknown',
    isOwner,
  };
}

export default async function ProblemSetReviewPage({
  params,
  searchParams,
}: {
  params: Promise<{ id: string }>;
  searchParams: Promise<{ problemId?: string; sessionId?: string }>;
}) {
  const t = await getTranslations('ProblemSets');
  const { id } = await params;
  const { problemId, sessionId } = await searchParams;
  const user = await getCurrentUser();

  // Session-based review requires authentication (sessions are tied to user_id)
  if (sessionId) {
    if (!user) {
      redirect(`/auth/login?redirect=/problem-sets/${id}/review`);
    }

    const problemSet = await loadProblemSet(id);
    if (!problemSet) {
      notFound();
    }

    return (
      <SessionReviewClient
        problemSetId={id}
        sessionId={sessionId}
        subjectId={problemSet.subject_id}
        subjectName={problemSet.subject_name}
        isReadOnly={!problemSet.isOwner}
        allowCopying={!problemSet.isOwner && problemSet.allow_copying}
      />
    );
  }

  // Legacy review mode: allow anonymous access for public problem sets
  const problemSet = await loadProblemSet(id);
  if (!problemSet) {
    // If no user and problem set not found (could be private), redirect to login
    if (!user) {
      redirect(`/auth/login?redirect=/problem-sets/${id}/review`);
    }
    notFound();
  }

  // If anonymous user and problem set is not public, redirect to login
  if (!user && problemSet.sharing_level !== 'public') {
    redirect(`/auth/login?redirect=/problem-sets/${id}/review`);
  }

  const problems = problemSet.problems.map(problem => ({
    ...problem,
    assets: problem.assets || [],
    solution_assets: problem.solution_assets || [],
  }));

  if (problems.length === 0) {
    return (
      <div className="container mx-auto py-6">
        <div className="text-center">
          <h1 className="text-2xl font-bold mb-4">{t('noProblemsInSet')}</h1>
          <p className="text-muted-foreground mb-4">
            {t('thisProblemSetHasNoProblemsYet')}
          </p>
          <BackLink href={`/problem-sets/${id}`}>{t('backToSet')}</BackLink>
        </div>
      </div>
    );
  }

  // If no specific problem is requested, redirect to the first problem
  if (!problemId) {
    return (
      <div className="container mx-auto py-6">
        <div className="text-center">
          <h1 className="text-2xl font-bold mb-4">{t('startingReview')}</h1>
          <p className="text-muted-foreground mb-4">
            {t('redirectingToFirstProblem')}
          </p>
          <script>
            {`window.location.href = '/problem-sets/${id}/review?problemId=${problems[0].id}';`}
          </script>
        </div>
      </div>
    );
  }

  // Find the current problem
  const currentProblem = problems.find(p => p.id === problemId);
  if (!currentProblem) {
    notFound();
  }

  // Find the current problem index
  const currentIndex = problems.findIndex(p => p.id === problemId);
  const prevProblem = currentIndex > 0 ? problems[currentIndex - 1] : null;
  const nextProblem =
    currentIndex < problems.length - 1 ? problems[currentIndex + 1] : null;

  return (
    <ProblemReview
      key={currentProblem.id}
      problem={currentProblem}
      subject={{ id: problemSet.subject_id, name: problemSet.subject_name }}
      allProblems={problems}
      prevProblem={prevProblem}
      nextProblem={nextProblem}
      isProblemSetMode={true}
      problemSetId={id}
      isReadOnly={!problemSet.isOwner}
      allowCopying={!problemSet.isOwner && problemSet.allow_copying}
      copyProblemSetId={id}
      isAuthenticated={!!user}
    />
  );
}
