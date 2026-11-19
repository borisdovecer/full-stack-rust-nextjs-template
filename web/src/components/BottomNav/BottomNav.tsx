"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";

import { NAV, isActive } from "@/lib/nav";
import { cn } from "@/lib/utils";

/** The phone navigation. Hidden from `md` up, where the sidebar takes over. */
export function BottomNav({ className }: { className?: string }) {
  const pathname = usePathname();
  return (
    <nav
      aria-label="Main"
      className={cn(
        "fixed inset-x-0 bottom-0 z-40 grid grid-cols-3 border-t bg-background pb-[env(safe-area-inset-bottom)]",
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
              "flex flex-col items-center gap-1 py-2 text-xs text-muted-foreground [&_svg]:size-5",
              active && "text-foreground",
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
