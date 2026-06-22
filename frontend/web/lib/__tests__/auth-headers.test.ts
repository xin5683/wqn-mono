import { describe, expect, it } from 'vitest';
import {
  decodeAuthUserHeader,
  encodeAuthUserHeader,
} from '../api/auth-headers';

describe('auth header helpers', () => {
  it('round-trips an authenticated user payload', () => {
    const user = {
      id: 'user-1',
      email: 'student@example.com',
      profile: { username: 'student', display_name: 'Student One' },
    };

    expect(decodeAuthUserHeader(encodeAuthUserHeader(user))).toEqual(user);
  });

  it('returns null for missing or malformed values', () => {
    expect(decodeAuthUserHeader(null)).toBeNull();
    expect(decodeAuthUserHeader('%E0%A4%A')).toBeNull();
    expect(decodeAuthUserHeader('not-json')).toBeNull();
  });
});
