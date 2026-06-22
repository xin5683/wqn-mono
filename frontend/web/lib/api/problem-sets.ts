import type {
  ProblemInSet,
  ProblemSetDetail,
  ProblemSetDetailApiResponse,
  ProblemSetProblemLink,
  ProblemSetWithDetails,
} from '@/lib/types';

export function flattenProblemSetProblem(
  link: ProblemSetProblemLink
): ProblemInSet {
  return {
    ...link.problems,
    added_at: link.added_at ?? null,
    tags: link.problems.tags ?? [],
  };
}

export function normalizeProblemSetDetail(
  problemSet: ProblemSetDetailApiResponse
): ProblemSetDetail & { problems: ProblemInSet[] } {
  return {
    ...problemSet,
    problems: (problemSet.problems ?? []).map(flattenProblemSetProblem),
  };
}

export function ensureProblemSetDetails<T extends ProblemSetDetail>(
  problemSet: T
): T & ProblemSetWithDetails {
  return {
    ...problemSet,
    subject_name: problemSet.subject_name ?? 'Unknown',
    problem_count: problemSet.problem_count ?? problemSet.problems?.length ?? 0,
  };
}
