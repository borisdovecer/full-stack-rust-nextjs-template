import { BookOpenIcon, LayoutDashboardIcon, SettingsIcon } from "lucide-react";
import type { Route } from "next";

/** The signed-in navigation. Sidebar and bottom nav both draw this list. */
export const NAV: { href: Route; label: string; Icon: typeof BookOpenIcon }[] = [
  { href: "/dashboard", label: "Dashboard", Icon: LayoutDashboardIcon },
  { href: "/settings", label: "Settings", Icon: SettingsIcon },
  { href: "/help", label: "Help", Icon: BookOpenIcon },
];

export function isActive(pathname: string, href: string): boolean {
  return pathname === href || pathname.startsWith(`${href}/`);
}
