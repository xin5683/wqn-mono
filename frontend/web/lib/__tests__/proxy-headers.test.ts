import { NextRequest, NextResponse } from 'next/server';
import { describe, it, expect } from 'vitest';
import {
  applyRequestHeaders,
  createAuthRequestHeaders,
} from '../proxy-headers';
import {
  AUTH_CHECKED_HEADER,
  AUTH_CHECKED_VALUE,
  AUTH_USER_HEADER,
  encodeAuthUserHeader,
} from '../api/auth-headers';

/**
 * Builds a response that mimics what next-intl's middleware produces for a
 * locale-prefixed request: it forwards the resolved locale to server
 * components via the `x-next-intl-locale` request header (encoded in
 * `x-middleware-override-headers` + `x-middleware-request-*`) and syncs the
 * `NEXT_LOCALE` cookie.
 */
function simulateNextIntlResponse(locale: string): NextResponse {
  const requestHeaders = new Headers();
  requestHeaders.set('x-next-intl-locale', locale);
  const response = NextResponse.next({ request: { headers: requestHeaders } });
  response.headers.set('Set-Cookie', `NEXT_LOCALE=${locale}; Path=/`);
  return response;
}

function buildRequest(): NextRequest {
  return new NextRequest(new URL('http://localhost/zh-CN/subjects'), {
    headers: { host: 'localhost' },
  });
}

describe('proxy.applyRequestHeaders', () => {
  it('preserves the next-intl locale override when adding auth headers', () => {
    const intlResponse = simulateNextIntlResponse('zh-CN');
    const authHeaders = createAuthRequestHeaders(buildRequest(), {
      is_admin: false,
    });

    const result = applyRequestHeaders(intlResponse, authHeaders);

    const overrideNames = (
      result.headers.get('x-middleware-override-headers') ?? ''
    )
      .split(',')
      .map(n => n.trim())
      .filter(Boolean);

    // The locale header must survive so server-side getLocale() resolves the
    // URL's locale instead of falling back to the default (English).
    expect(overrideNames).toContain('x-next-intl-locale');
    expect(result.headers.get('x-middleware-request-x-next-intl-locale')).toBe(
      'zh-CN'
    );

    // Auth headers are added alongside it.
    expect(overrideNames).toContain(AUTH_CHECKED_HEADER);
    expect(
      result.headers.get(`x-middleware-request-${AUTH_CHECKED_HEADER}`)
    ).toBe(AUTH_CHECKED_VALUE);
  });

  it('keeps the NEXT_LOCALE cookie set by next-intl', () => {
    const intlResponse = simulateNextIntlResponse('zh-CN');
    const authHeaders = createAuthRequestHeaders(buildRequest(), null);

    const result = applyRequestHeaders(intlResponse, authHeaders);

    expect(result.headers.get('Set-Cookie')).toContain('NEXT_LOCALE=zh-CN');
  });

  it('forwards the encoded auth user header when a user is present', () => {
    const user = { is_admin: true };
    const intlResponse = simulateNextIntlResponse('en');
    const authHeaders = createAuthRequestHeaders(buildRequest(), user);

    const result = applyRequestHeaders(intlResponse, authHeaders);

    expect(result.headers.get(`x-middleware-request-${AUTH_USER_HEADER}`)).toBe(
      encodeAuthUserHeader(user)
    );
  });

  it('drops a client-spoofed auth user header when no user is present', () => {
    const request = new NextRequest(
      new URL('http://localhost/zh-CN/subjects'),
      {
        headers: {
          host: 'localhost',
          [AUTH_USER_HEADER]: encodeAuthUserHeader({ is_admin: true }),
        },
      }
    );
    const intlResponse = simulateNextIntlResponse('zh-CN');
    const authHeaders = createAuthRequestHeaders(request, null);

    const result = applyRequestHeaders(intlResponse, authHeaders);

    // The locale must still survive the spoofed-request path.
    expect(result.headers.get('x-middleware-request-x-next-intl-locale')).toBe(
      'zh-CN'
    );
    // And the spoofed user value must not be forwarded.
    expect(
      result.headers.get(`x-middleware-request-${AUTH_USER_HEADER}`)
    ).toBeNull();
  });
});
