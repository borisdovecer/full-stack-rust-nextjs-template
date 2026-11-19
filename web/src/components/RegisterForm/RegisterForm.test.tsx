import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { ME, json, mockApi, renderWithClient } from "@/test/render";

import { RegisterForm } from "./RegisterForm";

const replace = vi.fn();
vi.mock("next/navigation", () => ({ useRouter: () => ({ replace, push: vi.fn() }) }));

beforeEach(() => replace.mockReset());
afterEach(() => vi.unstubAllGlobals());

const CONFIG = {
  app_name: "App",
  registration_mode: "open",
  login_methods: ["password", "code"],
  admin_login_methods: ["password", "code"],
  version: "1",
};

async function fillProfile() {
  await userEvent.type(await screen.findByLabelText("First name"), "Ana");
  await userEvent.type(screen.getByLabelText("Last name"), "Example");
  await userEvent.type(screen.getByLabelText("Email"), "ana@example.com");
}

describe("RegisterForm", () => {
  it("registers with a password", async () => {
    const api = mockApi({ "GET /api/config": CONFIG, "POST /api/auth/register": ME });
    renderWithClient(<RegisterForm />);
    await fillProfile();
    await userEvent.type(screen.getByLabelText("Password"), "a-long-password");
    await userEvent.click(screen.getByRole("button", { name: "Create account" }));
    await waitFor(() => expect(replace).toHaveBeenCalledWith("/dashboard"));
    const req = api.calls.find((r) => r.url.endsWith("/api/auth/register"));
    expect(await req?.clone().json()).toEqual({
      method: "password",
      first_name: "Ana",
      last_name: "Example",
      bio: null,
      email: "ana@example.com",
      password: "a-long-password",
      login_code: null,
    });
  });

  it("registers with a code sent to the email", async () => {
    const api = mockApi({
      "GET /api/config": CONFIG,
      "POST /api/auth/code": () => new Response(null, { status: 204 }),
      "POST /api/auth/register": ME,
    });
    renderWithClient(<RegisterForm />);
    await userEvent.click(await screen.findByRole("tab", { name: "Email code" }));
    await fillProfile();
    expect(screen.queryByLabelText("Password")).toBeNull();
    await userEvent.click(screen.getByRole("button", { name: "Send code" }));
    await userEvent.type(await screen.findByLabelText("Code"), "654321");
    await userEvent.click(screen.getByRole("button", { name: "Create account" }));
    await waitFor(() => expect(replace).toHaveBeenCalledWith("/dashboard"));

    const ask = api.calls.find((r) => r.url.endsWith("/api/auth/code"));
    expect(await ask?.clone().json()).toEqual({ email: "ana@example.com", purpose: "register" });
    const req = api.calls.find((r) => r.url.endsWith("/api/auth/register"));
    expect(await req?.clone().json()).toMatchObject({
      method: "code",
      email: "ana@example.com",
      login_code: "654321",
      password: null,
    });
  });

  it("finishes an account Google or GitHub vouched for", async () => {
    const api = mockApi({
      "GET /api/config": CONFIG,
      "GET /api/auth/oauth/signup": {
        provider: "google",
        email: "ana@example.com",
        first_name: "Ana",
        last_name: null,
      },
      "POST /api/auth/register": ME,
    });
    window.history.replaceState(null, "", "/register?via=external");
    renderWithClient(<RegisterForm />);
    expect(await screen.findByText(/You sign up with Google/)).toBeInTheDocument();
    expect(screen.getByLabelText("Email")).toHaveValue("ana@example.com");
    expect(screen.getByLabelText("First name")).toHaveValue("Ana");
    await userEvent.click(screen.getByRole("button", { name: "Create account" }));
    expect(await screen.findByText("Required.")).toBeInTheDocument();
    await userEvent.type(screen.getByLabelText("Last name"), "Lee");
    await userEvent.click(screen.getByRole("button", { name: "Create account" }));
    await waitFor(() => expect(replace).toHaveBeenCalledWith("/dashboard"));
    const req = api.calls.find((r) => r.url.endsWith("/api/auth/register"));
    expect(await req?.clone().json()).toEqual({
      method: "external",
      email: "ana@example.com",
      first_name: "Ana",
      last_name: "Lee",
      bio: null,
    });
    window.history.replaceState(null, "", "/register");
  });

  it("says when the provider's answer is gone", async () => {
    mockApi({
      "GET /api/config": CONFIG,
      "GET /api/auth/oauth/signup": () =>
        json({ error: { code: "not_found", message: "", request_id: null } }, 404),
    });
    window.history.replaceState(null, "", "/register?via=external");
    renderWithClient(<RegisterForm />);
    expect(await screen.findByText(/did not finish/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Start again" })).toHaveAttribute(
      "href",
      "/register",
    );
    window.history.replaceState(null, "", "/register");
  });

  it("asks for the names before sending a code", async () => {
    const api = mockApi({ "GET /api/config": CONFIG });
    renderWithClient(<RegisterForm />);
    await userEvent.click(await screen.findByRole("tab", { name: "Email code" }));
    await userEvent.type(screen.getByLabelText("Email"), "ana@example.com");
    await userEvent.click(screen.getByRole("button", { name: "Send code" }));
    expect(await screen.findAllByText("Required.")).toHaveLength(2);
    expect(api.calls.some((r) => r.url.endsWith("/api/auth/code"))).toBe(false);
  });
});
