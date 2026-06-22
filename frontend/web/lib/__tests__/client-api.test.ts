import { afterEach, describe, expect, it, vi } from 'vitest';
import { z } from 'zod';
import { clientApi, ClientApiError, clientApiResult } from '../api/client';

afterEach(() => {
  vi.restoreAllMocks();
});

describe('clientApi', () => {
  it('unwraps Rust API data envelopes and sends credentials by default', async () => {
    const fetchMock = vi
      .spyOn(globalThis, 'fetch')
      .mockResolvedValue(
        new Response(JSON.stringify({ success: true, data: { id: 's1' } }))
      );

    await expect(clientApi<{ id: string }>('/api/subjects')).resolves.toEqual({
      id: 's1',
    });

    expect(fetchMock).toHaveBeenCalledWith(
      'http://localhost:3000/api/subjects',
      expect.objectContaining({ credentials: 'include' })
    );
  });

  it('serializes JSON request bodies and validates response data when a schema is provided', async () => {
    const fetchMock = vi
      .spyOn(globalThis, 'fetch')
      .mockResolvedValue(
        new Response(JSON.stringify({ success: true, data: { count: 2 } }))
      );
    const schema = z.object({ count: z.number() });

    await expect(
      clientApi('/api/test', { method: 'POST', body: { ok: true } }, schema)
    ).resolves.toEqual({ count: 2 });

    expect(fetchMock).toHaveBeenCalledWith(
      'http://localhost:3000/api/test',
      expect.objectContaining({
        method: 'POST',
        body: JSON.stringify({ ok: true }),
      })
    );
  });

  it('can return response status alongside unwrapped data', async () => {
    vi.spyOn(globalThis, 'fetch').mockResolvedValue(
      new Response(JSON.stringify({ success: true, data: { queued: true } }), {
        status: 202,
        statusText: 'Accepted',
      })
    );

    await expect(clientApiResult('/api/test')).resolves.toMatchObject({
      data: { queued: true },
      status: 202,
      statusText: 'Accepted',
    });
  });

  it('throws a typed error with the API error message and response body', async () => {
    vi.spyOn(globalThis, 'fetch').mockResolvedValue(
      new Response(JSON.stringify({ error: 'Quota exceeded' }), {
        status: 403,
        statusText: 'Forbidden',
      })
    );

    await expect(clientApi('/api/test')).rejects.toMatchObject({
      name: 'ClientApiError',
      message: 'Quota exceeded',
      status: 403,
      body: { error: 'Quota exceeded' },
    } satisfies Partial<ClientApiError>);
  });
});
