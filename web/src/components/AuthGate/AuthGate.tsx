"use client";

import type { Route } from "next";
import { usePathname, useRouter } from "next/navigation";
import { type ReactNode, useEffect } from "react";

import { ErrorAlert } from "@/components/ErrorAlert/ErrorAlert";
import { Empty } from "@/components/Empty/Empty";
import { Skeleton } from "@/components/ui/skeleton";
import { type Permission, useMe } from "@/lib/queries";

import { gate } from "./gate";

interface Props {
  children: ReactNode;
  permission?: Permission;
  allowPending?: boolean;
}

/**
 * Shows its children only to a signed-in user who may see them. This is navigation, not
 * security: the API checks every request on its own.
 */
export function AuthGate({ children, permission, allowPending }: Props) {
  const me = useMe();
  const pathname = usePathname();
  const router = useRouter();
  const result = gate(me, { permission, allowPending, pathname });
  const redirect = result.kind === "redirect" ? result.to : null;

  useEffect(() => {
    if (redirect) router.replace(redirect as Route);
  }, [redirect, router]);

  switch (result.kind) {
    case "ok":
      return children;
    case "forbidden":
      return (
        <Empty
          title="You cannot see this page"
          description="Your account does not have permission for it. Ask an admin if you need it."
        />
      );
    case "error":
      return (
        <div className="mx-auto w-full max-w-3xl p-4">
          <ErrorAlert error={result.error} onRetry={() => void me.refetch()} />
        </div>
      );
    default:
      return <PageSkeleton />;
  }
}

export function PageSkeleton() {
  return (
    <div className="mx-auto w-full max-w-5xl space-y-4 p-4 md:p-6" aria-busy="true">
      <Skeleton className="h-8 w-48" />
      <Skeleton className="h-4 w-80 max-w-full" />
      <Skeleton className="h-40 w-full" />
    </div>
  );
}
