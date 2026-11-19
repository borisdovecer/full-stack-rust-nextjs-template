import { createSignInMessageText } from "@solana/wallet-standard-util";
import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { ApiError } from "@/lib/api/client";
import { describeError } from "@/lib/errors";
import { mockApi, renderWithClient } from "@/test/render";

import { WalletConnect, type WalletProof, toBase64 } from "./WalletConnect";

const ADDRESS = "4Nd1mBQtrMJVYVfKf2PJy9NZUZdTAsp7D4xWLs4gDB4T";
const CHALLENGE = {
  domain: "localhost:3000",
  address: ADDRESS,
  statement: "Sign in to App. This is a signature, not a transaction, and costs nothing.",
  nonce: "Nonce1234567890ab",
  issued_at: "2030-01-01T12:00:00Z",
  expiration_time: "2030-01-01T12:05:00Z",
};

// The adapter's state, set per test.
type Fake = Record<string, unknown>;
let fake: Fake = {};
vi.mock("@solana/wallet-adapter-react", () => ({
  WalletProvider: ({ children }: { children: ReactNode }) => children,
  useWallet: () => fake,
}));

function connected(extra: Fake): Fake {
  return {
    wallets: [],
    publicKey: { toBase58: () => ADDRESS },
    wallet: { adapter: { name: "Phantom" } },
    connected: true,
    connecting: false,
    connect: vi.fn(),
    disconnect: vi.fn(),
    select: vi.fn(),
    ...extra,
  };
}

function SignButton() {
  return (
    <WalletConnect>
      {({ sign }) => (
        <button
          type="button"
          onClick={() =>
            sign().then(
              (p: WalletProof) => (document.title = JSON.stringify(p)),
              (e: unknown) => (document.title = `error:${describeError(e).code}`),
            )
          }
        >
          Sign
        </button>
      )}
    </WalletConnect>
  );
}

beforeEach(() => {
  document.title = "";
});
afterEach(() => vi.unstubAllGlobals());

describe("WalletConnect", () => {
  it("lists the browser's wallets and connects the one picked", async () => {
    const select = vi.fn();
    fake = {
      wallets: [
        { adapter: { name: "Phantom" }, readyState: "Installed" },
        { adapter: { name: "Gone" }, readyState: "NotDetected" },
      ],
      publicKey: null,
      wallet: null,
      connected: false,
      connecting: false,
      connect: vi.fn(),
      disconnect: vi.fn(),
      select,
    };
    renderWithClient(<SignButton />);
    await userEvent.click(screen.getByRole("button", { name: "Connect a wallet" }));
    expect(screen.queryByRole("button", { name: "Gone" })).toBeNull();
    await userEvent.click(screen.getByRole("button", { name: "Phantom" }));
    expect(select).toHaveBeenCalledWith("Phantom");
    expect(screen.queryByRole("button", { name: "Sign" })).toBeNull();
  });

  it("says when the browser has no wallet", async () => {
    fake = { ...connected({}), publicKey: null, connected: false, wallets: [] };
    renderWithClient(<SignButton />);
    await userEvent.click(screen.getByRole("button", { name: "Connect a wallet" }));
    expect(screen.getByText(/No Solana wallet found/)).toBeInTheDocument();
  });

  it("signs the challenge with the wallet's own sign-in", async () => {
    const signed = new TextEncoder().encode("signed text");
    const signIn = vi.fn(() =>
      Promise.resolve({
        account: { address: ADDRESS },
        signedMessage: signed,
        signature: new Uint8Array([1, 2, 3]),
      }),
    );
    fake = connected({ signIn });
    mockApi({ "POST /api/auth/wallet/nonce": CHALLENGE });
    renderWithClient(<SignButton />);
    expect(screen.getByText("4Nd1…DB4T")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Sign" }));
    await waitFor(() => expect(document.title).not.toBe(""));
    expect(signIn).toHaveBeenCalledWith({
      domain: CHALLENGE.domain,
      address: ADDRESS,
      statement: CHALLENGE.statement,
      nonce: CHALLENGE.nonce,
      issuedAt: CHALLENGE.issued_at,
      expirationTime: CHALLENGE.expiration_time,
    });
    expect(JSON.parse(document.title)).toEqual({
      address: ADDRESS,
      message: toBase64(signed),
      signature: toBase64(new Uint8Array([1, 2, 3])),
    });
  });

  it("falls back to signing the same text as a message", async () => {
    const signMessage = vi.fn(() => Promise.resolve(new Uint8Array([9])));
    fake = connected({ signMessage });
    mockApi({ "POST /api/auth/wallet/nonce": CHALLENGE });
    renderWithClient(<SignButton />);
    await userEvent.click(screen.getByRole("button", { name: "Sign" }));
    await waitFor(() => expect(document.title).not.toBe(""));
    const text = createSignInMessageText({
      domain: CHALLENGE.domain,
      address: ADDRESS,
      statement: CHALLENGE.statement,
      nonce: CHALLENGE.nonce,
      issuedAt: CHALLENGE.issued_at,
      expirationTime: CHALLENGE.expiration_time,
    });
    expect(JSON.parse(document.title).message).toBe(toBase64(new TextEncoder().encode(text)));
  });

  it("does not ask the wallet to sign for another address than the page's", async () => {
    const signIn = vi.fn();
    fake = connected({ signIn });
    mockApi({ "POST /api/auth/wallet/nonce": { ...CHALLENGE, domain: "127.0.0.1:3000" } });
    renderWithClient(<SignButton />);
    await userEvent.click(screen.getByRole("button", { name: "Sign" }));
    await waitFor(() => expect(document.title).toBe("error:wallet_wrong_address"));
    expect(signIn).not.toHaveBeenCalled();
    const words = describeError(
      new ApiError("wallet_wrong_address", 0, null, { expected: "127.0.0.1:3000" }),
    ).words;
    expect(words).toBe(
      "Wallets sign only for the app's own address. Open the app at 127.0.0.1:3000 and try again.",
    );
  });

  it("names what went wrong with the wallet", async () => {
    mockApi({ "POST /api/auth/wallet/nonce": CHALLENGE });
    fake = connected({ signMessage: () => Promise.reject(new Error("User rejected")) });
    const { unmount } = renderWithClient(<SignButton />);
    await userEvent.click(screen.getByRole("button", { name: "Sign" }));
    await waitFor(() => expect(document.title).toBe("error:wallet_declined"));
    unmount();

    document.title = "";
    fake = connected({
      signIn: () =>
        Promise.resolve({
          account: { address: "Other111111111111111111111111111111111111111" },
          signedMessage: new Uint8Array(),
          signature: new Uint8Array(),
        }),
    });
    const second = renderWithClient(<SignButton />);
    await userEvent.click(screen.getByRole("button", { name: "Sign" }));
    await waitFor(() => expect(document.title).toBe("error:wallet_mismatch"));
    second.unmount();

    document.title = "";
    fake = connected({});
    renderWithClient(<SignButton />);
    await userEvent.click(screen.getByRole("button", { name: "Sign" }));
    await waitFor(() => expect(document.title).toBe("error:wallet_cannot_sign"));
  });
});
