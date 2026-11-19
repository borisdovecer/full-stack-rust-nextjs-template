import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render } from "@testing-library/react";
import type { ReactElement } from "react";
import { vi } from "vitest";

import { TooltipProvider } from "@/components/ui/tooltip";

/** Renders with a fresh query client that never retries, so errors show at once. */
export function renderWithClient(ui: ReactElement) {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  return {
    client,
    ...render(
      <QueryClientProvider client={client}>
        <TooltipProvider>{ui}</TooltipProvider>
      </QueryClientProvider>,
    ),
  };
}

type Handler = (req: Request) => Response | Promise<Response>;

/** Stubs fetch with one handler per "METHOD /path". Unknown requests answer 404. */
export function mockApi(routes: Record<string, Handler | unknown>) {
  const calls: Request[] = [];
  const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
    const req =
      input instanceof Request ? input : new Request(new URL(String(input), "http://test"), init);
    calls.push(req);
    const key = `${req.method} ${new URL(req.url, "http://test").pathname}`;
    const route = routes[key];
    if (route === undefined)
      return json({ error: { code: "not_found", message: "", request_id: null } }, 404);
    return typeof route === "function" ? (route as Handler)(req) : json(route);
  });
  vi.stubGlobal("fetch", fetchMock);
  return { calls, fetchMock };
}

export function json(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "content-type": "application/json" },
  });
}

export function apiError(code: string, status: number, details?: object): Response {
  return json({ error: { code, message: code, request_id: "req-1", details } }, status);
}

export const ME = {
  user: {
    id: "u1",
    email: "ana@example.com",
    first_name: "Ana",
    last_name: "Example",
    bio: null,
    login_type: "password",
    wallet: null,
    role: "admin",
    status: "active",
    created_at: "2030-01-01T00:00:00Z",
    last_login_at: null,
  },
  permissions: ["users.read", "users.manage"],
  preferences: { theme: "system" },
} as const;
