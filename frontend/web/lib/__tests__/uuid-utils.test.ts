import { afterEach, describe, expect, it, vi } from 'vitest';
import { z } from 'zod';
import { generateUuid, isUuid } from '../utils/uuid';

// `z.uuid()` is the exact validator that rejected the old `rnd-<ts>` fallback
// in CreateProblemDto, so validating against it proves the fix.
const matchesZodUuid = (value: string) => z.uuid().safeParse(value).success;

describe('generateUuid', () => {
  it('returns a valid RFC 4122 UUID', () => {
    const id = generateUuid();
    expect(isUuid(id)).toBe(true);
    expect(matchesZodUuid(id)).toBe(true);
  });

  it('produces unique values across calls', () => {
    const ids = new Set(Array.from({ length: 1000 }, () => generateUuid()));
    expect(ids.size).toBe(1000);
  });

  it('marks the UUID as version 4', () => {
    // The 13th hex char (index 12) must be '4' for a v4 UUID.
    for (let i = 0; i < 100; i++) {
      expect(generateUuid()[14]).toBe('4');
    }
  });

  it('sets the RFC 4122 variant (8/9/a/b) in the 17th hex char', () => {
    for (let i = 0; i < 100; i++) {
      const variantChar = generateUuid()[19];
      expect(['8', '9', 'a', 'b']).toContain(variantChar);
    }
  });

  describe('when crypto.randomUUID is unavailable (non-secure HTTP context)', () => {
    // In a non-secure context the real Crypto object is still present with a
    // working getRandomValues — only randomUUID is removed. We simulate that
    // by shadowing randomUUID with an undefined own property.
    const originalDescriptor = Object.getOwnPropertyDescriptor(
      globalThis.crypto,
      'randomUUID'
    );

    afterEach(() => {
      // Remove the shadowing own property so the prototype method shows
      // through again (restores native randomUUID for other tests).
      delete (globalThis.crypto as unknown as Record<string, unknown>)
        .randomUUID;
      if (originalDescriptor) {
        Object.defineProperty(
          globalThis.crypto,
          'randomUUID',
          originalDescriptor
        );
      }
    });

    it('still produces a valid v4 UUID via getRandomValues', () => {
      Object.defineProperty(globalThis.crypto, 'randomUUID', {
        value: undefined,
        configurable: true,
      });

      const id = generateUuid();
      expect(isUuid(id)).toBe(true);
      expect(matchesZodUuid(id)).toBe(true);
      expect(id[14]).toBe('4');
    });

    it('falls back to Math.random when crypto is entirely absent', () => {
      const originalCrypto = globalThis.crypto;
      Object.defineProperty(globalThis, 'crypto', {
        value: undefined,
        configurable: true,
        writable: true,
      });

      const id = generateUuid();
      expect(isUuid(id)).toBe(true);
      expect(matchesZodUuid(id)).toBe(true);
      expect(id[14]).toBe('4');

      Object.defineProperty(globalThis, 'crypto', {
        value: originalCrypto,
        configurable: true,
        writable: true,
      });
    });
  });

  it('uses native crypto.randomUUID when available', () => {
    const spy = vi
      .spyOn(globalThis.crypto, 'randomUUID')
      .mockReturnValue('550e8400-e29b-41d4-a716-446655440000');

    expect(generateUuid()).toBe('550e8400-e29b-41d4-a716-446655440000');
    expect(spy).toHaveBeenCalledOnce();
    spy.mockRestore();
  });
});

describe('isUuid', () => {
  it('accepts a well-formed UUID', () => {
    expect(isUuid('550e8400-e29b-41d4-a716-446655440000')).toBe(true);
  });

  it('rejects the old broken fallback value', () => {
    expect(isUuid('rnd-1718900000000')).toBe(false);
  });

  it('rejects malformed strings', () => {
    expect(isUuid('not-a-uuid')).toBe(false);
    expect(isUuid('')).toBe(false);
  });
});
