import type { z } from 'zod';

function formatIssuePath(path: PropertyKey[]) {
  return path.length > 0 ? path.join('.') : '<root>';
}

export function validatePayload<T>(
  payload: unknown,
  schema: z.ZodType<T>,
  label: string
): T {
  const parsed = schema.safeParse(payload);

  if (!parsed.success) {
    const details = parsed.error.issues
      .map(issue => `${formatIssuePath(issue.path)}: ${issue.message}`)
      .join('; ');
    throw new Error(`Invalid ${label} payload: ${details}`);
  }

  return parsed.data;
}
