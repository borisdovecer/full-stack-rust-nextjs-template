"use client";

import { useState } from "react";
import type { z } from "zod";

import { describeError } from "@/lib/errors";
import { fieldErrors } from "@/lib/validation";

/**
 * Form state for the common shape: check in the browser, send, show the API's answer. Field
 * problems from either side land in `errors[field]`; anything else in `formError`.
 */
export function useForm<T extends Record<string, string>>(initial: T, schema: z.ZodType) {
  const [values, setValues] = useState<T>(initial);
  const [errors, setErrors] = useState<Record<string, string>>({});
  const [formError, setFormError] = useState<unknown>(null);
  const [pending, setPending] = useState(false);

  const set = (field: keyof T) => (e: { target: { value: string } }) => {
    setValues((v) => ({ ...v, [field]: e.target.value }));
    setErrors((er) => {
      const { [field as string]: _, ...rest } = er;
      return rest;
    });
  };

  const submit = (send: (values: T) => Promise<void>) => async (e?: { preventDefault(): void }) => {
    e?.preventDefault();
    const local = fieldErrors(schema, values);
    setFormError(null);
    if (Object.keys(local).length) {
      setErrors(local);
      return;
    }
    setPending(true);
    try {
      await send(values);
    } catch (err) {
      const d = describeError(err);
      if (Object.keys(d.fields).length) setErrors(d.fields);
      else setFormError(err);
    } finally {
      setPending(false);
    }
  };

  const reset = () => {
    setValues(initial);
    setErrors({});
    setFormError(null);
  };

  return { values, errors, formError, pending, set, submit, reset, setValues };
}
