"use client";

import * as Sentry from "@sentry/react";
import { useEffect } from "react";

import { Empty } from "@/components/Empty/Empty";
import { Button } from "@/components/ui/button";

export default function ErrorPage({
  error,
  reset,
}: {
  error: Error & { digest?: string };
  reset: () => void;
}) {
  useEffect(() => {
    Sentry.captureException(error);
  }, [error]);
  return (
    <div className="flex min-h-dvh items-center justify-center">
      <Empty
        title="Something went wrong"
        description="The page hit an error. Try again, and if it keeps happening, reload."
        action={<Button onClick={reset}>Try again</Button>}
      />
    </div>
  );
}
