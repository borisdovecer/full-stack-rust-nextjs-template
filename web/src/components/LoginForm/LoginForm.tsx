"use client";

import { useQueryClient } from "@tanstack/react-query";
import type { Route } from "next";
import dynamic from "next/dynamic";
import Link from "next/link";
import { useRouter } from "next/navigation";
import { useState } from "react";
import { z } from "zod";

import { safeNext } from "@/components/AuthGate/gate";
import { CodeField, SendAgain } from "@/components/CodeField/CodeField";
import { ErrorAlert } from "@/components/ErrorAlert/ErrorAlert";
import { Field, TextField } from "@/components/Field/Field";
import { MethodSwitch } from "@/components/MethodSwitch/MethodSwitch";
import { PasswordInput } from "@/components/PasswordInput/PasswordInput";
import { ProviderButtons } from "@/components/ProviderButtons/ProviderButtons";
import { Spinner } from "@/components/Spinner/Spinner";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { type Schemas, api, call } from "@/lib/api/client";
import { keys, usePublicConfig } from "@/lib/queries";
import { errorFromCode } from "@/lib/errors";
import { useForm } from "@/lib/use-form";
import { useQueryParam } from "@/lib/use-query-param";
import type { WalletProof } from "@/components/WalletConnect/WalletConnect";
import { emailSchema } from "@/lib/validation";

type LoginType = Schemas["LoginType"];

/** The methods this form can draw. A method's part adds itself here when it is built. */
export const LOGIN_PANELS: LoginType[] = ["password", "code", "wallet", "external"];

/** Loaded only when the wallet is chosen, so other pages do not carry the wallet libraries. */
const WalletConnect = dynamic(
  () => import("@/components/WalletConnect/WalletConnect").then((m) => m.WalletConnect),
  { ssr: false, loading: () => <Skeleton className="h-10 w-full" /> },
);

export function LoginForm() {
  const router = useRouter();
  const client = useQueryClient();
  const { data: config } = usePublicConfig();
  const methods = (config?.login_methods ?? ["password"]).filter((m) => LOGIN_PANELS.includes(m));
  const [method, setMethod] = useState<LoginType>("password");
  // Set when Google or GitHub sent the person back here with a problem.
  const returned = errorFromCode(useQueryParam("error"));

  const signedIn = (me: Schemas["Me"]) => {
    client.setQueryData(keys.me, me);
    // Read at submit time rather than with useSearchParams, which would need a Suspense
    // boundary that hydrates late and mismatches the server HTML.
    const next = new URLSearchParams(window.location.search).get("next");
    router.replace((me.user.status === "pending" ? "/pending" : safeNext(next)) as Route);
  };

  return (
    <div className="grid gap-4">
      <MethodSwitch methods={methods} value={method} onChange={setMethod} />
      {returned ? <ErrorAlert error={returned} /> : null}
      {method === "external" ? (
        <ProviderButtons providers={config?.oauth_providers ?? []} intent="login" />
      ) : null}
      {method === "code" ? <CodeLogin onSignedIn={signedIn} /> : null}
      {method === "wallet" ? <WalletLogin onSignedIn={signedIn} /> : null}
      {method === "password" ? <PasswordLogin onSignedIn={signedIn} /> : null}
      {config && config.registration_mode !== "closed" ? (
        <p className="text-center text-muted-foreground">
          No account?{" "}
          <Link
            href="/register"
            className="font-medium text-foreground underline underline-offset-4"
          >
            Register
          </Link>
        </p>
      ) : null}
    </div>
  );
}

const passwordSchema = z.object({
  email: z.string().trim().min(1, { error: "Required." }),
  password: z.string().min(1, { error: "Required." }),
});

function PasswordLogin({ onSignedIn }: { onSignedIn: (me: Schemas["Me"]) => void }) {
  const form = useForm({ email: "", password: "" }, passwordSchema);
  const onSubmit = form.submit(async (v) => {
    onSignedIn(await call(api.POST("/api/auth/login", { body: { method: "password", ...v } })));
  });
  return (
    <form className="grid gap-4" onSubmit={(e) => void onSubmit(e)} noValidate>
      {form.formError ? <ErrorAlert error={form.formError} /> : null}
      <TextField
        label="Email"
        type="email"
        autoComplete="email"
        value={form.values.email}
        onChange={form.set("email")}
        error={form.errors.email}
      />
      <Field label="Password" error={form.errors.password}>
        {(control) => (
          <PasswordInput
            {...control}
            autoComplete="current-password"
            value={form.values.password}
            onChange={form.set("password")}
          />
        )}
      </Field>
      <Button type="submit" disabled={form.pending} className="w-full">
        {form.pending ? <Spinner /> : null}
        Log in
      </Button>
    </form>
  );
}

const codeSchema = z.object({ email: emailSchema, login_code: z.string() });
const sentCodeSchema = z.object({
  email: emailSchema,
  login_code: z.string().trim().min(1, { error: "Required." }),
});

/** Email first; the same button then sends the code, and after that signs in with it. */
function CodeLogin({ onSignedIn }: { onSignedIn: (me: Schemas["Me"]) => void }) {
  const [sent, setSent] = useState(false);
  const form = useForm({ email: "", login_code: "" }, sent ? sentCodeSchema : codeSchema);
  const send = async (email: string) => {
    await call(api.POST("/api/auth/code", { body: { email, purpose: "login" } }));
  };
  const onSubmit = form.submit(async (v) => {
    if (!sent) {
      await send(v.email);
      setSent(true);
      return;
    }
    onSignedIn(
      await call(
        api.POST("/api/auth/login", {
          body: { method: "code", email: v.email, login_code: v.login_code.trim() },
        }),
      ),
    );
  });
  return (
    <form className="grid gap-4" onSubmit={(e) => void onSubmit(e)} noValidate>
      {form.formError ? <ErrorAlert error={form.formError} /> : null}
      <TextField
        label="Email"
        type="email"
        autoComplete="email"
        readOnly={sent}
        value={form.values.email}
        onChange={form.set("email")}
        error={form.errors.email}
      />
      {sent ? (
        <>
          <CodeField
            value={form.values.login_code}
            onChange={form.set("login_code")}
            error={form.errors.login_code}
          />
          <div className="flex flex-wrap items-center justify-between gap-2">
            <SendAgain onSend={() => send(form.values.email)} />
            <Button
              type="button"
              variant="link"
              size="sm"
              className="h-auto p-0"
              onClick={() => {
                setSent(false);
                form.setValues((v) => ({ ...v, login_code: "" }));
              }}
            >
              Use another email
            </Button>
          </div>
        </>
      ) : null}
      <Button type="submit" disabled={form.pending} className="w-full">
        {form.pending ? <Spinner /> : null}
        {sent ? "Log in" : "Send code"}
      </Button>
    </form>
  );
}

/** Email first, then the wallet that account uses signs a one-time message. */
function WalletLogin({ onSignedIn }: { onSignedIn: (me: Schemas["Me"]) => void }) {
  const form = useForm({ email: "" }, z.object({ email: emailSchema }));
  const signIn = (sign: () => Promise<WalletProof>) =>
    form.submit(async (v) => {
      const proof = await sign();
      onSignedIn(
        await call(
          api.POST("/api/auth/login", { body: { method: "wallet", email: v.email, ...proof } }),
        ),
      );
    });
  return (
    <div className="grid gap-4">
      {form.formError ? <ErrorAlert error={form.formError} /> : null}
      <TextField
        label="Email"
        type="email"
        autoComplete="email"
        value={form.values.email}
        onChange={form.set("email")}
        error={form.errors.email}
      />
      <WalletConnect>
        {({ sign }) => (
          <Button
            type="button"
            disabled={form.pending}
            className="w-full"
            onClick={() => void signIn(sign)()}
          >
            {form.pending ? <Spinner /> : null}
            Sign in with this wallet
          </Button>
        )}
      </WalletConnect>
    </div>
  );
}
