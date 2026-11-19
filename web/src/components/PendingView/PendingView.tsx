"use client";

import { useQueryClient } from "@tanstack/react-query";

import { useLogout } from "@/components/UserMenu/UserMenu";
import { Button } from "@/components/ui/button";
import { keys } from "@/lib/queries";

export function PendingView() {
  const logout = useLogout();
  const client = useQueryClient();
  return (
    <div className="grid gap-4">
      <p className="text-muted-foreground">
        You can sign in once an admin approves your account. Check back later.
      </p>
      <div className="flex gap-2">
        <Button
          variant="outline"
          onClick={() => void client.invalidateQueries({ queryKey: keys.me })}
        >
          Check again
        </Button>
        <Button variant="ghost" onClick={() => void logout()}>
          Log out
        </Button>
      </div>
    </div>
  );
}
