"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";

import { NAV, isActive } from "@/lib/nav";
import { cn } from "@/lib/utils";

export function Sidebar({ className }: { className?: string }) {
  const pathname = usePathname();
  return (
    <nav
      aria-label="Main"
      className={cn(
        "sticky top-14 h-[calc(100dvh-3.5rem)] w-56 shrink-0 flex-col gap-1 border-r bg-sidebar p-3 text-sidebar-foreground",
        className,
      )}
    >
      {NAV.map(({ href, label, Icon }) => {
        const active = isActive(pathname, href);
        return (
          <Link
            key={href}
            href={href}
            aria-current={active ? "page" : undefined}
            className={cn(
              "flex items-center gap-2 rounded-md px-3 py-2 font-medium transition-colors hover:bg-sidebar-accent hover:text-sidebar-accent-foreground [&_svg]:size-4",
              active && "bg-sidebar-accent text-sidebar-accent-foreground",
            )}
          >
            <Icon />
            {label}
          </Link>
        );
      })}
    </nav>
  );
}
