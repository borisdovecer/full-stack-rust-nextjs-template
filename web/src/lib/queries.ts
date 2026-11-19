"use client";

import { type QueryClient, useQuery } from "@tanstack/react-query";

import { api, call, ApiError, type Schemas } from "@/lib/api/client";

export type Me = Schemas["Me"];
export type Permission = Schemas["Permission"];

/** Query keys in one place, so invalidation cannot miss one. */
export const keys = {
  me: ["me"] as const,
  config: ["config"] as const,
  sessions: ["me", "sessions"] as const,
  users: (params: object) => ["users", params] as const,
  user: (id: string) => ["users", "detail", id] as const,
  help: ["help"] as const,
  helpPage: (slug: string) => ["help", slug] as const,
};

export function useMe() {
  return useQuery({
    queryKey: keys.me,
    queryFn: () => call(api.GET("/api/me")),
    staleTime: 30_000,
  });
}

export function usePublicConfig() {
  return useQuery({
    queryKey: keys.config,
    queryFn: () => call(api.GET("/api/config")),
    staleTime: Infinity,
  });
}

/** Retry only what might pass on a second try: never a 4xx. */
export function shouldRetry(failureCount: number, error: unknown): boolean {
  if (error instanceof ApiError && error.status >= 400 && error.status < 500) return false;
  return failureCount < 2;
}

/** Forget everything about the signed-in user (after logout or a 401). */
export function clearUserData(client: QueryClient) {
  client.removeQueries({ predicate: (q) => q.queryKey[0] !== "config" });
}
