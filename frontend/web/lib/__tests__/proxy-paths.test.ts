import { describe, expect, it } from 'vitest';
import { isPublicContentPath } from '../proxy-paths';

describe('isPublicContentPath', () => {
  it('treats the problem sets list and detail pages as public', () => {
    expect(isPublicContentPath('/problem-sets')).toBe(true);
    expect(isPublicContentPath('/problem-sets/abc123')).toBe(true);
  });

  it('does not treat adjacent prefixes as public paths', () => {
    expect(isPublicContentPath('/problem-sets-archive')).toBe(false);
    expect(isPublicContentPath('/authentication')).toBe(false);
  });
});
