import { describe, expect, it } from "vitest";
import { z } from "zod";

import {
  BIO_MAX,
  bioHint,
  bioSchema,
  emailSchema,
  fieldErrors,
  nameSchema,
  passwordSchema,
} from "./validation";

const form = z.object({ email: emailSchema, name: nameSchema, password: passwordSchema });

describe("validation", () => {
  it("passes good values", () => {
    expect(fieldErrors(form, { email: " a@b.co ", name: "Ana", password: "a".repeat(12) })).toEqual(
      {},
    );
  });

  it("gives the first problem per field", () => {
    expect(fieldErrors(form, { email: "nope", name: "  ", password: "short" })).toEqual({
      email: "Enter a valid email address.",
      name: "Required.",
      password: "At least 12 characters.",
    });
  });

  it("takes a blank bio and refuses one over the limit", () => {
    expect(bioSchema.safeParse("   ").success).toBe(true);
    expect(bioSchema.safeParse(` ${"ž".repeat(BIO_MAX)} `).success).toBe(true);
    expect(bioSchema.safeParse("a".repeat(BIO_MAX + 1)).success).toBe(false);
    expect(bioHint("  ")).toBe("Optional.");
    expect(bioHint(" Hi ")).toBe(`2 of ${BIO_MAX} characters.`);
  });

  it("counts characters, not bytes, like the API", () => {
    expect(passwordSchema.safeParse("ž".repeat(12)).success).toBe(true);
    expect(passwordSchema.safeParse("a".repeat(129)).success).toBe(false);
  });
});
