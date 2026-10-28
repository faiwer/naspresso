import camelcaseKeys from "camelcase-keys";
import snakecaseKeys from "snakecase-keys";
import type { z } from "zod";

/**
 * GET `path` (relative to the API root) and validate the JSON body against `schema`.
 * Throws an `ApiError` subclass on failure.
 */
export const apiGet = async <T extends z.ZodType>(
  path: string,
  schema: T,
  options: ApiRequestOptions = {},
): Promise<z.infer<T>> => {
  const response = await fetchOk(path, options);
  return decodeJson(response, schema);
};

/** Absolute URL of an API endpoint, for places that load it themselves (`<a href>`, `<img src>`). */
export const buildApiUrl = (path: string, query?: ApiQuery): string =>
  API_ROOT + buildPath(path, query);

/**
 * Query-string values. `undefined` is dropped. Keys are camelCase here and
 * snake_cased at the wire boundary, like response bodies.
 */
export type ApiQuery = Record<string, string | number | boolean | undefined>;

export interface ApiRequestOptions {
  signal?: AbortSignal;
  query?: ApiQuery;
}

/** Base class so callers can `catch (e) { if (e instanceof ApiError) ... }`. */
export class ApiError extends Error {
  constructor(message: string, options?: { cause?: unknown }) {
    super(message, options);
    this.name = new.target.name;
  }
}

/** Network-level failure: offline, DNS, fetch threw, etc. */
export class NetworkError extends ApiError {}

/** Server returned a non-2xx response with an `{ error, message }` body. */
export class HttpError extends ApiError {
  readonly status: number;
  /** The server's machine-readable `error` code, like `"not_found"`. `null` if the body was unreadable. */
  readonly code: string | null;

  constructor(status: number, message: string | null, code: string | null) {
    super(message ?? `HTTP ${status}`);
    this.status = status;
    this.code = code;
  }
}

/** Server returned a 2xx but the body didn't match the expected schema. */
export class DecodeError extends ApiError {
  readonly issues: z.core.$ZodIssue[];
  readonly raw: unknown;

  constructor(issues: z.core.$ZodIssue[], raw: unknown) {
    super(`Response did not match schema: ${formatIssues(issues)}`);
    this.issues = issues;
    this.raw = raw;
  }
}

const API_ROOT = "/api";

const fetchOk = async (
  path: string,
  options: ApiRequestOptions,
): Promise<Response> => {
  const url = buildApiUrl(path, options.query);

  let response: Response;
  try {
    response = await fetch(url, { signal: options.signal });
  } catch (cause) {
    if (cause instanceof DOMException && cause.name === "AbortError") {
      throw cause;
    }

    throw new NetworkError(`GET ${url} failed`, { cause });
  }

  if (!response.ok) {
    throw await readErrorBody(response);
  }
  return response;
};

const buildPath = (path: string, query: ApiQuery | undefined): string => {
  if (!query) {
    return path;
  }

  const params = new URLSearchParams();
  for (const [key, value] of Object.entries(snakecaseKeys(query))) {
    if (value !== undefined) {
      params.append(key, String(value));
    }
  }

  const qs = params.toString();
  return qs ? `${path}?${qs}` : path;
};

const readErrorBody = async (response: Response): Promise<HttpError> => {
  let raw: unknown;
  try {
    raw = await response.json();
  } catch {
    return new HttpError(response.status, null, null);
  }

  const body =
    raw && typeof raw === "object"
      ? (raw as { error?: string; message?: string })
      : null;
  return new HttpError(
    response.status,
    body?.message ?? null,
    body?.error ?? null,
  );
};

const decodeJson = async <T extends z.ZodType>(
  response: Response,
  schema: T,
): Promise<z.infer<T>> => {
  let raw: unknown;
  try {
    raw = await response.json();
  } catch (cause) {
    throw new DecodeError([], cause);
  }

  // The wire is snake_case (Rust convention); the rest of the client is camelCase.
  // Converting at this single boundary keeps snake_case keys out of schemas and components.
  const body =
    raw && typeof raw === "object"
      ? camelcaseKeys(raw as Record<string, unknown>, { deep: true })
      : raw;
  const parsed = schema.safeParse(body);
  if (!parsed.success) {
    throw new DecodeError(parsed.error.issues, body);
  }
  return parsed.data;
};

const formatIssues = (issues: z.core.$ZodIssue[]): string =>
  issues
    .map((issue) => `${issue.path.join(".") || "<root>"}: ${issue.message}`)
    .join("; ");
