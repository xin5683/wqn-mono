export type ApiErrorEnvelope = {
  error?: unknown;
  message?: unknown;
  details?: unknown;
  [key: string]: unknown;
};

type ApiErrorInit = {
  message: string;
  status: number;
  statusText: string;
  body: unknown;
  path: string;
  requestId: string | null;
};

export class ApiError extends Error {
  readonly status: number;
  readonly statusText: string;
  readonly body: unknown;
  readonly path: string;
  readonly details: unknown;
  readonly requestId: string | null;

  constructor({
    message,
    status,
    statusText,
    body,
    path,
    requestId,
  }: ApiErrorInit) {
    super(message);
    this.name = 'ApiError';
    this.status = status;
    this.statusText = statusText;
    this.body = body;
    this.path = path;
    this.requestId = requestId;
    this.details =
      body && typeof body === 'object'
        ? (body as ApiErrorEnvelope).details
        : undefined;
  }
}

function asNonEmptyString(value: unknown): string | null {
  return typeof value === 'string' && value.trim() ? value : null;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return Boolean(value && typeof value === 'object' && !Array.isArray(value));
}

export function asApiErrorEnvelope(
  body: unknown
): ApiErrorEnvelope | null | undefined {
  return isRecord(body) ? (body as ApiErrorEnvelope) : null;
}

export function getApiErrorMessage(body: unknown, fallback: string): string {
  const envelope = asApiErrorEnvelope(body);
  const directError = asNonEmptyString(envelope?.error);
  if (directError) return directError;

  if (envelope?.error && typeof envelope.error === 'object') {
    const nestedError = envelope.error as { message?: unknown };
    const nestedMessage = asNonEmptyString(nestedError.message);
    if (nestedMessage) return nestedMessage;
  }

  const directMessage = asNonEmptyString(envelope?.message);
  if (directMessage) return directMessage;

  const textBody = asNonEmptyString(body);
  return textBody ?? fallback;
}

export function parseApiBodyText(text: string): unknown {
  if (!text) return null;
  try {
    return JSON.parse(text);
  } catch {
    return text;
  }
}

export async function readApiResponseBody(
  response: Response
): Promise<unknown> {
  return parseApiBodyText(await response.text());
}

export function unwrapApiData(body: unknown): unknown {
  if (
    isRecord(body) &&
    body.success === true &&
    Object.prototype.hasOwnProperty.call(body, 'data')
  ) {
    return body.data;
  }

  return body;
}

export function createApiError({
  response,
  body,
  path,
  fallback,
}: {
  response: Response;
  body: unknown;
  path: string;
  fallback?: string;
}): ApiError {
  return new ApiError({
    message: getApiErrorMessage(
      body,
      fallback ||
        response.statusText ||
        `API request failed: ${response.status}`
    ),
    status: response.status,
    statusText: response.statusText,
    body,
    path,
    requestId: response.headers.get('x-request-id'),
  });
}
