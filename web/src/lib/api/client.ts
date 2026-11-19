import createClient from "openapi-fetch";

import type { components, paths } from "./schema";

export type Schemas = components["schemas"];
export type ErrorCode = Schemas["ErrorCode"];
/** Codes the client adds: the API could not be reached, or answered something unreadable. */
export type ClientErrorCode =
  | "network"
  | "unknown"
  /** The wallet refused, was closed, or failed to sign. */
  | "wallet_declined"
  /** The wallet signed with another account than the one connected. */
  | "wallet_mismatch"
  /** The connected wallet cannot sign messages. */
  | "wallet_cannot_sign"
  /** The page is open at another address than the API's `wallet.domain`; wallets refuse that.
   * `details.expected` is the address to open. */
  | "wallet_wrong_address";

/** Every failed API call becomes this. Decide on `code`, never on the message. */
export class ApiError extends Error {
  constructor(
    readonly code: ErrorCode | ClientErrorCode,
    readonly status: number,
    readonly requestId: string | null,
    readonly details?: Record<string, unknown>,
  ) {
    super(code);
    this.name = "ApiError";
  }

  /** Problems per field from a `validation_failed` answer. */
  get fields(): Record<string, string[]> {
    const f = this.details?.fields;
    return f && typeof f === "object" ? (f as Record<string, string[]>) : {};
  }
}

/**
 * The typed client. Same origin, so the session cookie goes along. Every request is JSON: the
 * API refuses a write without `Content-Type: application/json` (its CSRF guard).
 */
export const api = createClient<paths>({
  // The page's own origin: the same as a relative URL in a browser, and it lets `Request` be
  // built where relative URLs are not allowed (tests).
  baseUrl: globalThis.location?.origin ?? "",
  credentials: "same-origin",
  // Looked up per call, not captured once, so tests can stub it.
  fetch: (request: Request) => globalThis.fetch(request),
  headers: { "Content-Type": "application/json" },
});

type Result<T> = { data?: T; error?: unknown; response: Response };

/** Awaits an `api.*` call and returns its data, or throws an `ApiError`. */
export async function call<T>(request: Promise<Result<T>>): Promise<T> {
  let result: Result<T>;
  try {
    result = await request;
  } catch {
    throw new ApiError("network", 0, null);
  }
  const { data, error, response } = result;
  if (response.ok) return data as T;

  const body = error as Partial<Schemas["ErrorBody"]> | undefined;
  const detail = body?.error;
  throw new ApiError(
    detail?.code ?? "unknown",
    response.status,
    detail?.request_id ?? response.headers.get("x-request-id"),
    (detail?.details as Record<string, unknown> | undefined) ?? undefined,
  );
}
