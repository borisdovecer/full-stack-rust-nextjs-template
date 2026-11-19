"use client";

import { useQuery } from "@tanstack/react-query";
import type { Route } from "next";
import Link from "next/link";

import { Empty } from "@/components/Empty/Empty";
import { ErrorAlert } from "@/components/ErrorAlert/ErrorAlert";
import { Markdown } from "@/components/Markdown/Markdown";
import { Skeleton } from "@/components/ui/skeleton";
import { ApiError } from "@/lib/api/client";
import { keys } from "@/lib/queries";
import { cn } from "@/lib/utils";

/** The page is markdown, not JSON, so it is fetched directly rather than through `api`. */
async function fetchPage(slug: string): Promise<string> {
  const res = await fetch(`/api/help/${encodeURIComponent(slug)}`, { credentials: "same-origin" });
  if (!res.ok) {
    const body = (await res.json().catch(() => null)) as {
      error?: { code?: string; request_id?: string };
    } | null;
    throw new ApiError(
      (body?.error?.code as ApiError["code"]) ?? "unknown",
      res.status,
      body?.error?.request_id ?? null,
    );
  }
  return res.text();
}

async function fetchIndex() {
  const res = await fetch("/api/help", { credentials: "same-origin" });
  if (!res.ok)
    throw new ApiError(res.status === 401 ? "unauthenticated" : "unknown", res.status, null);
  return (await res.json()) as { slug: string; title: string }[];
}

export function HelpView({ slug }: { slug?: string }) {
  const index = useQuery({ queryKey: keys.help, queryFn: fetchIndex, staleTime: 60_000 });
  const current = slug ?? index.data?.[0]?.slug;
  const page = useQuery({
    queryKey: keys.helpPage(current ?? ""),
    queryFn: () => fetchPage(current ?? ""),
    enabled: Boolean(current),
  });

  return (
    <div className="mx-auto grid w-full max-w-6xl grid-cols-[minmax(0,1fr)] gap-6 px-4 py-4 md:grid-cols-[12rem_minmax(0,1fr)] md:px-5">
      <nav aria-label="Help pages" className="min-w-0 md:sticky md:top-18 md:self-start">
        {index.isPending ? <Skeleton className="h-40 w-full" /> : null}
        {index.error ? <ErrorAlert error={index.error} /> : null}
        <ul className="flex gap-1 overflow-x-auto pb-1 md:flex-col md:overflow-visible">
          {index.data?.map((e) => (
            <li key={e.slug} className="shrink-0">
              <Link
                href={`/help/${e.slug}` as Route}
                aria-current={e.slug === current ? "page" : undefined}
                className={cn(
                  "block rounded-md px-3 py-1.5 text-muted-foreground hover:bg-muted hover:text-foreground",
                  e.slug === current && "bg-muted font-medium text-foreground",
                )}
              >
                {e.title}
              </Link>
            </li>
          ))}
        </ul>
      </nav>
      <article className="min-w-0">
        {index.data?.length === 0 ? (
          <Empty title="No help pages yet" description="Add them to docs/system/README.md." />
        ) : null}
        {page.isPending && current ? (
          <div className="grid gap-3">
            <Skeleton className="h-8 w-64" />
            <Skeleton className="h-4 w-full" />
            <Skeleton className="h-4 w-5/6" />
          </div>
        ) : null}
        {page.error ? (
          page.error instanceof ApiError && page.error.code === "not_found" ? (
            <Empty title="This page does not exist" description="Pick one from the list." />
          ) : (
            <ErrorAlert error={page.error} onRetry={() => void page.refetch()} />
          )
        ) : null}
        {page.data ? <Markdown>{page.data}</Markdown> : null}
      </article>
    </div>
  );
}
