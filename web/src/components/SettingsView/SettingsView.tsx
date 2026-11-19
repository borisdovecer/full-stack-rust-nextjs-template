"use client";

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import dynamic from "next/dynamic";
import { useRouter } from "next/navigation";
import { useEffect, useState } from "react";
import { toast } from "sonner";
import { z } from "zod";

import { CodeField, SendAgain } from "@/components/CodeField/CodeField";
import { ConfirmModal } from "@/components/ConfirmModal/ConfirmModal";
import { ErrorAlert } from "@/components/ErrorAlert/ErrorAlert";
import { Field, TextField, TextareaField } from "@/components/Field/Field";
import { FormActions } from "@/components/FormActions/FormActions";
import { PageBar } from "@/components/PageBar/PageBar";
import { PasswordInput } from "@/components/PasswordInput/PasswordInput";
import { ProviderButtons } from "@/components/ProviderButtons/ProviderButtons";
import { Spinner } from "@/components/Spinner/Spinner";
import { THEMES, useThemeChoice } from "@/components/ThemeToggle/ThemeToggle";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Separator } from "@/components/ui/separator";
import { Skeleton } from "@/components/ui/skeleton";
import type { WalletProof } from "@/components/WalletConnect/WalletConnect";
import { ApiError, type Schemas, api, call } from "@/lib/api/client";
import { describeError } from "@/lib/errors";
import { formatRelative } from "@/lib/format";
import { LOGIN_TYPE_SENTENCE } from "@/lib/labels";
import { clearUserData, keys, useMe, usePublicConfig } from "@/lib/queries";
import { errorFromCode } from "@/lib/errors";
import { useForm } from "@/lib/use-form";
import { useQueryParam } from "@/lib/use-query-param";
import { cn } from "@/lib/utils";
import { PASSWORD_MIN, bioHint, bioSchema, nameSchema, passwordSchema } from "@/lib/validation";

export function SettingsView() {
  return (
    <div className="mx-auto w-full max-w-3xl px-4 py-4 md:px-5">
      <PageBar title="Settings" />
      <div className="grid gap-4">
        <AppearanceCard />
        <ProfileCard />
        <SignInMethodCard />
        <SessionsCard />
      </div>
    </div>
  );
}

function AppearanceCard() {
  const { theme, choose } = useThemeChoice();
  return (
    <Card>
      <CardHeader>
        <CardTitle>Appearance</CardTitle>
        <CardDescription>
          Saved to your account, so it follows you to other browsers.
        </CardDescription>
      </CardHeader>
      <CardContent>
        <div role="radiogroup" aria-label="Theme" className="grid grid-cols-3 gap-2">
          {THEMES.map(({ value, label, Icon }) => (
            <button
              key={value}
              type="button"
              role="radio"
              aria-checked={theme === value}
              onClick={() => choose(value)}
              className={cn(
                "flex flex-col items-center gap-2 rounded-lg border p-3 font-medium transition-colors hover:bg-muted [&_svg]:size-5",
                theme === value && "border-primary ring-1 ring-primary",
              )}
            >
              <Icon />
              {label}
            </button>
          ))}
        </div>
      </CardContent>
    </Card>
  );
}

const profileSchema = z.object({ first_name: nameSchema, last_name: nameSchema, bio: bioSchema });

function ProfileCard() {
  const { data: me } = useMe();
  // The form copies its starting values once, so it waits for the account to load.
  if (!me) return <Skeleton className="h-72 w-full" />;
  return <ProfileForm user={me.user} />;
}

function ProfileForm({ user }: { user: Schemas["UserDto"] }) {
  const client = useQueryClient();
  const saved = { first_name: user.first_name, last_name: user.last_name, bio: user.bio ?? "" };
  const form = useForm(saved, profileSchema);
  const unchanged =
    form.values.first_name.trim() === saved.first_name &&
    form.values.last_name.trim() === saved.last_name &&
    form.values.bio.trim() === saved.bio;
  const onSubmit = form.submit(async (v) => {
    const updated = await call(api.PATCH("/api/me", { body: v }));
    client.setQueryData(keys.me, updated);
    toast.success("Profile saved.");
  });
  return (
    <Card>
      <CardHeader>
        <CardTitle>Profile</CardTitle>
        <CardDescription>{user.email}</CardDescription>
      </CardHeader>
      <CardContent>
        <form className="grid gap-3" onSubmit={(e) => void onSubmit(e)} noValidate>
          {form.formError ? <ErrorAlert error={form.formError} /> : null}
          <div className="grid gap-3 sm:grid-cols-2">
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
          <FormActions>
            <Button type="submit" disabled={form.pending || unchanged}>
              {form.pending ? <Spinner /> : null}
              Save
            </Button>
          </FormActions>
        </form>
      </CardContent>
    </Card>
  );
}

const passwordForm = z.object({
  current_password: z.string().min(1, { error: "Required." }),
  new_password: passwordSchema,
});

/** How the person signs in: change the password of a password account, or switch to another
 * method the server offers. A switch needs a recent sign-in, which the API checks. */
function SignInMethodCard() {
  const { data: me } = useMe();
  const { data: config } = usePublicConfig();
  const [switching, setSwitching] = useState<Schemas["LoginType"] | null>(null);
  // Google or GitHub sent the person back here after a switch.
  const returned = errorFromCode(useQueryParam("error"));
  const changed = useQueryParam("changed");
  useEffect(() => {
    if (changed === "external") toast.success("You now sign in with Google or GitHub.");
  }, [changed]);
  if (!me) return <Skeleton className="h-64 w-full" />;
  const current = me.user.login_type;
  const others = SWITCHABLE.filter(
    (m) => m !== current && (config?.login_methods.includes(m) ?? false),
  );
  const done = () => setSwitching(null);
  return (
    <Card>
      <CardHeader>
        <CardTitle>Sign-in method</CardTitle>
        <CardDescription>{LOGIN_TYPE_SENTENCE[current]}</CardDescription>
      </CardHeader>
      <CardContent className="grid gap-5">
        {returned ? <ErrorAlert error={returned} /> : null}
        {current === "password" ? <ChangePasswordForm /> : null}
        {current === "password" && others.length ? <Separator /> : null}
        {switching === "password" ? <UsePasswordForm onDone={done} /> : null}
        {switching === "code" ? <UseCodeForm email={me.user.email} onDone={done} /> : null}
        {switching === "wallet" ? <UseWalletForm onDone={done} /> : null}
        {switching === "external" ? (
          <div className="grid gap-3">
            <p className="text-muted-foreground">
              Pick the account to sign in with. It must use {me.user.email}. Your current method
              stops working.
            </p>
            <ProviderButtons providers={config?.oauth_providers ?? []} intent="link" />
            <FormActions>
              <Button type="button" variant="ghost" onClick={done}>
                Cancel
              </Button>
            </FormActions>
          </div>
        ) : null}
        {switching === null && others.length ? (
          <div className="flex flex-wrap gap-2">
            {others.map((m) => (
              <Button key={m} variant="outline" onClick={() => setSwitching(m)}>
                {SWITCH_LABEL[m]}
              </Button>
            ))}
          </div>
        ) : null}
      </CardContent>
    </Card>
  );
}

/** Methods a person can switch to from Settings; each needs its form below. */
const SWITCHABLE: Schemas["LoginType"][] = ["password", "code", "wallet", "external"];

const SWITCH_LABEL: Partial<Record<Schemas["LoginType"], string>> = {
  password: "Use a password instead",
  code: "Use email codes instead",
  wallet: "Use a wallet instead",
  external: "Use Google or GitHub instead",
};

/** Loaded only when the wallet is chosen, so other pages do not carry the wallet libraries. */
const WalletConnect = dynamic(
  () => import("@/components/WalletConnect/WalletConnect").then((m) => m.WalletConnect),
  { ssr: false, loading: () => <Skeleton className="h-10 w-full" /> },
);

/** Connects a wallet and switches to it with a signed challenge. */
function UseWalletForm({ onDone }: { onDone: () => void }) {
  const client = useQueryClient();
  const form = useForm({}, z.object({}));
  const use = (sign: () => Promise<WalletProof>) =>
    form.submit(async () => {
      const proof = await sign();
      const me = await call(
        api.PUT("/api/me/login-method", { body: { method: "wallet", ...proof } }),
      );
      switched(client, me, "You now sign in with your email and this wallet.");
      onDone();
    });
  return (
    <div className="grid gap-3">
      {form.formError ? <ErrorAlert error={form.formError} /> : null}
      <p className="text-muted-foreground">
        Connect the wallet you want to sign in with. Your current method stops working.
      </p>
      <WalletConnect>
        {({ sign }) => (
          <FormActions>
            <Button type="button" variant="ghost" onClick={onDone}>
              Cancel
            </Button>
            <Button type="button" disabled={form.pending} onClick={() => void use(sign)()}>
              {form.pending ? <Spinner /> : null}
              Use this wallet
            </Button>
          </FormActions>
        )}
      </WalletConnect>
    </div>
  );
}

function switched(client: ReturnType<typeof useQueryClient>, me: Schemas["Me"], words: string) {
  client.setQueryData(keys.me, me);
  void client.invalidateQueries({ queryKey: keys.sessions });
  toast.success(`${words} Your other sessions are signed out.`);
}

function UsePasswordForm({ onDone }: { onDone: () => void }) {
  const client = useQueryClient();
  const form = useForm({ password: "" }, z.object({ password: passwordSchema }));
  const onSubmit = form.submit(async (v) => {
    const me = await call(
      api.PUT("/api/me/login-method", { body: { method: "password", password: v.password } }),
    );
    switched(client, me, "You now sign in with a password.");
    onDone();
  });
  return (
    <form className="grid gap-3" onSubmit={(e) => void onSubmit(e)} noValidate>
      {form.formError ? <ErrorAlert error={form.formError} /> : null}
      <Field
        label="New password"
        hint={`At least ${PASSWORD_MIN} characters. Your current method stops working.`}
        error={form.errors.password}
      >
        {(c) => (
          <PasswordInput
            {...c}
            autoComplete="new-password"
            value={form.values.password}
            onChange={form.set("password")}
          />
        )}
      </Field>
      <FormActions>
        <Button type="button" variant="ghost" onClick={onDone}>
          Cancel
        </Button>
        <Button type="submit" disabled={form.pending}>
          {form.pending ? <Spinner /> : null}
          Use a password
        </Button>
      </FormActions>
    </form>
  );
}

/** Sends a code to the account's own email, then switches with it. */
function UseCodeForm({ email, onDone }: { email: string; onDone: () => void }) {
  const client = useQueryClient();
  const [sent, setSent] = useState(false);
  const form = useForm(
    { login_code: "" },
    z.object({
      login_code: sent ? z.string().trim().min(1, { error: "Required." }) : z.string(),
    }),
  );
  const send = async () => {
    await call(api.POST("/api/me/login-code"));
  };
  const onSubmit = form.submit(async (v) => {
    if (!sent) {
      await send();
      setSent(true);
      return;
    }
    const me = await call(
      api.PUT("/api/me/login-method", {
        body: { method: "code", login_code: v.login_code.trim() },
      }),
    );
    switched(client, me, "You now sign in with a code sent to your email.");
    onDone();
  });
  return (
    <form className="grid gap-3" onSubmit={(e) => void onSubmit(e)} noValidate>
      {form.formError ? <ErrorAlert error={form.formError} /> : null}
      <p className="text-muted-foreground">
        {sent
          ? `We sent a code to ${email}. Enter it to switch; your current method stops working.`
          : `We will send a code to ${email} to make sure it reaches you.`}
      </p>
      {sent ? (
        <>
          <CodeField
            value={form.values.login_code}
            onChange={form.set("login_code")}
            error={form.errors.login_code}
          />
          <SendAgain onSend={send} />
        </>
      ) : null}
      <FormActions>
        <Button type="button" variant="ghost" onClick={onDone}>
          Cancel
        </Button>
        <Button type="submit" disabled={form.pending}>
          {form.pending ? <Spinner /> : null}
          {sent ? "Use email codes" : "Send code"}
        </Button>
      </FormActions>
    </form>
  );
}

function ChangePasswordForm() {
  const client = useQueryClient();
  const form = useForm({ current_password: "", new_password: "" }, passwordForm);
  const onSubmit = form.submit(async (v) => {
    try {
      await call(api.POST("/api/me/password", { body: v }));
    } catch (e) {
      // A wrong current password belongs under that field, not at the top of the form.
      if (e instanceof ApiError && e.code === "wrong_password") {
        throw new ApiError("validation_failed", e.status, e.requestId, {
          fields: { current_password: ["wrong_password"] },
        });
      }
      throw e;
    }
    await client.invalidateQueries({ queryKey: keys.sessions });
    toast.success("Password changed. Your other sessions are signed out.");
    form.reset();
  });
  return (
    <form className="grid gap-3" onSubmit={(e) => void onSubmit(e)} noValidate>
      {form.formError ? <ErrorAlert error={form.formError} /> : null}
      <Field label="Current password" error={form.errors.current_password}>
        {(c) => (
          <PasswordInput
            {...c}
            autoComplete="current-password"
            value={form.values.current_password}
            onChange={form.set("current_password")}
          />
        )}
      </Field>
      <Field
        label="New password"
        hint={`At least ${PASSWORD_MIN} characters. Changing it signs you out everywhere else.`}
        error={form.errors.new_password}
      >
        {(c) => (
          <PasswordInput
            {...c}
            autoComplete="new-password"
            value={form.values.new_password}
            onChange={form.set("new_password")}
          />
        )}
      </Field>
      <FormActions>
        <Button type="submit" disabled={form.pending}>
          {form.pending ? <Spinner /> : null}
          Change password
        </Button>
      </FormActions>
    </form>
  );
}

function SessionsCard() {
  const client = useQueryClient();
  const router = useRouter();
  const [confirmAll, setConfirmAll] = useState(false);
  const sessions = useQuery({
    queryKey: keys.sessions,
    queryFn: () => call(api.GET("/api/me/sessions")),
  });

  const revoke = useMutation({
    mutationFn: (id: string) =>
      call(api.DELETE("/api/me/sessions/{id}", { params: { path: { id } } })),
    onSuccess: () => void client.invalidateQueries({ queryKey: keys.sessions }),
    onError: (e) => toast.error(describeError(e).words),
  });
  const logoutAll = useMutation({
    mutationFn: () => call(api.POST("/api/auth/logout-all")),
    onSuccess: () => {
      clearUserData(client);
      router.replace("/login");
    },
    onError: (e) => toast.error(describeError(e).words),
  });

  return (
    <Card>
      <CardHeader>
        <CardTitle>Sessions</CardTitle>
        <CardDescription>Every browser where you are signed in.</CardDescription>
      </CardHeader>
      <CardContent className="grid gap-3">
        {sessions.error ? (
          <ErrorAlert error={sessions.error} onRetry={() => void sessions.refetch()} />
        ) : null}
        {sessions.isPending ? <Skeleton className="h-16 w-full" /> : null}
        <ul className="grid gap-2">
          {sessions.data?.map((s) => (
            <li
              key={s.id}
              className="flex items-center justify-between gap-3 rounded-lg border p-3"
            >
              <div className="min-w-0">
                <p className="truncate font-medium">{describeAgent(s.user_agent)}</p>
                <p className="text-xs text-muted-foreground">
                  {s.ip ?? "Unknown address"} · active {formatRelative(s.last_seen_at)}
                </p>
              </div>
              {s.current ? (
                <Badge variant="secondary">This browser</Badge>
              ) : (
                <Button
                  variant="outline"
                  size="sm"
                  disabled={revoke.isPending}
                  onClick={() => revoke.mutate(s.id)}
                >
                  Sign out
                </Button>
              )}
            </li>
          ))}
        </ul>
        <FormActions>
          <Button variant="destructive" onClick={() => setConfirmAll(true)}>
            Log out everywhere
          </Button>
        </FormActions>
      </CardContent>
      <ConfirmModal
        open={confirmAll}
        onOpenChange={setConfirmAll}
        title="Log out everywhere?"
        description="Every session ends, this one too. You will need to log in again."
        confirmLabel="Log out everywhere"
        destructive
        pending={logoutAll.isPending}
        onConfirm={() => logoutAll.mutate()}
      />
    </Card>
  );
}

/** A short name for a browser from its user agent. Good enough to tell sessions apart. */
export function describeAgent(ua: string | null | undefined): string {
  if (!ua) return "Unknown browser";
  const browser = /Edg\//.test(ua)
    ? "Edge"
    : /Firefox\//.test(ua)
      ? "Firefox"
      : /Chrome\//.test(ua)
        ? "Chrome"
        : /Safari\//.test(ua)
          ? "Safari"
          : /curl\//.test(ua)
            ? "curl"
            : "Browser";
  const os = /Windows/.test(ua)
    ? "Windows"
    : /iPhone|iPad/.test(ua)
      ? "iOS"
      : /Mac OS X/.test(ua)
        ? "macOS"
        : /Android/.test(ua)
          ? "Android"
          : /Linux/.test(ua)
            ? "Linux"
            : null;
  return os ? `${browser} on ${os}` : browser;
}
