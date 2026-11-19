"use client";

import { useTheme } from "next-themes";
import { useEffect } from "react";

import { useMe } from "@/lib/queries";

/**
 * Applies the signed-in user's saved theme, so it follows them to another browser. Signed out,
 * the theme this browser remembers stays.
 */
export function ThemeSync() {
  const { data: me } = useMe();
  const { setTheme } = useTheme();
  const saved = me?.preferences.theme;
  useEffect(() => {
    if (saved) setTheme(saved);
  }, [saved, setTheme]);
  return null;
}
