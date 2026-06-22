import { NextRequest, NextResponse } from 'next/server';
import {
  AUTH_CHECKED_HEADER,
  AUTH_CHECKED_VALUE,
  AUTH_USER_HEADER,
  encodeAuthUserHeader,
} from './api/auth-headers';

/**
 * Builds the request headers forwarded to server components, carrying the
 * auth state (whether the session was checked, and the resolved user). Any
 * client-supplied `AUTH_USER_HEADER` is stripped first to prevent spoofing.
 */
export function createAuthRequestHeaders(
  request: NextRequest,
  user: unknown | null
) {
  const requestHeaders = new Headers(request.headers);
  requestHeaders.delete(AUTH_USER_HEADER);
  requestHeaders.set(AUTH_CHECKED_HEADER, AUTH_CHECKED_VALUE);

  if (user) {
    requestHeaders.set(AUTH_USER_HEADER, encodeAuthUserHeader(user));
  }

  return requestHeaders;
}

/**
 * Adds the auth header overrides onto `response` (the next-intl middleware
 * response) while MERGING them with the request-header overrides already
 * present instead of replacing them.
 *
 * next-intl forwards the active locale to server components via the
 * `x-next-intl-locale` request header (encoded in `x-middleware-override-headers`
 * + `x-middleware-request-*`). Overwriting that list here would drop the locale,
 * causing `getLocale()` in server components to fall back to the default locale
 * (English) even on locale-prefixed URLs. Merging keeps the locale override and
 * adds the auth overrides alongside it.
 */
export function applyRequestHeaders(response: NextResponse, headers: Headers) {
  const headerOverrideResponse = NextResponse.next({
    request: {
      headers,
    },
  });

  const mergedOverrideNames = new Set<string>();
  const existingOverride = response.headers.get(
    'x-middleware-override-headers'
  );
  if (existingOverride) {
    for (const name of existingOverride.split(',')) {
      const trimmed = name.trim();
      if (trimmed) mergedOverrideNames.add(trimmed);
    }
  }

  headerOverrideResponse.headers.forEach((value, key) => {
    if (key === 'x-middleware-override-headers') {
      for (const name of value.split(',')) {
        const trimmed = name.trim();
        if (trimmed) mergedOverrideNames.add(trimmed);
      }
    } else if (key.startsWith('x-middleware-request-')) {
      response.headers.set(key, value);
    }
  });

  response.headers.set(
    'x-middleware-override-headers',
    Array.from(mergedOverrideNames).join(',')
  );

  return response;
}

/**
 * Convenience: stamp the auth state (derived from `user`) onto the next-intl
 * middleware `response` so server components can read it.
 */
export function withAuthState(
  response: NextResponse,
  request: NextRequest,
  user: unknown | null
) {
  return applyRequestHeaders(response, createAuthRequestHeaders(request, user));
}
