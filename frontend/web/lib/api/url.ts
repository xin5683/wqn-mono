const DEFAULT_SITE_URL = 'http://localhost:3000';
const DEFAULT_RUST_API_BASE_URL = 'http://127.0.0.1:8080';

function withoutTrailingSlash(url: string): string {
  return url.replace(/\/$/, '');
}

function withoutLeadingSlash(path: string): string {
  return path.startsWith('/') ? path.slice(1) : path;
}

function withLeadingSlash(path: string): string {
  return path.startsWith('/') ? path : `/${path}`;
}

export function getAppApiBaseUrl(): string {
  if (typeof window !== 'undefined') {
    return withoutTrailingSlash(window.location.origin);
  }
  return getCanonicalSiteUrl();
}

export function getCanonicalSiteUrl(): string {
  return withoutTrailingSlash(process.env.SITE_URL || DEFAULT_SITE_URL);
}

export function getRustApiBaseUrl(): string {
  return withoutTrailingSlash(
    process.env.WQN_API_BASE_URL || DEFAULT_RUST_API_BASE_URL
  );
}

/**
 * Build a full API URL
 * Ensures API paths work correctly in both locale and non-locale contexts
 */
export function apiUrl(path: string): string {
  return `${getAppApiBaseUrl()}/${withoutLeadingSlash(path)}`;
}

export function rustApiUrl(path: string): string {
  return `${getRustApiBaseUrl()}${withLeadingSlash(path)}`;
}

export function absoluteSiteUrl(path = '/'): string {
  return `${getCanonicalSiteUrl()}${withLeadingSlash(path)}`;
}
