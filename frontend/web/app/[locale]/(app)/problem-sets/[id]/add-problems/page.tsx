import { notFound } from 'next/navigation';
import AddProblemsToSetClient from './add-problems-to-set-client';
import { serverApi } from '@/lib/api/server';
import {
  ensureProblemSetDetails,
  normalizeProblemSetDetail,
} from '@/lib/api/problem-sets';
import {
  ProblemListResponseSchema,
  ProblemSetDetailResponseSchema,
} from '@/lib/api/response-schemas';
import type { Tag } from '@/lib/types';

async function loadProblemSet(id: string) {
  const problemSet = await serverApi(
    `/api/problem-sets/${encodeURIComponent(id)}`,
    {},
    ProblemSetDetailResponseSchema
  ).catch(() => null);
  return problemSet
    ? ensureProblemSetDetails(normalizeProblemSetDetail(problemSet))
    : null;
}

async function loadData(subjectId: string) {
  const [problems, availableTags] = await Promise.all([
    serverApi(
      `/api/problems?subject_id=${subjectId}`,
      {},
      ProblemListResponseSchema
    ),
    serverApi<Tag[]>(`/api/tags?subject_id=${subjectId}`),
  ]);
  const tagsByProblem: Record<string, Tag[]> = {};
  for (const problem of problems ?? []) {
    tagsByProblem[problem.id] = problem.tags || [];
  }

  return {
    problems: problems ?? [],
    tagsByProblem,
    availableTags: availableTags ?? [],
  };
}

export default async function AddProblemsToSetPage({
  params,
}: {
  params: Promise<{ id: string }>;
}) {
  const { id } = await params;

  const problemSet = await loadProblemSet(id);
  if (!problemSet) {
    notFound();
  }

  const [{ problems, tagsByProblem, availableTags }, problemSetProblemIds] =
    await Promise.all([
      loadData(problemSet.subject_id),
      Promise.resolve((problemSet.problems ?? []).map(problem => problem.id)),
    ]);

  return (
    <AddProblemsToSetClient
      problemSet={problemSet}
      problems={problems}
      tagsByProblem={tagsByProblem}
      availableTags={availableTags}
      problemSetProblemIds={problemSetProblemIds}
    />
  );
}
