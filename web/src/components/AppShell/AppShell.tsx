import type { ReactNode } from "react";

import { BottomNav } from "@/components/BottomNav/BottomNav";
import { Header } from "@/components/Header/Header";
import { Sidebar } from "@/components/Sidebar/Sidebar";
import { ThemeSync } from "@/components/ThemeSync/ThemeSync";

/**
 * The signed-in frame: header on top, sidebar from `md`, bottom nav below it. Both navs are
 * drawn and one is hidden by width, so there is no screen-size check in JavaScript. The page
 * scrolls, never a box inside it.
 */
export function AppShell({ children }: { children: ReactNode }) {
  return (
    <div className="min-h-dvh">
      <ThemeSync />
      <Header />
      <div className="flex">
        <Sidebar className="hidden md:flex" />
        <main className="min-w-0 flex-1 pb-20 md:pb-0">{children}</main>
      </div>
      <BottomNav className="md:hidden" />
    </div>
  );
}
