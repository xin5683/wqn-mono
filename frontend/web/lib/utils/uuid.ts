/**
 * UUID generation utilities.
 *
 * `crypto.randomUUID()` is only available in secure contexts (HTTPS or
 * localhost). When the app is served over plain HTTP (e.g. a LAN IP behind
 * Docker), `randomUUID` is `undefined` and callers must fall back to a manual
 * RFC 4122 v4 UUID built from `crypto.getRandomValues()`, which *is* available
 * in non-secure contexts.
 */

/** Regex for a canonical RFC 4122 UUID (any version/variant). */
const UUID_REGEX =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i;

export function isUuid(value: string): boolean {
  return UUID_REGEX.test(value);
}

/**
 * Generate a fresh RFC 4122 v4 UUID in any context (secure or not).
 *
 * 1. Prefers the native `crypto.randomUUID()` (secure contexts only).
 * 2. Falls back to a manual v4 UUID via `crypto.getRandomValues()` (works in
 *    non-secure contexts, e.g. plain HTTP over a LAN IP).
 * 3. Last-resort `Math.random()` fallback if `crypto` is entirely absent
 *    (extremely old runtime).
 */
export function generateUuid(): string {
  const crypto = globalThis.crypto;

  // 1. Native path — secure contexts only.
  if (typeof crypto?.randomUUID === 'function') {
    return crypto.randomUUID();
  }

  // 2. Manual v4 via getRandomValues — works in non-secure contexts.
  if (typeof crypto?.getRandomValues === 'function') {
    const bytes = crypto.getRandomValues(new Uint8Array(16));
    // RFC 4122 §4.4: set version (4) and variant (10xx) bits.
    bytes[6] = (bytes[6] & 0x0f) | 0x40; // version 4
    bytes[8] = (bytes[8] & 0x3f) | 0x80; // variant 10xx → 8/9/a/b
    const hex = Array.from(bytes, b => b.toString(16).padStart(2, '0')).join(
      ''
    );
    return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20, 32)}`;
  }

  // 3. Last-resort fallback (no Crypto object at all).
  return 'xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx'.replace(/[xy]/g, c => {
    const r = (Math.random() * 16) | 0;
    const v = c === 'x' ? r : (r & 0x3) | 0x8;
    return v.toString(16);
  });
}
