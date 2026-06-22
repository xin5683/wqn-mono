'use client';

import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { clientApi } from '@/lib/api/client';
import { queryKeys } from '@/lib/api/query-keys';
import {
  ProblemListResponseSchema,
  ProblemResponseSchema,
  type ProblemResponse,
} from '@/lib/api/response-schemas';

export type ProblemMutationInput = {
  problemId: string;
  payload: Record<string, unknown>;
};

// 列表读取带 Zod 校验：契约漂移（如 answer_config 不符合 union）会让
// clientApi 抛错 → useQuery error → UI 错误态，而不再是静默的空列表。
export function useProblems(
  subjectId: string,
  options?: { initialData?: ProblemResponse[] }
) {
  return useQuery({
    queryKey: queryKeys.problems(subjectId),
    queryFn: () =>
      clientApi<ProblemResponse[]>(
        `/api/problems?subject_id=${subjectId}`,
        {},
        ProblemListResponseSchema
      ),
    initialData: options?.initialData,
  });
}

export function useCreateProblem(subjectId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (payload: Record<string, unknown>) =>
      clientApi<ProblemResponse>(
        '/api/problems',
        { method: 'POST', body: payload },
        ProblemResponseSchema
      ),
    onSuccess: () => {
      queryClient.invalidateQueries({
        queryKey: queryKeys.problems(subjectId),
      });
    },
  });
}

export function useUpdateProblem(subjectId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ problemId, payload }: ProblemMutationInput) =>
      clientApi<ProblemResponse>(
        `/api/problems/${problemId}`,
        { method: 'PATCH', body: payload },
        ProblemResponseSchema
      ),
    onSuccess: () => {
      queryClient.invalidateQueries({
        queryKey: queryKeys.problems(subjectId),
      });
    },
  });
}

export function useDeleteProblem(subjectId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (problemId: string) =>
      clientApi<void>(`/api/problems/${problemId}`, { method: 'DELETE' }),
    onSuccess: () => {
      queryClient.invalidateQueries({
        queryKey: queryKeys.problems(subjectId),
      });
    },
  });
}
