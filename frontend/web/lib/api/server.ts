import { cookies, headers } from 'next/headers';
import type { z } from 'zod';
import {
  AUTH_CHECKED_HEADER,
  AUTH_CHECKED_VALUE,
  AUTH_SESSION_COOKIE_NAME,
  AUTH_USER_HEADER,
  decodeAuthUserHeader,
} from '@/lib/api/auth-headers';
import {
  CurrentUserResponseSchema,
  type CurrentUserResponse,
} from '@/lib/api/response-schemas';
import { apiFetchCore } from '@/lib/api/fetch-core';
import { rustApiUrl } from '@/lib/api/url';

export async function serverApi<T>(
  path: string,
  init: RequestInit = {},
  schema?: z.ZodType<T>
): Promise<T> {
  const cookieStore = await cookies();
  const cookieHeader = cookieStore
    .getAll()
    .map(cookie => `${cookie.name}=${cookie.value}`)
    .join('; ');
  const headers = new Headers(init.headers);
  if (cookieHeader) headers.set('cookie', cookieHeader);
  if (init.body && !headers.has('content-type')) {
    headers.set('content-type', 'application/json');
  }

  const result = await apiFetchCore<T>({
    url: rustApiUrl(path),
    init: {
      ...init,
      headers,
      cache: init.cache ?? 'no-store',
    },
    path,
    schema,
  });
  return result.data;
}

export async function getCurrentUser() {
  const headerStore = await headers();
  const headerUser = decodeAuthUserHeader<CurrentUserResponse>(
    headerStore.get(AUTH_USER_HEADER)
  );
  if (headerUser) return headerUser;

  if (headerStore.get(AUTH_CHECKED_HEADER) === AUTH_CHECKED_VALUE) {
    return null;
  }

  const cookieStore = await cookies();
  if (!cookieStore.has(AUTH_SESSION_COOKIE_NAME)) {
    return null;
  }

  try {
    return await serverApi('/api/auth/me', {}, CurrentUserResponseSchema);
  } catch {
    return null;
  }
}
