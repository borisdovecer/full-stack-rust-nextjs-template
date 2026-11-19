"use client";

import { CircleAlertIcon } from "lucide-react";

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { describeError } from "@/lib/errors";

interface Props {
  error: unknown;
  title?: string;
  onRetry?: () => void;
}

/** An API error in words, with the request id so support can find it in the logs. */
export function ErrorAlert({ error, title, onRetry }: Props) {
  const { words, requestId } = describeError(error);
  return (
    <Alert variant="destructive">
      <CircleAlertIcon />
      <AlertTitle>{title ?? words}</AlertTitle>
      <AlertDescription>
        {title ? <p>{words}</p> : null}
        {requestId ? (
          <p className="text-xs text-muted-foreground">
            Reference: <code className="font-mono">{requestId}</code>
          </p>
        ) : null}
        {onRetry ? (
          <Button variant="outline" size="sm" className="mt-2" onClick={onRetry}>
            Try again
          </Button>
        ) : null}
      </AlertDescription>
    </Alert>
  );
}
