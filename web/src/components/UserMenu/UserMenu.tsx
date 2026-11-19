"use client";

import { useQueryClient } from "@tanstack/react-query";
import { LogOutIcon, SettingsIcon, UserIcon } from "lucide-react";
import Link from "next/link";
import { useRouter } from "next/navigation";

import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { api, call } from "@/lib/api/client";
import { formatName } from "@/lib/format";
import { clearUserData, useMe } from "@/lib/queries";

export function useLogout() {
  const client = useQueryClient();
  const router = useRouter();
  return async () => {
    try {
      await call(api.POST("/api/auth/logout"));
    } catch {
      // Signed out already, or the server is down: either way the browser forgets the user.
    }
    clearUserData(client);
    router.replace("/login");
  };
}

export function UserMenu() {
  const { data: me } = useMe();
  const logout = useLogout();
  if (!me) return null;
  return (
    <DropdownMenu>
      <DropdownMenuTrigger render={<Button variant="ghost" size="icon" aria-label="Account" />}>
        <UserIcon />
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" className="min-w-56">
        <DropdownMenuGroup>
          <DropdownMenuLabel>
            <span className="block truncate font-medium text-foreground">
              {formatName(me.user)}
            </span>
            <span className="block truncate text-xs font-normal">{me.user.email}</span>
          </DropdownMenuLabel>
        </DropdownMenuGroup>
        <DropdownMenuSeparator />
        <DropdownMenuItem render={<Link href="/settings" />}>
          <SettingsIcon />
          Settings
        </DropdownMenuItem>
        <DropdownMenuItem onClick={() => void logout()}>
          <LogOutIcon />
          Log out
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
