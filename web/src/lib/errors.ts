import { ApiError, type ClientErrorCode, type ErrorCode } from "@/lib/api/client";

import { formatTime } from "./format";

/**
 * The words a person reads for each API error code. The only place error text is written.
 * The `Record` type makes a new code from the API a compile error until it has words here.
 * Each says what went wrong and, where there is one, what to do.
 */
const WORDS: Record<ErrorCode | ClientErrorCode, string> = {
  invalid_request: "Something in the request was wrong. Reload the page and try again.",
  validation_failed: "Some fields need fixing.",
  unsupported_media_type: "The request was sent in a form the server does not accept.",
  payload_too_large: "That is too much data to send at once.",
  unauthenticated: "Your session has ended. Sign in again.",
  invalid_credentials: "Wrong email or password.",
  invalid_code: "That code is wrong or no longer works. Check it, or send a new one.",
  invalid_signature: "The wallet's signature could not be checked. Sign again.",
  email_not_verified:
    "Google or GitHub has not verified this email. Verify it there, then try again.",
  provider_email_mismatch:
    "That Google or GitHub account has another email than this account. Use the one with the same email.",
  provider_failed: "Signing in with Google or GitHub did not finish. Try again.",
  wrong_password: "Your current password is not right.",
  forbidden: "You do not have permission to do this.",
  account_pending: "Your account is waiting for an admin to approve it.",
  account_suspended: "This account is suspended. Contact an admin.",
  registration_closed: "Registration is closed. Ask an admin for an account.",
  not_found: "We could not find that.",
  method_not_allowed: "That action is not available here.",
  email_taken: "An account with this email already exists. Sign in instead.",
  wallet_taken: "Another account signs in with this wallet. Use another wallet.",
  identity_taken: "Another account signs in with that Google or GitHub account. Use another one.",
  last_admin: "There must be at least one active admin.",
  cannot_change_self:
    "You cannot change your own role, status, permissions, sign-in method or password here. Use Settings for your own account.",
  login_method_unavailable: "This sign-in method is not available. Choose another one.",
  login_method_unchanged: "This account already signs in this way.",
  reauth_required: "For your safety, sign out and sign in again, then make this change.",
  too_many_attempts: "Too many attempts. Wait a while and try again.",
  request_timeout: "The server took too long. Try again.",
  internal: "Something went wrong on our side. Try again in a moment.",
  unavailable: "The service is unavailable right now. Try again in a moment.",
  network: "Could not reach the server. Check your connection and try again.",
  wallet_declined: "The wallet did not sign. Try again, and approve the request in the wallet.",
  wallet_mismatch:
    "The wallet signed with another account. Switch to the connected one and try again.",
  wallet_cannot_sign: "This wallet cannot sign messages. Use another wallet.",
  wallet_wrong_address:
    "This page is open at another address than the app is set up for, so wallets will not sign.",
  unknown: "Something went wrong. Try again.",
};

/** Words for one problem with one form field, from `details.fields`. */
const FIELD_WORDS: Record<string, string> = {
  required: "Required.",
  too_long: "Too long.",
  too_short: "Too short.",
  invalid_email: "Enter a valid email address.",
  invalid_address: "This is not a Solana wallet address.",
  invalid: "Not a valid value.",
  unknown_permission: "Unknown permission.",
  wrong_password: "This is not your current password.",
};

export interface DescribedError {
  words: string;
  code: ErrorCode | ClientErrorCode;
  requestId: string | null;
  fields: Record<string, string>;
}

/** An error code a page was sent back with (`?error=`), as an error to show; unknown codes
 * become `unknown`, so a hand-edited URL cannot inject words. */
export function errorFromCode(code: string | null): ApiError | null {
  if (!code) return null;
  const known = Object.prototype.hasOwnProperty.call(WORDS, code);
  return new ApiError(known ? (code as ErrorCode) : "unknown", 0, null);
}

export function describeError(error: unknown): DescribedError {
  const e = error instanceof ApiError ? error : new ApiError("unknown", 0, null);
  let words = WORDS[e.code];
  if (e.code === "wallet_wrong_address" && typeof e.details?.expected === "string") {
    words = `Wallets sign only for the app's own address. Open the app at ${e.details.expected} and try again.`;
  }
  if (e.code === "too_many_attempts" && typeof e.details?.retry_at === "string") {
    words = `Too many attempts. Try again after ${formatTime(e.details.retry_at)}.`;
  }
  const fields: Record<string, string> = {};
  for (const [field, problems] of Object.entries(e.fields)) {
    const first = problems[0];
    if (first) fields[field] = FIELD_WORDS[first] ?? "Not valid.";
  }
  return { words, code: e.code, requestId: e.requestId, fields };
}

export const ERROR_CODES = Object.keys(WORDS) as (ErrorCode | ClientErrorCode)[];
