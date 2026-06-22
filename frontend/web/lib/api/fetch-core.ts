import type { z } from 'zod';
import { createApiError, readApiResponseBody, unwrapApiData } from './errors';
import { validateApiData } from './validation';

export interface ApiFetchResult<T> {
  data: T;
  status: number;
  statusText: string;
  body: unknown;
}

/**
 * Shared transport core for both `clientApi` (browser fetch via the Next.js
 * rewrite, cookies sent automatically with `credentials: 'include'`) and
 * `serverApi` (Node fetch directly to the Rust backend, cookies forwarded
 * from `next/headers`). Both callers prepare their URL, headers and body,
 * then hand off here so the fetch → read → status-check → unwrap → validate
 * pipeline lives in exactly one place. HTTP failures throw `ApiError` (carrying
 * `x-request-id`); callers that need a distinct error type wrap it.
 */
export async function apiFetchCore<T>(opts: {
  url: string;
  init: RequestInit;
  path: string;
  schema?: z.ZodType<T>;
}): Promise<ApiFetchResult<T>> {
  const { url, init, path, schema } = opts;
  const response = await fetch(url, init);
  const body = await readApiResponseBody(response);

  if (!response.ok) {
    throw createApiError({ response, body, path });
  }

  const data = unwrapApiData(body);
  return {
    data: schema ? validateApiData(data, schema, path) : (data as T),
    status: response.status,
    statusText: response.statusText,
    body,
  };
}
