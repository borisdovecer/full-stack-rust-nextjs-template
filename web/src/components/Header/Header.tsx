"use client";

import Link from "next/link";

import { AppName } from "@/components/AppName/AppName";
import { ThemeToggle } from "@/components/ThemeToggle/ThemeToggle";
import { UserMenu } from "@/components/UserMenu/UserMenu";

export function Header() {
  return (
    <header className="sticky top-0 z-40 flex h-14 items-center gap-2 border-b bg-background/95 px-4 backdrop-blur md:px-5">
      <Link href="/dashboard" className="mr-auto text-base font-semibold">
        <AppName />
      </Link>
      <ThemeToggle />
      <UserMenu />
    </header>
  );
}
