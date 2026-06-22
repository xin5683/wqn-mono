import type { z } from 'zod';

function formatIssuePath(path: PropertyKey[]) {
  return path.length > 0 ? path.join('.') : '<root>';
}

export function validateApiData<T>(
  data: unknown,
  schema: z.ZodType<T>,
  path: string
): T {
  const parsed = schema.safeParse(data);

  if (!parsed.success) {
    const details = parsed.error.issues
      .map(issue => `${formatIssuePath(issue.path)}: ${issue.message}`)
      .join('; ');
    throw new Error(`Invalid API response for ${path}: ${details}`);
  }

  return parsed.data;
}
