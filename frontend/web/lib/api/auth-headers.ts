export const AUTH_SESSION_COOKIE_NAME = 'wqn_session';
export const AUTH_CHECKED_HEADER = 'x-wqn-auth-checked';
export const AUTH_CHECKED_VALUE = '1';
export const AUTH_USER_HEADER = 'x-wqn-user';

export function encodeAuthUserHeader(user: unknown): string {
  return encodeURIComponent(JSON.stringify(user));
}

export function decodeAuthUserHeader<T = unknown>(
  value: string | null
): T | null {
  if (!value) return null;

  try {
    return JSON.parse(decodeURIComponent(value)) as T;
  } catch {
    return null;
  }
}
