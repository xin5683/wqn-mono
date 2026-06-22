import { describe, expect, it } from 'vitest';
import {
  ApiError,
  createApiError,
  getApiErrorMessage,
  parseApiBodyText,
  unwrapApiData,
} from '../api/errors';

describe('getApiErrorMessage', () => {
  it('reads the Rust API flat error envelope', () => {
    expect(getApiErrorMessage({ error: 'Subject already exists' }, 'Bad')).toBe(
      'Subject already exists'
    );
  });

  it('keeps compatibility with nested error messages', () => {
    expect(
      getApiErrorMessage({ error: { message: 'Nested error' } }, 'Bad')
    ).toBe('Nested error');
  });

  it('falls back to message and then response text', () => {
    expect(getApiErrorMessage({ message: 'Message field' }, 'Bad')).toBe(
      'Message field'
    );
    expect(getApiErrorMessage('Plain text error', 'Bad')).toBe(
      'Plain text error'
    );
    expect(getApiErrorMessage({}, 'Bad')).toBe('Bad');
  });

  it('parses JSON bodies and leaves non-JSON text intact', () => {
    expect(parseApiBodyText('{"error":"Bad input"}')).toEqual({
      error: 'Bad input',
    });
    expect(parseApiBodyText('plain text')).toBe('plain text');
    expect(parseApiBodyText('')).toBeNull();
  });

  it('unwraps success data envelopes and leaves other bodies intact', () => {
    expect(unwrapApiData({ success: true, data: { id: 'p1' } })).toEqual({
      id: 'p1',
    });
    expect(unwrapApiData({ error: 'Bad input' })).toEqual({
      error: 'Bad input',
    });
  });

  it('creates a structured API error with details', () => {
    const response = new Response(JSON.stringify({}), {
      status: 403,
      statusText: 'Forbidden',
    });
    const error = createApiError({
      response,
      path: '/api/test',
      body: {
        error: 'Quota exceeded',
        details: { resource_type: 'subjects' },
      },
    });

    expect(error).toBeInstanceOf(ApiError);
    expect(error).toMatchObject({
      message: 'Quota exceeded',
      status: 403,
      statusText: 'Forbidden',
      path: '/api/test',
      details: { resource_type: 'subjects' },
    });
  });
});
