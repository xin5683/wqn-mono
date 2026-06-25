import createNextIntlMiddleware from 'next-intl/middleware';
import { routing } from './i18n/routing';
import { NextResponse, NextRequest } from 'next/server';
import { isPublicContentPath } from './lib/proxy-paths';
import { AUTH_SESSION_COOKIE_NAME } from './lib/api/auth-headers';
import { withAuthState } from './lib/proxy-headers';
import { readApiResponseBody, unwrapApiData } from './lib/api/errors';
import { rustApiUrl } from './lib/api/url';

const intlMiddleware = createNextIntlMiddleware(routing);

type ProxyAuthUser = {
  is_admin?: boolean;
};

function stripLocaleFromPath(pathname: string): string {
  for (const locale of routing.locales) {
    const prefix = `/${locale}`;
    if (pathname === prefix || pathname.startsWith(prefix + '/')) {
      const stripped = pathname.slice(prefix.length);
      return stripped || '/';
    }
  }
  return pathname;
}

async function getUserFromRequest(
  request: NextRequest
): Promise<ProxyAuthUser | null> {
  try {
    const response = await fetch(rustApiUrl('/api/auth/me'), {
      headers: {
        cookie: request.headers.get('cookie') || '',
      },
      cache: 'no-store',
    });
    if (!response.ok) return null;
    const body = await readApiResponseBody(response);
    if (
      body &&
      typeof body === 'object' &&
      'success' in body &&
      (body as { success?: unknown }).success !== true
    ) {
      return null;
    }
    return unwrapApiData(body) as ProxyAuthUser;
  } catch {
    return null;
  }
}

function hasSessionCookie(request: NextRequest): boolean {
  return Boolean(request.cookies.get(AUTH_SESSION_COOKIE_NAME)?.value);
}

export async function proxy(request: NextRequest) {
  const originalPathname = request.nextUrl.pathname;

  if (originalPathname.startsWith('/api/')) {
    return NextResponse.next();
  }

  // Digital Asset Links (TWA) and other well-known files must be served as
  // static assets directly — Google's TWA verifier fetches
  // /.well-known/assetlinks.json and must receive the JSON, not a locale
  // redirect or an auth redirect to /login. Short-circuit before intl + auth.
  if (originalPathname.startsWith('/.well-known/')) {
    return NextResponse.next();
  }

  let locale = routing.defaultLocale;
  const intlResponse = await intlMiddleware(request);

  if (intlResponse.status === 307 || intlResponse.status === 308) {
    return intlResponse;
  }

  const localeHeader = intlResponse.headers.get('x-next-intl-locale');
  if (localeHeader && (localeHeader === 'en' || localeHeader === 'zh-CN')) {
    locale = localeHeader;
  } else {
    for (const l of routing.locales) {
      if (originalPathname.startsWith(`/${l}`)) {
        locale = l;
        break;
      }
    }
  }

  const contentPath = stripLocaleFromPath(originalPathname);
  const isPublicPath = isPublicContentPath(contentPath);
  const user = hasSessionCookie(request)
    ? await getUserFromRequest(request)
    : null;

  if (contentPath.startsWith('/admin')) {
    if (!user) {
      return NextResponse.redirect(
        new URL(`/${locale}/auth/login`, request.url)
      );
    }

    if (!user.is_admin) {
      return NextResponse.redirect(new URL(`/${locale}/subjects`, request.url));
    }

    return withAuthState(intlResponse, request, user);
  }

  if (isPublicPath) {
    return withAuthState(intlResponse, request, user);
  }

  if (!user) {
    const loginUrl = new URL(`/${locale}/auth/login`, request.url);
    if (contentPath !== '/') {
      loginUrl.searchParams.set('redirect', contentPath);
    }
    return NextResponse.redirect(loginUrl);
  }

  return withAuthState(intlResponse, request, user);
}

export const config = {
  matcher: [
    '/((?!_next/static|_next/image|favicon.ico|.*\\.(?:svg|png|jpg|jpeg|gif|webp|xml|txt)$).*)',
  ],
};
