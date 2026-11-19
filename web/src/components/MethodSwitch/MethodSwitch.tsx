"use client";

import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import type { Schemas } from "@/lib/api/client";
import { LOGIN_TYPE_LABEL } from "@/lib/labels";
import { cn } from "@/lib/utils";

type LoginType = Schemas["LoginType"];

/** Picks a sign-in method. Draws nothing when there is only one to pick. */
export function MethodSwitch({
  methods,
  value,
  onChange,
  label = "Sign-in method",
}: {
  methods: LoginType[];
  value: LoginType;
  onChange: (method: LoginType) => void;
  label?: string;
}) {
  if (methods.length < 2) return null;
  return (
    <Tabs value={value} onValueChange={(v) => onChange(v as LoginType)}>
      <TabsList
        aria-label={label}
        className={cn(
          "grid h-auto w-full grid-cols-2",
          methods.length === 3 && "sm:grid-cols-3",
          methods.length >= 4 && "sm:grid-cols-4",
        )}
      >
        {methods.map((m) => (
          <TabsTrigger key={m} value={m}>
            {LOGIN_TYPE_LABEL[m]}
          </TabsTrigger>
        ))}
      </TabsList>
    </Tabs>
  );
}
