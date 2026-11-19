import { screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { ME, mockApi, renderWithClient } from "@/test/render";

import { DashboardView } from "./DashboardView";

vi.mock("next/navigation", () => ({ useRouter: () => ({ replace: vi.fn(), push: vi.fn() }) }));
afterEach(() => vi.unstubAllGlobals());

const USERS = {
  items: [{ ...ME.user, id: "u2", first_name: "Bo", email: "bo@example.com", role: "user" }],
  total: 1,
  total_capped: false,
  page: 1,
  per_page: 20,
};

describe("DashboardView", () => {
  it("shows the user list to someone with users.read", async () => {
    mockApi({ "GET /api/me": ME, "GET /api/users": USERS });
    renderWithClient(<DashboardView />);
    expect(await screen.findByText("Bo Example")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Create user" })).toBeInTheDocument();
  });

  it("hides the user list without the permission", async () => {
    mockApi({ "GET /api/me": { ...ME, user: { ...ME.user, role: "user" }, permissions: [] } });
    renderWithClient(<DashboardView />);
    expect(await screen.findByText("Hello, Ana")).toBeInTheDocument();
    expect(screen.queryByRole("heading", { name: "Users" })).toBeNull();
  });

  it("says so when nothing matches", async () => {
    mockApi({ "GET /api/me": ME, "GET /api/users": { ...USERS, items: [], total: 0 } });
    renderWithClient(<DashboardView />);
    expect(await screen.findByText("No users yet")).toBeInTheDocument();
  });
});
