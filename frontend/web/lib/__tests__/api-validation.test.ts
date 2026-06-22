import { describe, expect, it } from 'vitest';
import { z } from 'zod';
import { validateApiData } from '../api/validation';

describe('validateApiData', () => {
  it('returns parsed API data when it matches the schema', () => {
    const schema = z.object({ id: z.string(), count: z.number() });

    expect(
      validateApiData({ id: 'item-1', count: 2 }, schema, '/api/test')
    ).toEqual({
      id: 'item-1',
      count: 2,
    });
  });

  it('throws a path-specific error when the response shape drifts', () => {
    const schema = z.object({ data: z.object({ id: z.string() }) });

    expect(() => validateApiData({ data: {} }, schema, '/api/test')).toThrow(
      'Invalid API response for /api/test: data.id'
    );
  });
});
