import { z } from "zod";

/** The API's rules, checked in the browser first so a form can answer before a round trip. */
export const NAME_MAX = 100;
export const BIO_MAX = 1000;
export const PASSWORD_MIN = 12;
export const PASSWORD_MAX = 128;

const chars = (s: string) => [...s].length;

export const nameSchema = z
  .string()
  .trim()
  .min(1, { error: "Required." })
  .refine((s) => chars(s) <= NAME_MAX, { error: "Too long." });

/** Optional; blank is fine and is saved as no bio. */
export const bioSchema = z
  .string()
  .trim()
  .refine((s) => chars(s) <= BIO_MAX, { error: `At most ${BIO_MAX} characters.` });

export const emailSchema = z
  .string()
  .trim()
  .min(1, { error: "Required." })
  .max(255, { error: "Too long." })
  .pipe(z.email({ error: "Enter a valid email address." }));

export const passwordSchema = z
  .string()
  .refine((s) => chars(s) >= PASSWORD_MIN, { error: `At least ${PASSWORD_MIN} characters.` })
  .refine((s) => chars(s) <= PASSWORD_MAX, { error: `At most ${PASSWORD_MAX} characters.` });

/** The hint under a bio field: optional, and how much room is left. */
export function bioHint(value: string): string {
  const used = chars(value.trim());
  return used ? `${used} of ${BIO_MAX} characters.` : "Optional.";
}

/** The first problem per field, or an empty object when the values pass. */
export function fieldErrors<T extends z.ZodType>(
  schema: T,
  values: unknown,
): Record<string, string> {
  const result = schema.safeParse(values);
  if (result.success) return {};
  const out: Record<string, string> = {};
  for (const issue of result.error.issues) {
    const key = String(issue.path[0] ?? "");
    out[key] ??= issue.message;
  }
  return out;
}
