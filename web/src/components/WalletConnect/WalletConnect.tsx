"use client";

import { WalletReadyState, type WalletName } from "@solana/wallet-adapter-base";
import { WalletProvider, useWallet } from "@solana/wallet-adapter-react";
import { createSignInMessageText } from "@solana/wallet-standard-util";
import { WalletIcon } from "lucide-react";
import { type ReactNode, useEffect, useRef, useState } from "react";

import { ErrorAlert } from "@/components/ErrorAlert/ErrorAlert";
import { Modal } from "@/components/Modal/Modal";
import { Spinner } from "@/components/Spinner/Spinner";
import { Button } from "@/components/ui/button";
import { ApiError, api, call } from "@/lib/api/client";

/** A signed sign-in challenge, as the API takes it. */
export interface WalletProof {
  address: string;
  /** The exact bytes the wallet signed, base64. */
  message: string;
  /** The Ed25519 signature, base64. */
  signature: string;
}

export interface ConnectedWallet {
  address: string;
  /** Asks the API for a challenge and has the wallet sign it. Throws an `ApiError`. */
  sign: () => Promise<WalletProof>;
}

const NO_ADAPTERS: never[] = [];

/**
 * Connects a Solana wallet (Phantom, Solflare and other wallet-standard wallets the browser
 * has) and, once connected, renders `children` with the address and a way to sign a sign-in
 * challenge. Nothing here touches the chain: signing a message costs nothing.
 */
export function WalletConnect({ children }: { children: (wallet: ConnectedWallet) => ReactNode }) {
  return (
    <WalletProvider wallets={NO_ADAPTERS} autoConnect={false}>
      <Connector>{children}</Connector>
    </WalletProvider>
  );
}

function Connector({ children }: { children: (wallet: ConnectedWallet) => ReactNode }) {
  const wallet = useWallet();
  const [picking, setPicking] = useState(false);
  // The wallet picked but not connected yet; not state, since it never changes what is drawn.
  const wanted = useRef<WalletName | null>(null);
  const [error, setError] = useState<unknown>(null);
  const address = wallet.publicKey?.toBase58() ?? null;
  const usable = wallet.wallets.filter(
    (w) =>
      w.readyState === WalletReadyState.Installed || w.readyState === WalletReadyState.Loadable,
  );

  // `select` takes effect on the next render; connect once the chosen wallet is the current one.
  const { wallet: current, connected, connecting, connect } = wallet;
  useEffect(() => {
    if (!wanted.current || current?.adapter.name !== wanted.current || connected || connecting) {
      return;
    }
    wanted.current = null;
    connect().catch(() => setError(new ApiError("wallet_declined", 0, null)));
  }, [current, connected, connecting, connect]);

  const choose = (name: WalletName) => {
    setError(null);
    setPicking(false);
    if (current?.adapter.name === name) {
      connect().catch(() => setError(new ApiError("wallet_declined", 0, null)));
      return;
    }
    wanted.current = name;
    wallet.select(name);
  };

  const sign = async (): Promise<WalletProof> => {
    if (!address) throw new ApiError("wallet_declined", 0, null);
    const c = await call(api.POST("/api/auth/wallet/nonce", { body: { address } }));
    // A wallet refuses, without a word to the person, to sign for a domain that is not the
    // page's own; say which address to open instead.
    if (c.domain !== window.location.host) {
      throw new ApiError("wallet_wrong_address", 0, null, { expected: c.domain });
    }
    const input = {
      domain: c.domain,
      address,
      statement: c.statement,
      nonce: c.nonce,
      issuedAt: c.issued_at,
      expirationTime: c.expiration_time,
    };
    try {
      if (wallet.signIn) {
        const out = await wallet.signIn(input);
        if (out.account.address !== address) throw new ApiError("wallet_mismatch", 0, null);
        return {
          address,
          message: toBase64(out.signedMessage),
          signature: toBase64(out.signature),
        };
      }
      if (wallet.signMessage) {
        const bytes = new TextEncoder().encode(createSignInMessageText(input));
        const signature = await wallet.signMessage(bytes);
        return { address, message: toBase64(bytes), signature: toBase64(signature) };
      }
    } catch (e) {
      throw e instanceof ApiError ? e : new ApiError("wallet_declined", 0, null);
    }
    throw new ApiError("wallet_cannot_sign", 0, null);
  };

  if (address) {
    return (
      <div className="grid gap-4">
        <div className="flex items-center justify-between gap-2 rounded-lg border px-3 py-2">
          <div className="min-w-0">
            <p className="text-xs text-muted-foreground">
              {wallet.wallet?.adapter.name ?? "Wallet"}
            </p>
            <p className="truncate font-mono text-xs" title={address}>
              {shorten(address)}
            </p>
          </div>
          <Button type="button" variant="ghost" size="sm" onClick={() => void wallet.disconnect()}>
            Disconnect
          </Button>
        </div>
        {children({ address, sign })}
      </div>
    );
  }

  return (
    <div className="grid gap-2">
      {error ? <ErrorAlert error={error} /> : null}
      <Button
        type="button"
        variant="outline"
        className="w-full"
        disabled={wallet.connecting}
        onClick={() => setPicking(true)}
      >
        {wallet.connecting ? <Spinner /> : <WalletIcon />}
        Connect a wallet
      </Button>
      <p className="text-xs text-muted-foreground">
        You sign a message to prove the wallet is yours. No transaction, no fee.
      </p>
      <Modal
        open={picking}
        onOpenChange={setPicking}
        title="Connect a wallet"
        description="Pick the Solana wallet you use in this browser."
      >
        {usable.length ? (
          <div className="grid gap-2">
            {usable.map((w) => (
              <Button
                key={w.adapter.name}
                type="button"
                variant="outline"
                className="justify-start"
                onClick={() => choose(w.adapter.name)}
              >
                <WalletIcon />
                {w.adapter.name}
              </Button>
            ))}
          </div>
        ) : (
          <p className="text-muted-foreground">
            No Solana wallet found in this browser. Install Phantom or Solflare, then reload the
            page.
          </p>
        )}
      </Modal>
    </div>
  );
}

function shorten(address: string): string {
  return address.length > 12 ? `${address.slice(0, 4)}…${address.slice(-4)}` : address;
}

export function toBase64(bytes: Uint8Array): string {
  let s = "";
  for (const b of bytes) s += String.fromCharCode(b);
  return btoa(s);
}
