import type { z } from 'zod';
import { ApiError } from './errors';
import { apiUrl } from './url';
import { apiFetchCore } from './fetch-core';

export class ClientApiError extends ApiError {
  constructor(error: ApiError) {
    super(error);
    this.name = 'ClientApiError';
  }
}

export type ClientApiInit = Omit<RequestInit, 'body'> & {
  body?: unknown;
};

export interface ClientApiResult<T> {
  data: T;
  status: number;
  statusText: string;
  body: unknown;
}

export async function clientApiResult<T>(
  path: string,
  init: ClientApiInit = {},
  schema?: z.ZodType<T>
): Promise<ClientApiResult<T>> {
  const { body: requestBody, headers: initHeaders, ...requestInit } = init;
  const headers = new Headers(initHeaders);
  const hasBody = requestBody !== undefined;

  if (hasBody && !headers.has('content-type')) {
    headers.set('content-type', 'application/json');
  }

  try {
    return await apiFetchCore<T>({
      url: apiUrl(path),
      init: {
        ...requestInit,
        credentials: requestInit.credentials ?? 'include',
        headers,
        body: hasBody ? JSON.stringify(requestBody) : undefined,
      },
      path,
      schema,
    });
  } catch (error) {
    // HTTP failures surface as ApiError (with x-request-id); wrap them so
    // callers can `instanceof ClientApiError` and read `.status`. Zod
    // validation failures are plain Errors and pass through unchanged.
    throw error instanceof ApiError ? new ClientApiError(error) : error;
  }
}

export async function clientApi<T>(
  path: string,
  init: ClientApiInit = {},
  schema?: z.ZodType<T>
): Promise<T> {
  return (await clientApiResult(path, init, schema)).data;
}
