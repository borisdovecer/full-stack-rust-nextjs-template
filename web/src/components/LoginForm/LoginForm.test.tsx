import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { ME, apiError, mockApi, renderWithClient } from "@/test/render";

import { LoginForm } from "./LoginForm";

const ADDRESS = "4Nd1mBQtrMJVYVfKf2PJy9NZUZdTAsp7D4xWLs4gDB4T";
vi.mock("@solana/wallet-adapter-react", () => ({
  WalletProvider: ({ children }: { children: React.ReactNode }) => children,
  useWallet: () => ({
    wallets: [],
    publicKey: { toBase58: () => ADDRESS },
    wallet: { adapter: { name: "Phantom" } },
    connected: true,
    connecting: false,
    connect: vi.fn(),
    disconnect: vi.fn(),
    select: vi.fn(),
    signIn: () =>
      Promise.resolve({
        account: { address: ADDRESS },
        signedMessage: new Uint8Array([104, 105]),
        signature: new Uint8Array([1]),
      }),
  }),
}));

const replace = vi.fn();
vi.mock("next/navigation", () => ({
  useRouter: () => ({ replace, push: vi.fn() }),
}));

beforeEach(() => replace.mockReset());
afterEach(() => vi.unstubAllGlobals());

async function fill(email: string, password: string) {
  await userEvent.type(screen.getByLabelText("Email"), email);
  await userEvent.type(screen.getByLabelText("Password"), password);
  await userEvent.click(screen.getByRole("button", { name: "Log in" }));
}

describe("LoginForm", () => {
  it("asks for both fields before sending", async () => {
    const { fetchMock } = mockApi({
      "GET /api/config": { app_name: "App", registration_mode: "open", version: "1" },
    });
    renderWithClient(<LoginForm />);
    await userEvent.click(screen.getByRole("button", { name: "Log in" }));
    expect(screen.getAllByText("Required.")).toHaveLength(2);
    expect(
      fetchMock.mock.calls.filter(([r]) => String((r as Request).url).includes("login")),
    ).toHaveLength(0);
  });

  it("signs in and goes where the person was going", async () => {
    const api = mockApi({
      "GET /api/config": { app_name: "App", registration_mode: "open", version: "1" },
      "POST /api/auth/login": ME,
    });
    window.history.replaceState(null, "", "/login?next=/users/9");
    renderWithClient(<LoginForm />);
    await fill("ana@example.com", "a-long-password");
    await waitFor(() => expect(replace).toHaveBeenCalledWith("/users/9"));
    const login = api.calls.find((r) => r.url.endsWith("/api/auth/login"));
    expect(login?.headers.get("content-type")).toBe("application/json");
    expect(await login?.clone().json()).toEqual({
      method: "password",
      email: "ana@example.com",
      password: "a-long-password",
    });
    // With one method there is nothing to choose between.
    expect(screen.queryByRole("tablist")).toBeNull();
  });

  it("sends a code, then signs in with it", async () => {
    const api = mockApi({
      "GET /api/config": {
        app_name: "App",
        registration_mode: "open",
        login_methods: ["password", "code"],
        admin_login_methods: ["password", "code"],
        version: "1",
      },
      "POST /api/auth/code": () => new Response(null, { status: 204 }),
      "POST /api/auth/login": ME,
    });
    window.history.replaceState(null, "", "/login");
    renderWithClient(<LoginForm />);
    await userEvent.click(await screen.findByRole("tab", { name: "Email code" }));
    await userEvent.type(screen.getByLabelText("Email"), "ana@example.com");
    await userEvent.click(screen.getByRole("button", { name: "Send code" }));
    await userEvent.type(await screen.findByLabelText("Code"), " 123456 ");
    await userEvent.click(screen.getByRole("button", { name: "Log in" }));
    await waitFor(() => expect(replace).toHaveBeenCalledWith("/dashboard"));

    const ask = api.calls.find((r) => r.url.endsWith("/api/auth/code"));
    expect(await ask?.clone().json()).toEqual({ email: "ana@example.com", purpose: "login" });
    const login = api.calls.find((r) => r.url.endsWith("/api/auth/login"));
    expect(await login?.clone().json()).toEqual({
      method: "code",
      email: "ana@example.com",
      login_code: "123456",
    });
  });

  it("signs in with the account's wallet", async () => {
    const api = mockApi({
      "GET /api/config": {
        app_name: "App",
        registration_mode: "open",
        login_methods: ["password", "code", "wallet"],
        admin_login_methods: ["password", "code"],
        version: "1",
      },
      "POST /api/auth/wallet/nonce": {
        domain: "localhost:3000",
        address: ADDRESS,
        statement: "Sign in.",
        nonce: "Nonce1234567890ab",
        issued_at: "2030-01-01T12:00:00Z",
        expiration_time: "2030-01-01T12:05:00Z",
      },
      "POST /api/auth/login": ME,
    });
    window.history.replaceState(null, "", "/login");
    renderWithClient(<LoginForm />);
    await userEvent.click(await screen.findByRole("tab", { name: "Wallet" }));
    await userEvent.type(screen.getByLabelText("Email"), "ana@example.com");
    await userEvent.click(await screen.findByRole("button", { name: "Sign in with this wallet" }));
    await waitFor(() => expect(replace).toHaveBeenCalledWith("/dashboard"));
    const login = api.calls.find((r) => r.url.endsWith("/api/auth/login"));
    expect(await login?.clone().json()).toEqual({
      method: "wallet",
      email: "ana@example.com",
      address: ADDRESS,
      message: btoa("hi"),
      signature: btoa(String.fromCharCode(1)),
    });
  });

  it("checks the email before asking the wallet", async () => {
    const api = mockApi({
      "GET /api/config": {
        app_name: "App",
        registration_mode: "open",
        login_methods: ["password", "wallet"],
        admin_login_methods: ["password"],
        version: "1",
      },
    });
    renderWithClient(<LoginForm />);
    await userEvent.click(await screen.findByRole("tab", { name: "Wallet" }));
    await userEvent.click(await screen.findByRole("button", { name: "Sign in with this wallet" }));
    expect(await screen.findByText("Required.")).toBeInTheDocument();
    expect(api.calls.some((r) => r.url.endsWith("/api/auth/wallet/nonce"))).toBe(false);
  });

  it("offers the configured providers and shows why one sent the person back", async () => {
    mockApi({
      "GET /api/config": {
        app_name: "App",
        registration_mode: "open",
        login_methods: ["password", "external"],
        admin_login_methods: ["password"],
        oauth_providers: ["github"],
        version: "1",
      },
    });
    window.history.replaceState(null, "", "/login?error=email_not_verified");
    renderWithClient(<LoginForm />);
    expect(await screen.findByText(/has not verified this email/)).toBeInTheDocument();
    await userEvent.click(await screen.findByRole("tab", { name: "Google or GitHub" }));
    expect(screen.getByRole("button", { name: "Continue with GitHub" })).toHaveAttribute(
      "href",
      "/api/auth/oauth/github/start?intent=login",
    );
    expect(screen.queryByRole("button", { name: "Continue with Google" })).toBeNull();
    window.history.replaceState(null, "", "/login?error=<script>");
    window.dispatchEvent(new PopStateEvent("popstate"));
    expect(await screen.findByText("Something went wrong. Try again.")).toBeInTheDocument();
    window.history.replaceState(null, "", "/login");
  });

  it("shows the API's words for a wrong password", async () => {
    mockApi({
      "GET /api/config": { app_name: "App", registration_mode: "closed", version: "1" },
      "POST /api/auth/login": () => apiError("invalid_credentials", 401),
    });
    renderWithClient(<LoginForm />);
    await fill("ana@example.com", "wrong-password");
    expect(await screen.findByText("Wrong email or password.")).toBeInTheDocument();
    expect(screen.queryByRole("link", { name: "Register" })).toBeNull();
    expect(replace).not.toHaveBeenCalled();
  });
});
