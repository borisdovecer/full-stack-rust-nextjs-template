import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";

import { ME, apiError, json, mockApi, renderWithClient } from "@/test/render";

import { SettingsView, describeAgent } from "./SettingsView";

vi.mock("next/navigation", () => ({ useRouter: () => ({ replace: vi.fn(), push: vi.fn() }) }));
vi.mock("next-themes", () => ({ useTheme: () => ({ theme: "system", setTheme: vi.fn() }) }));
afterEach(() => vi.unstubAllGlobals());

const SESSIONS = [
  {
    id: "s1",
    current: true,
    ip: "127.0.0.1",
    user_agent: "Mozilla/5.0 (X11; Linux x86_64) Chrome/140",
    created_at: "2030-01-01T00:00:00Z",
    last_seen_at: "2030-01-01T00:00:00Z",
    expires_at: "2030-01-08T00:00:00Z",
  },
  {
    id: "s2",
    current: false,
    ip: "10.0.0.2",
    user_agent: "Mozilla/5.0 (iPhone) Safari/1",
    created_at: "2030-01-01T00:00:00Z",
    last_seen_at: "2030-01-01T00:00:00Z",
    expires_at: "2030-01-08T00:00:00Z",
  },
];

describe("SettingsView", () => {
  it("lists sessions and signs one out", async () => {
    let deleted = "";
    mockApi({
      "GET /api/me": ME,
      "GET /api/me/sessions": SESSIONS,
      "DELETE /api/me/sessions/s2": (req: Request) => {
        deleted = req.url;
        return new Response(null, { status: 204 });
      },
    });
    renderWithClient(<SettingsView />);
    expect(await screen.findByText("Chrome on Linux")).toBeInTheDocument();
    expect(screen.getByText("This browser")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Sign out" }));
    await waitFor(() => expect(deleted).toContain("/api/me/sessions/s2"));
  });

  it("saves first name, last name and bio together", async () => {
    let sent: unknown = null;
    mockApi({
      "GET /api/me": ME,
      "GET /api/me/sessions": SESSIONS,
      "PATCH /api/me": async (req: Request) => {
        sent = await req.json();
        return json({ ...ME, user: { ...ME.user, last_name: "Bell", bio: "Hi" } });
      },
    });
    renderWithClient(<SettingsView />);
    const last = await screen.findByLabelText("Last name");
    expect(screen.getByRole("button", { name: "Save" })).toBeDisabled();
    await userEvent.clear(last);
    await userEvent.type(last, "Bell");
    await userEvent.type(screen.getByLabelText("Bio"), "Hi");
    expect(screen.getByText("2 of 1000 characters.")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() => expect(sent).toEqual({ first_name: "Ana", last_name: "Bell", bio: "Hi" }));
  });

  it("puts a blank last name under its field without sending", async () => {
    mockApi({ "GET /api/me": ME, "GET /api/me/sessions": SESSIONS });
    renderWithClient(<SettingsView />);
    await userEvent.clear(await screen.findByLabelText("Last name"));
    await userEvent.click(screen.getByRole("button", { name: "Save" }));
    expect(await screen.findByText("Required.")).toBeInTheDocument();
  });

  it("lets an account with another method switch to a password", async () => {
    const CODE_ME = { ...ME, user: { ...ME.user, login_type: "code" } };
    let sent: unknown = null;
    mockApi({
      "GET /api/me": CODE_ME,
      "GET /api/me/sessions": SESSIONS,
      "GET /api/config": {
        app_name: "App",
        registration_mode: "open",
        login_methods: ["password", "code"],
        admin_login_methods: ["password", "code"],
        version: "1",
      },
      "PUT /api/me/login-method": async (req: Request) => {
        sent = await req.json();
        return json(ME);
      },
    });
    renderWithClient(<SettingsView />);
    expect(
      await screen.findByText("You sign in with a code sent to your email."),
    ).toBeInTheDocument();
    expect(screen.queryByLabelText("Current password")).toBeNull();
    await userEvent.click(screen.getByRole("button", { name: "Use a password instead" }));
    await userEvent.type(await screen.findByLabelText("New password"), "a-brand-new-password");
    await userEvent.click(screen.getByRole("button", { name: "Use a password" }));
    await waitFor(() =>
      expect(sent).toEqual({ method: "password", password: "a-brand-new-password" }),
    );
    // The answer is a password account, so the card turns into the change-password form.
    expect(await screen.findByLabelText("Current password")).toBeInTheDocument();
  });

  it("lets a password account switch to email codes", async () => {
    const CODE_ME = { ...ME, user: { ...ME.user, login_type: "code" } };
    let sent: unknown = null;
    let asked = false;
    mockApi({
      "GET /api/me": ME,
      "GET /api/me/sessions": SESSIONS,
      "GET /api/config": {
        app_name: "App",
        registration_mode: "open",
        login_methods: ["password", "code"],
        admin_login_methods: ["password", "code"],
        version: "1",
      },
      "POST /api/me/login-code": () => {
        asked = true;
        return new Response(null, { status: 204 });
      },
      "PUT /api/me/login-method": async (req: Request) => {
        sent = await req.json();
        return json(CODE_ME);
      },
    });
    renderWithClient(<SettingsView />);
    await userEvent.click(await screen.findByRole("button", { name: "Use email codes instead" }));
    await userEvent.click(screen.getByRole("button", { name: "Send code" }));
    await waitFor(() => expect(asked).toBe(true));
    await userEvent.type(await screen.findByLabelText("Code"), "123456");
    await userEvent.click(screen.getByRole("button", { name: "Use email codes" }));
    await waitFor(() => expect(sent).toEqual({ method: "code", login_code: "123456" }));
    expect(
      await screen.findByText("You sign in with a code sent to your email."),
    ).toBeInTheDocument();
  });

  it("puts a wrong current password under its field", async () => {
    mockApi({
      "GET /api/me": ME,
      "GET /api/me/sessions": SESSIONS,
      "POST /api/me/password": () => apiError("wrong_password", 422),
    });
    renderWithClient(<SettingsView />);
    await userEvent.type(await screen.findByLabelText("Current password"), "not-it-at-all");
    await userEvent.type(screen.getByLabelText("New password"), "a-brand-new-password");
    await userEvent.click(screen.getByRole("button", { name: "Change password" }));
    expect(await screen.findByText("This is not your current password.")).toBeInTheDocument();
  });

  it("shows an error with a retry when sessions fail to load", async () => {
    mockApi({
      "GET /api/me": ME,
      "GET /api/me/sessions": () =>
        json({ error: { code: "internal", message: "", request_id: "r9" } }, 500),
    });
    renderWithClient(<SettingsView />);
    expect(await screen.findByText(/Something went wrong on our side/)).toBeInTheDocument();
    expect(screen.getByText("r9")).toBeInTheDocument();
  });
});

describe("describeAgent", () => {
  it("names common browsers", () => {
    expect(describeAgent(null)).toBe("Unknown browser");
    expect(describeAgent("Mozilla/5.0 (Windows NT 10.0) Chrome/140 Edg/140")).toBe(
      "Edge on Windows",
    );
    expect(describeAgent("curl/8.0")).toBe("curl");
  });
});
