import Link from "next/link";
import type { ReactNode } from "react";

import { AppName } from "@/components/AppName/AppName";
import { ThemeToggle } from "@/components/ThemeToggle/ThemeToggle";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";

interface Props {
  title: string;
  description?: string;
  children: ReactNode;
  footer?: ReactNode;
}

/** The frame of every signed-out page: app name, theme toggle, one centred card. */
export function AuthCard({ title, description, children, footer }: Props) {
  return (
    <div className="flex min-h-dvh flex-col">
      <header className="flex h-14 items-center justify-between px-4 md:px-5">
        <Link href="/" className="text-base font-semibold">
          <AppName />
        </Link>
        <ThemeToggle />
      </header>
      <main className="flex flex-1 items-start justify-center px-4 pt-8 pb-16 md:items-center md:pt-0">
        <div className="w-full max-w-sm">
          <Card>
            <CardHeader>
              <CardTitle className="text-lg font-semibold">{title}</CardTitle>
              {description ? <CardDescription>{description}</CardDescription> : null}
            </CardHeader>
            <CardContent>{children}</CardContent>
          </Card>
          {footer ? <div className="mt-4 text-center text-muted-foreground">{footer}</div> : null}
        </div>
      </main>
    </div>
  );
}
