"use client";

import { useQuery, useQueryClient } from "@tanstack/react-query";
import dynamic from "next/dynamic";
import Link from "next/link";
import { useRouter } from "next/navigation";
import { useState } from "react";
import { z } from "zod";

import { CodeField, SendAgain } from "@/components/CodeField/CodeField";
import { ErrorAlert } from "@/components/ErrorAlert/ErrorAlert";
import { Field, TextField, TextareaField } from "@/components/Field/Field";
import { LOGIN_PANELS } from "@/components/LoginForm/LoginForm";
import { MethodSwitch } from "@/components/MethodSwitch/MethodSwitch";
import { PasswordInput } from "@/components/PasswordInput/PasswordInput";
import { ProviderButtons } from "@/components/ProviderButtons/ProviderButtons";
import { Spinner } from "@/components/Spinner/Spinner";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import type { WalletProof } from "@/components/WalletConnect/WalletConnect";
import { type Schemas, api, call } from "@/lib/api/client";
import { keys, usePublicConfig } from "@/lib/queries";
import { errorFromCode } from "@/lib/errors";
import { PROVIDER_LABEL } from "@/lib/labels";
import { useForm } from "@/lib/use-form";
import { useQueryParam } from "@/lib/use-query-param";
import {
  PASSWORD_MIN,
  bioHint,
  bioSchema,
  emailSchema,
  nameSchema,
  passwordSchema,
} from "@/lib/validation";

/** Loaded only when the wallet is chosen, so other pages do not carry the wallet libraries. */
const WalletConnect = dynamic(
  () => import("@/components/WalletConnect/WalletConnect").then((m) => m.WalletConnect),
  { ssr: false, loading: () => <Skeleton className="h-10 w-full" /> },
);

const profile = {
  first_name: nameSchema,
  last_name: nameSchema,
  bio: bioSchema,
  email: emailSchema,
};

/** What is checked depends on the method and, for codes, on whether the code was sent. */
function schemaFor(method: Schemas["LoginType"], sent: boolean) {
  if (method === "code") {
    return z.object({
      ...profile,
      login_code: sent ? z.string().trim().min(1, { error: "Required." }) : z.string(),
    });
  }
  if (method === "wallet") return z.object(profile);
  return z.object({ ...profile, password: passwordSchema });
}

export function RegisterForm() {
  const router = useRouter();
  const client = useQueryClient();
  const config = usePublicConfig();
  // Google or GitHub sent the person back: to finish the account, or with a problem.
  const via = useQueryParam("via");
  const returned = errorFromCode(useQueryParam("error"));
  const methods = (config.data?.login_methods ?? ["password"]).filter((m) =>
    LOGIN_PANELS.includes(m),
  );
  const [method, setMethod] = useState<Schemas["LoginType"]>("password");
  const [sent, setSent] = useState(false);
  const form = useForm(
    { first_name: "", last_name: "", bio: "", email: "", password: "", login_code: "" },
    schemaFor(method, sent),
  );

  const sendCode = async (email: string) => {
    await call(api.POST("/api/auth/code", { body: { email, purpose: "register" } }));
  };

  const register = async (v: typeof form.values, proof?: WalletProof) => {
    const me = await call(
      api.POST("/api/auth/register", {
        body: {
          method,
          first_name: v.first_name,
          last_name: v.last_name,
          bio: v.bio.trim() || null,
          email: v.email,
          password: method === "password" ? v.password : null,
          login_code: method === "code" ? v.login_code.trim() : null,
          ...proof,
        },
      }),
    );
    client.setQueryData(keys.me, me);
    router.replace(me.user.status === "pending" ? "/pending" : "/dashboard");
  };

  const registerWithWallet = (sign: () => Promise<WalletProof>) =>
    form.submit(async (v) => register(v, await sign()));

  const onSubmit = form.submit(async (v) => {
    // The wallet's own button submits, once the wallet is connected.
    if (method === "wallet") return;
    if (method === "code" && !sent) {
      await sendCode(v.email);
      setSent(true);
      return;
    }
    await register(v);
  });

  const choose = (m: Schemas["LoginType"]) => {
    setMethod(m);
    setSent(false);
  };

  if (config.isPending) return <Skeleton className="h-64 w-full" />;
  if (config.data?.registration_mode === "closed") {
    return (
      <Alert>
        <AlertDescription>
          Registration is closed. Ask an admin to create an account for you.
        </AlertDescription>
      </Alert>
    );
  }

  if (via === "external") return <ExternalSignup />;

  if (method === "external") {
    return (
      <div className="grid gap-4">
        <MethodSwitch methods={methods} value={method} onChange={choose} />
        {returned ? <ErrorAlert error={returned} /> : null}
        <ProviderButtons providers={config.data?.oauth_providers ?? []} intent="register" />
        <p className="text-xs text-muted-foreground">
          You come back here to add your name before the account is made.
        </p>
      </div>
    );
  }

  return (
    <form className="grid gap-4" onSubmit={(e) => void onSubmit(e)} noValidate>
      <MethodSwitch methods={methods} value={method} onChange={choose} />
      {returned ? <ErrorAlert error={returned} /> : null}
      {form.formError ? <ErrorAlert error={form.formError} /> : null}
      <div className="grid gap-4 sm:grid-cols-2">
        <TextField
          label="First name"
          autoComplete="given-name"
          value={form.values.first_name}
          onChange={form.set("first_name")}
          error={form.errors.first_name}
        />
        <TextField
          label="Last name"
          autoComplete="family-name"
          value={form.values.last_name}
          onChange={form.set("last_name")}
          error={form.errors.last_name}
        />
      </div>
      <TextField
        label="Email"
        type="email"
        autoComplete="email"
        readOnly={sent}
        value={form.values.email}
        onChange={form.set("email")}
        error={form.errors.email}
      />
      {method === "password" ? (
        <Field
          label="Password"
          hint={`At least ${PASSWORD_MIN} characters.`}
          error={form.errors.password}
        >
          {(control) => (
            <PasswordInput
              {...control}
              autoComplete="new-password"
              value={form.values.password}
              onChange={form.set("password")}
            />
          )}
        </Field>
      ) : null}
      <TextareaField
        label="Bio"
        hint={bioHint(form.values.bio)}
        value={form.values.bio}
        onChange={form.set("bio")}
        error={form.errors.bio}
      />
      {config.data?.registration_mode === "approval" ? (
        <p className="text-xs text-muted-foreground">
          An admin approves new accounts before they can be used.
        </p>
      ) : null}
      {method === "code" && sent ? (
        <>
          <CodeField
            value={form.values.login_code}
            onChange={form.set("login_code")}
            error={form.errors.login_code}
          />
          <div className="flex flex-wrap items-center justify-between gap-2">
            <SendAgain onSend={() => sendCode(form.values.email)} />
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
      {method === "wallet" ? (
        <WalletConnect>
          {({ sign }) => (
            <Button
              type="button"
              disabled={form.pending}
              className="w-full"
              onClick={() => void registerWithWallet(sign)()}
            >
              {form.pending ? <Spinner /> : null}
              Create account with this wallet
            </Button>
          )}
        </WalletConnect>
      ) : (
        <Button type="submit" disabled={form.pending} className="w-full">
          {form.pending ? <Spinner /> : null}
          {method === "code" && !sent ? "Send code" : "Create account"}
        </Button>
      )}
      <p className="text-center text-muted-foreground">
        Already registered?{" "}
        <Link href="/login" className="font-medium text-foreground underline underline-offset-4">
          Log in
        </Link>
      </p>
    </form>
  );
}

/** The second half of signing up with Google or GitHub: what the provider vouched for, and the
 * names it did not give. */
function ExternalSignup() {
  const pending = useQuery({
    queryKey: ["oauth-signup"],
    queryFn: () => call(api.GET("/api/auth/oauth/signup")),
    retry: false,
  });
  if (pending.isPending) return <Skeleton className="h-64 w-full" />;
  if (!pending.data) {
    return (
      <div className="grid gap-3">
        <ErrorAlert error={errorFromCode("provider_failed")} />
        <Button variant="outline" nativeButton={false} render={<Link href="/register" />}>
          Start again
        </Button>
      </div>
    );
  }
  return <ExternalSignupForm signup={pending.data} />;
}

function ExternalSignupForm({ signup }: { signup: Schemas["PendingSignupDto"] }) {
  const router = useRouter();
  const client = useQueryClient();
  const form = useForm(
    { first_name: signup.first_name ?? "", last_name: signup.last_name ?? "", bio: "" },
    z.object({ first_name: nameSchema, last_name: nameSchema, bio: bioSchema }),
  );
  const onSubmit = form.submit(async (v) => {
    const me = await call(
      api.POST("/api/auth/register", {
        body: {
          method: "external",
          email: signup.email,
          first_name: v.first_name,
          last_name: v.last_name,
          bio: v.bio.trim() || null,
        },
      }),
    );
    client.setQueryData(keys.me, me);
    router.replace(me.user.status === "pending" ? "/pending" : "/dashboard");
  });
  return (
    <form className="grid gap-4" onSubmit={(e) => void onSubmit(e)} noValidate>
      {form.formError ? <ErrorAlert error={form.formError} /> : null}
      <p className="text-muted-foreground">
        You sign up with {PROVIDER_LABEL[signup.provider] ?? signup.provider}. Check your name, then
        create the account.
      </p>
      <TextField label="Email" type="email" readOnly value={signup.email} />
      <div className="grid gap-4 sm:grid-cols-2">
        <TextField
          label="First name"
          autoComplete="given-name"
          value={form.values.first_name}
          onChange={form.set("first_name")}
          error={form.errors.first_name}
        />
        <TextField
          label="Last name"
          autoComplete="family-name"
          value={form.values.last_name}
          onChange={form.set("last_name")}
          error={form.errors.last_name}
        />
      </div>
      <TextareaField
        label="Bio"
        hint={bioHint(form.values.bio)}
        value={form.values.bio}
        onChange={form.set("bio")}
        error={form.errors.bio}
      />
      <Button type="submit" disabled={form.pending} className="w-full">
        {form.pending ? <Spinner /> : null}
        Create account
      </Button>
    </form>
  );
}
