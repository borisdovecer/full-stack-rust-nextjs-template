"use client";

import { useMutation, useQueryClient } from "@tanstack/react-query";
import { MonitorIcon, MoonIcon, SunIcon } from "lucide-react";
import { useTheme } from "next-themes";

import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { api, call, type Schemas } from "@/lib/api/client";
import { keys } from "@/lib/queries";

type Theme = Schemas["Theme"];

export const THEMES: { value: Theme; label: string; Icon: typeof SunIcon }[] = [
  { value: "system", label: "System", Icon: MonitorIcon },
  { value: "light", label: "Light", Icon: SunIcon },
  { value: "dark", label: "Dark", Icon: MoonIcon },
];

/** Changes the theme at once, and saves it to the account when signed in. */
export function useThemeChoice() {
  const { theme, setTheme } = useTheme();
  const client = useQueryClient();
  const save = useMutation({
    mutationFn: (value: Theme) =>
      call(api.PATCH("/api/me/preferences", { body: { theme: value } })),
    onSuccess: (data) => client.setQueryData(keys.me, data),
  });
  const choose = (value: Theme) => {
    setTheme(value);
    // Saved to the account only when signed in. Reads the cache, so a signed-out page does
    // not ask the API who is signed in.
    if (client.getQueryData(keys.me)) save.mutate(value);
  };
  return { theme: (theme ?? "system") as Theme, choose };
}

export function ThemeToggle() {
  const { theme, choose } = useThemeChoice();
  return (
    <DropdownMenu>
      <DropdownMenuTrigger
        render={<Button variant="ghost" size="icon" aria-label="Change theme" />}
      >
        <SunIcon className="dark:hidden" />
        <MoonIcon className="hidden dark:block" />
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end">
        <DropdownMenuRadioGroup value={theme} onValueChange={(v) => choose(v as Theme)}>
          {THEMES.map(({ value, label, Icon }) => (
            <DropdownMenuRadioItem key={value} value={value}>
              <Icon />
              {label}
            </DropdownMenuRadioItem>
          ))}
        </DropdownMenuRadioGroup>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
