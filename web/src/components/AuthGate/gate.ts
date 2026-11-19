import { ApiError } from "@/lib/api/client";
import type { Me, Permission } from "@/lib/queries";

export type GateResult =
  | { kind: "loading" }
  | { kind: "redirect"; to: string }
  | { kind: "forbidden" }
  | { kind: "error"; error: unknown }
  | { kind: "ok"; me: Me };

export interface GateOptions {
  /** The route also serves pending users (only /pending does). */
  allowPending?: boolean;
  permission?: Permission;
  /** Where the person was going, to come back after signing in. */
  pathname: string;
}

/** What a protected page shows, from the `/api/me` query. Pure, so it is easy to test. */
export function gate(
  query: { data?: Me; error: unknown; isPending: boolean },
  opts: GateOptions,
): GateResult {
  const { data: me, error } = query;
  if (error instanceof ApiError && error.code === "unauthenticated") {
    const next =
      opts.pathname && opts.pathname !== "/" ? `?next=${encodeURIComponent(opts.pathname)}` : "";
    return { kind: "redirect", to: `/login${next}` };
  }
  if (!me) return error ? { kind: "error", error } : { kind: "loading" };

  const pending = me.user.status === "pending";
  if (pending && !opts.allowPending) return { kind: "redirect", to: "/pending" };
  if (!pending && opts.allowPending) return { kind: "redirect", to: "/dashboard" };
  if (opts.permission && !me.permissions.includes(opts.permission)) return { kind: "forbidden" };
  return { kind: "ok", me };
}

/** Only a local path may be a `next` target, so a link cannot send someone off-site. */
export function safeNext(next: string | null | undefined): string {
  if (!next || !next.startsWith("/") || next.startsWith("//") || next.startsWith("/\\")) {
    return "/dashboard";
  }
  return next;
}
