import type { ReactNode } from "react";

import { cn } from "@/lib/utils";

interface Props {
  title: string;
  description?: string;
  icon?: ReactNode;
  action?: ReactNode;
  className?: string;
}

/** What a list or a page shows when there is nothing in it. Never a bare "none". */
export function Empty({ title, description, icon, action, className }: Props) {
  return (
    <div
      className={cn(
        "mx-auto flex max-w-md flex-col items-center gap-2 px-4 py-12 text-center",
        className,
      )}
    >
      {icon ? <div className="text-muted-foreground [&_svg]:size-8">{icon}</div> : null}
      <p className="text-base font-medium">{title}</p>
      {description ? <p className="text-muted-foreground">{description}</p> : null}
      {action ? <div className="mt-2">{action}</div> : null}
    </div>
  );
}
