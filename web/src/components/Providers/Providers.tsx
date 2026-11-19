"use client";

import { MutationCache, QueryCache, QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { ThemeProvider } from "next-themes";
import { type ReactNode, useState } from "react";

import { Toaster } from "@/components/ui/sonner";
import { TooltipProvider } from "@/components/ui/tooltip";
import { ApiError } from "@/lib/api/client";
import { keys, shouldRetry } from "@/lib/queries";

function makeClient(): QueryClient {
  const client: QueryClient = new QueryClient({
    defaultOptions: {
      queries: { retry: shouldRetry, refetchOnWindowFocus: true },
      mutations: { retry: false },
    },
    // Any other call answered 401 means the session ended: re-ask /api/me, whose own 401 makes
    // AuthGate send the person to /login. A 401 on /api/me itself is the answer, not a reason
    // to ask again.
    queryCache: new QueryCache({
      onError: (e, query) => {
        if (query.queryKey[0] !== keys.me[0] || query.queryKey.length > 1) {
          onUnauthenticated(client, e);
        }
      },
    }),
    mutationCache: new MutationCache({ onError: (e) => onUnauthenticated(client, e) }),
  });
  return client;
}

function onUnauthenticated(client: QueryClient, error: unknown) {
  if (error instanceof ApiError && error.code === "unauthenticated") {
    void client.invalidateQueries({ queryKey: keys.me, exact: true });
  }
}

export function Providers({ children }: { children: ReactNode }) {
  const [client] = useState(makeClient);
  return (
    <QueryClientProvider client={client}>
      <ThemeProvider attribute="class" defaultTheme="system" enableSystem disableTransitionOnChange>
        <TooltipProvider>
          {children}
          <Toaster />
        </TooltipProvider>
      </ThemeProvider>
    </QueryClientProvider>
  );
}
