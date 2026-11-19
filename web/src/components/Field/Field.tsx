import { type ComponentProps, type ReactNode, useId } from "react";

import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import { cn } from "@/lib/utils";

interface FieldProps {
  label: string;
  /** Shown under the control while there is no error. */
  hint?: string;
  error?: string;
  className?: string;
  /** Receives the ids the control needs to be labelled and described. */
  children: (control: {
    id: string;
    "aria-invalid"?: true;
    "aria-describedby"?: string;
  }) => ReactNode;
}

/** A label, a control, and one line under it: the hint, or the error when there is one. */
export function Field({ label, hint, error, className, children }: FieldProps) {
  const id = useId();
  const messageId = `${id}-message`;
  const message = error ?? hint;
  return (
    <div className={cn("grid gap-1.5", className)}>
      <Label htmlFor={id}>{label}</Label>
      {children({
        id,
        "aria-invalid": error ? true : undefined,
        "aria-describedby": message ? messageId : undefined,
      })}
      {message ? (
        <p
          id={messageId}
          className={cn("text-xs", error ? "text-destructive" : "text-muted-foreground")}
        >
          {message}
        </p>
      ) : null}
    </div>
  );
}

type TextFieldProps = Omit<FieldProps, "children"> &
  Omit<ComponentProps<typeof Input>, "id" | "className">;

/** The common case: a `Field` around an `Input`. */
export function TextField({ label, hint, error, className, ...input }: TextFieldProps) {
  return (
    <Field label={label} hint={hint} error={error} className={className}>
      {(control) => <Input {...control} {...input} />}
    </Field>
  );
}

type TextareaFieldProps = Omit<FieldProps, "children"> &
  Omit<ComponentProps<typeof Textarea>, "id" | "className">;

/** A `Field` around a `Textarea`, for longer text such as a bio. */
export function TextareaField({ label, hint, error, className, ...textarea }: TextareaFieldProps) {
  return (
    <Field label={label} hint={hint} error={error} className={className}>
      {(control) => <Textarea {...control} {...textarea} />}
    </Field>
  );
}
