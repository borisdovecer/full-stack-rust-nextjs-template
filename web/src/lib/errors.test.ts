import { describe, expect, it } from "vitest";

import { ApiError } from "@/lib/api/client";

import { ERROR_CODES, describeError } from "./errors";

describe("describeError", () => {
  it("has words for every code", () => {
    for (const code of ERROR_CODES) {
      const { words } = describeError(new ApiError(code, 400, null));
      expect(words.length).toBeGreaterThan(5);
      expect(words).not.toBe(code);
      expect(words).not.toContain("_");
    }
  });

  it("turns anything that is not an ApiError into unknown", () => {
    expect(describeError(new Error("boom")).code).toBe("unknown");
    expect(describeError(undefined).code).toBe("unknown");
  });

  it("maps field problems to words", () => {
    const e = new ApiError("validation_failed", 422, "r1", {
      fields: { email: ["invalid_email"], name: ["required", "too_long"], x: ["odd"] },
    });
    const d = describeError(e);
    expect(d.requestId).toBe("r1");
    expect(d.fields).toEqual({
      email: "Enter a valid email address.",
      name: "Required.",
      x: "Not valid.",
    });
  });

  it("says when to retry a throttled login", () => {
    const e = new ApiError("too_many_attempts", 429, null, {
      retry_at: "2030-01-01T10:15:00Z",
    });
    expect(describeError(e).words).toMatch(/^Too many attempts\. Try again after .+\.$/);
  });
});
