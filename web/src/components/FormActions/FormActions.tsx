import type { ReactNode } from "react";

import { cn } from "@/lib/utils";

/** The buttons at the end of a form: the main action last, on the right. */
export function FormActions({ children, className }: { children: ReactNode; className?: string }) {
  return <div className={cn("flex flex-wrap justify-end gap-2 pt-2", className)}>{children}</div>;
}
