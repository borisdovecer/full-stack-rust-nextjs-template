import { describe, expect, it } from "vitest";

import { ApiError } from "@/lib/api/client";
import type { Me } from "@/lib/queries";

import { gate, safeNext } from "./gate";

function me(status: "active" | "pending", permissions: Me["permissions"] = []): Me {
  return {
    user: {
      id: "u1",
      email: "a@example.com",
      first_name: "A",
      last_name: "B",
      login_type: "password",
      role: "user",
      status,
      created_at: "2030-01-01T00:00:00Z",
      last_login_at: null,
    },
    permissions,
    preferences: { theme: "system" },
  };
}

const q = (data?: Me, error: unknown = null) => ({ data, error, isPending: !data && !error });

describe("gate", () => {
  it("waits while loading", () => {
    expect(gate(q(), { pathname: "/dashboard" })).toEqual({ kind: "loading" });
  });

  it("sends a signed-out visitor to login and remembers where they were going", () => {
    const err = new ApiError("unauthenticated", 401, null);
    expect(gate(q(undefined, err), { pathname: "/users/1" })).toEqual({
      kind: "redirect",
      to: "/login?next=%2Fusers%2F1",
    });
  });

  it("shows other errors", () => {
    const err = new ApiError("network", 0, null);
    expect(gate(q(undefined, err), { pathname: "/dashboard" }).kind).toBe("error");
  });

  it("keeps pending users on /pending and active users off it", () => {
    expect(gate(q(me("pending")), { pathname: "/dashboard" })).toEqual({
      kind: "redirect",
      to: "/pending",
    });
    expect(gate(q(me("pending")), { pathname: "/pending", allowPending: true }).kind).toBe("ok");
    expect(gate(q(me("active")), { pathname: "/pending", allowPending: true })).toEqual({
      kind: "redirect",
      to: "/dashboard",
    });
  });

  it("checks the permission", () => {
    const opts = { pathname: "/users/1", permission: "users.read" as const };
    expect(gate(q(me("active")), opts).kind).toBe("forbidden");
    expect(gate(q(me("active", ["users.read"])), opts).kind).toBe("ok");
  });
});

describe("safeNext", () => {
  it("allows only local paths", () => {
    expect(safeNext("/users/1")).toBe("/users/1");
    for (const bad of [null, "", "https://evil.example", "//evil.example", "/\\evil.example"]) {
      expect(safeNext(bad)).toBe("/dashboard");
    }
  });
});
