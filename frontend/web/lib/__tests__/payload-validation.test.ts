import { describe, expect, it } from 'vitest';
import { z } from 'zod';
import { validatePayload } from '../validation/payload';
import { UpdateProblemSetDto } from '../validation/schemas';

describe('validatePayload', () => {
  it('returns parsed payload data when it matches the schema', () => {
    const schema = z.object({ id: z.string(), count: z.number() });

    expect(validatePayload({ id: 'item-1', count: 2 }, schema, 'test')).toEqual(
      {
        id: 'item-1',
        count: 2,
      }
    );
  });

  it('throws a path-specific error when the payload is invalid', () => {
    const schema = z.object({ data: z.object({ id: z.uuid() }) });

    expect(() => validatePayload({ data: {} }, schema, 'test')).toThrow(
      'Invalid test payload: data.id'
    );
  });

  it('does not add create defaults to update problem set payloads', () => {
    expect(UpdateProblemSetDto.parse({ name: 'Review set' })).toEqual({
      name: 'Review set',
    });
  });
});
