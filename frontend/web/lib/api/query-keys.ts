export const queryKeys = {
  contentLimit: (resourceType: string, subjectId?: string) =>
    ['content-limit', resourceType, subjectId ?? null] as const,
  problemSetStats: (problemSetId: string) =>
    ['problem-set', problemSetId, 'stats'] as const,
  usage: () => ['usage'] as const,
  adminLimitDefaults: () => ['admin', 'limit-defaults'] as const,
  problems: (subjectId: string) => ['problems', subjectId] as const,
  problem: (problemId: string) => ['problem', problemId] as const,
};
