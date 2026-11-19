"use client";

import { useEffect, useState } from "react";

import { TextField } from "@/components/Field/Field";
import { Button } from "@/components/ui/button";

/** The API sends at most one code per address this often (`login_code.resend_seconds`). */
export const RESEND_SECONDS = 60;

/** The 6-digit code from the email. Phones offer the code from the mail app. Keeps only digits,
 * so a pasted code with spaces or a dash still fits. */
export function CodeField({
  value,
  onChange,
  error,
}: {
  value: string;
  onChange: (e: { target: { value: string } }) => void;
  error?: string;
}) {
  return (
    <TextField
      label="Code"
      hint="Check your email. The code works once, for 10 minutes."
      inputMode="numeric"
      autoComplete="one-time-code"
      value={value}
      onChange={(e) =>
        onChange({ target: { value: e.target.value.replace(/\D/g, "").slice(0, 6) } })
      }
      error={error}
    />
  );
}

/** Sends another code, after a wait that starts when it appears (a code was just sent). */
export function SendAgain({ onSend }: { onSend: () => Promise<void> }) {
  const [wait, setWait] = useState(RESEND_SECONDS);
  const [pending, setPending] = useState(false);

  useEffect(() => {
    if (wait <= 0) return;
    const t = setTimeout(() => setWait((w) => w - 1), 1000);
    return () => clearTimeout(t);
  }, [wait]);

  const send = async () => {
    setPending(true);
    try {
      await onSend();
      setWait(RESEND_SECONDS);
    } finally {
      setPending(false);
    }
  };

  return (
    <Button
      type="button"
      variant="link"
      size="sm"
      className="h-auto justify-self-start p-0"
      disabled={wait > 0 || pending}
      onClick={() => void send()}
    >
      {wait > 0 ? `Send a new code in ${wait} s` : "Send a new code"}
    </Button>
  );
}
