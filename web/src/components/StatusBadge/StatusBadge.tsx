import { Badge } from "@/components/ui/badge";
import type { Schemas } from "@/lib/api/client";
import { cn } from "@/lib/utils";

type Value = Schemas["UserStatus"] | Schemas["Role"];

const STYLE: Record<Value, string> = {
  active: "bg-success text-success-foreground",
  pending: "bg-warning text-warning-foreground",
  suspended: "bg-destructive text-destructive-foreground",
  admin: "bg-primary text-primary-foreground",
  user: "border-border bg-transparent text-foreground",
};

const LABEL: Record<Value, string> = {
  active: "Active",
  pending: "Pending",
  suspended: "Suspended",
  admin: "Admin",
  user: "User",
};

/** A user's status or role as a coloured pill. */
export function StatusBadge({ value, className }: { value: Value; className?: string }) {
  return <Badge className={cn(STYLE[value], className)}>{LABEL[value]}</Badge>;
}
