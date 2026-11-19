"use client";

import Link from "next/link";

import { AppName } from "@/components/AppName/AppName";
import { ThemeToggle } from "@/components/ThemeToggle/ThemeToggle";
import { Button } from "@/components/ui/button";
import { useMe, usePublicConfig } from "@/lib/queries";

export function Welcome() {
  const { data: me } = useMe();
  const { data: config } = usePublicConfig();
  const canRegister = config?.registration_mode !== "closed";
  return (
    <div className="flex min-h-dvh flex-col">
      <header className="flex h-14 items-center justify-end px-4 md:px-5">
        <ThemeToggle />
      </header>
      <main className="mx-auto flex w-full max-w-2xl flex-1 flex-col items-center justify-center gap-6 px-4 pb-24 text-center">
        <h1 className="text-3xl font-semibold tracking-tight md:text-4xl">
          <AppName />
        </h1>
        <p className="max-w-md text-base text-muted-foreground">
          A starting point for apps on Rust, Next.js and PostgreSQL, with accounts, roles and
          permissions already in place.
        </p>
        <div className="flex flex-wrap justify-center gap-2">
          {me ? (
            <Button nativeButton={false} size="lg" render={<Link href="/dashboard" />}>
              Open the dashboard
            </Button>
          ) : (
            <>
              <Button nativeButton={false} size="lg" render={<Link href="/login" />}>
                Log in
              </Button>
              {canRegister ? (
                <Button
                  nativeButton={false}
                  size="lg"
                  variant="outline"
                  render={<Link href="/register" />}
                >
                  Register
                </Button>
              ) : null}
            </>
          )}
        </div>
      </main>
    </div>
  );
}
