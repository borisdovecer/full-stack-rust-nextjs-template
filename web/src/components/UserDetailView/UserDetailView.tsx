"use client";

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { ArrowLeftIcon, KeyRoundIcon, MailIcon } from "lucide-react";
import Link from "next/link";
import { useState } from "react";
import { toast } from "sonner";
import { z } from "zod";

import { PageSkeleton } from "@/components/AuthGate/AuthGate";
import { ConfirmModal } from "@/components/ConfirmModal/ConfirmModal";
import { CopyValue } from "@/components/CopyValue/CopyValue";
import { Empty } from "@/components/Empty/Empty";
import { ErrorAlert } from "@/components/ErrorAlert/ErrorAlert";
import { Fact } from "@/components/Fact/Fact";
import { Field, TextField, TextareaField } from "@/components/Field/Field";
import { Modal } from "@/components/Modal/Modal";
import { PageBar } from "@/components/PageBar/PageBar";
import { PasswordInput } from "@/components/PasswordInput/PasswordInput";
import { Spinner } from "@/components/Spinner/Spinner";
import { StatusBadge } from "@/components/StatusBadge/StatusBadge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Checkbox } from "@/components/ui/checkbox";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { api, call, type Schemas } from "@/lib/api/client";
import { describeError } from "@/lib/errors";
import { formatDateTime, formatName, formatRelative } from "@/lib/format";
import {
  LOGIN_TYPE_LABEL,
  PERMISSION_LABEL,
  ROLE_LABEL,
  STATUS_LABEL,
  actionLabel,
} from "@/lib/labels";
import { keys, useMe, usePublicConfig } from "@/lib/queries";
import { useForm } from "@/lib/use-form";
import { PASSWORD_MIN, bioHint, bioSchema, nameSchema, passwordSchema } from "@/lib/validation";

type Detail = Schemas["UserDetail"];
type Permission = Schemas["Permission"];

export function UserDetailView({ id }: { id: string }) {
  const { data: me } = useMe();
  const detail = useQuery({
    queryKey: keys.user(id),
    queryFn: () => call(api.GET("/api/users/{id}", { params: { path: { id } } })),
  });

  const back = (
    <Button nativeButton={false} variant="ghost" size="sm" render={<Link href="/dashboard" />}>
      <ArrowLeftIcon />
      Users
    </Button>
  );

  if (detail.isPending) return <PageSkeleton />;
  if (detail.error) {
    const notFound = describeError(detail.error).code === "not_found";
    return (
      <div className="mx-auto w-full max-w-5xl px-4 py-4 md:px-5">
        {back}
        {notFound ? (
          <Empty
            title="This user does not exist"
            description="They may have been removed, or the link is wrong."
          />
        ) : (
          <ErrorAlert error={detail.error} onRetry={() => void detail.refetch()} />
        )}
      </div>
    );
  }

  const d = detail.data;
  const canManage = me?.permissions.includes("users.manage") ?? false;
  const isSelf = me?.user.id === d.user.id;
  // Only an admin changes an admin's account or makes someone an admin; the API enforces it.
  const canTouch = canManage && (me?.user.role === "admin" || d.user.role !== "admin");

  return (
    <div className="mx-auto w-full max-w-5xl px-4 py-4 md:px-5">
      <div className="pb-2">{back}</div>
      <PageBar
        title={formatName(d.user)}
        description={d.user.email}
        actions={
          <>
            <StatusBadge value={d.user.role} />
            <StatusBadge value={d.user.status} />
          </>
        }
      />
      <div className="grid gap-4 lg:grid-cols-[2fr_1fr]">
        <div className="grid content-start gap-4">
          <Card>
            <CardHeader>
              <CardTitle>Account</CardTitle>
            </CardHeader>
            <CardContent>
              <dl className="grid gap-4 sm:grid-cols-2">
                <Fact label="Email">{d.user.email}</Fact>
                <Fact label="Sign-in method">{LOGIN_TYPE_LABEL[d.user.login_type]}</Fact>
                <Fact label="Created">{formatDateTime(d.user.created_at)}</Fact>
                <Fact label="Last login">
                  {d.user.last_login_at ? formatDateTime(d.user.last_login_at) : "Never"}
                </Fact>
                <Fact label="Id">
                  <CopyValue value={d.user.id} label="Copy id" />
                </Fact>
                <Fact label="Bio" className="sm:col-span-2">
                  {d.user.bio ? <span className="whitespace-pre-line">{d.user.bio}</span> : "—"}
                </Fact>
              </dl>
            </CardContent>
          </Card>
          {canTouch ? <ManageCard detail={d} isSelf={isSelf} /> : null}
          <PermissionsCard detail={d} editable={canTouch && !isSelf} isSelf={isSelf} />
        </div>
        <ActivityCard detail={d} />
      </div>
    </div>
  );
}

function useUpdate(id: string) {
  const client = useQueryClient();
  return useMutation({
    mutationFn: (body: Schemas["UpdateUserRequest"]) =>
      call(api.PATCH("/api/users/{id}", { params: { path: { id } }, body })),
    onSuccess: (data) => {
      client.setQueryData(keys.user(id), data);
      void client.invalidateQueries({ queryKey: ["users"], exact: false, refetchType: "inactive" });
    },
  });
}

function ManageCard({ detail, isSelf }: { detail: Detail; isSelf: boolean }) {
  const { user } = detail;
  const update = useUpdate(user.id);
  const [confirm, setConfirm] = useState<null | {
    status?: Schemas["UserStatus"];
    role?: Schemas["Role"];
  }>(null);
  const [passwordOpen, setPasswordOpen] = useState(false);
  const [codeOpen, setCodeOpen] = useState(false);
  const { data: config } = usePublicConfig();
  const canGiveCode =
    !isSelf &&
    user.login_type !== "code" &&
    (config?.admin_login_methods.includes("code") ?? false);
  const client = useQueryClient();
  const toCode = useMutation({
    mutationFn: () =>
      call(
        api.PUT("/api/users/{id}/login-method", {
          params: { path: { id: user.id } },
          body: { method: "code" },
        }),
      ),
    onSuccess: (data) => {
      client.setQueryData(keys.user(user.id), data);
      setCodeOpen(false);
      toast.success(`${formatName(user)} now signs in with a code sent to their email.`);
    },
    onError: (e) => {
      setCodeOpen(false);
      toast.error(describeError(e).words);
    },
  });
  const saved = { first_name: user.first_name, last_name: user.last_name, bio: user.bio ?? "" };
  const profileForm = useForm(
    saved,
    z.object({ first_name: nameSchema, last_name: nameSchema, bio: bioSchema }),
  );
  const unchanged =
    profileForm.values.first_name.trim() === saved.first_name &&
    profileForm.values.last_name.trim() === saved.last_name &&
    profileForm.values.bio.trim() === saved.bio;

  const saveProfile = profileForm.submit(async (v) => {
    await update.mutateAsync(v);
    toast.success("Profile saved.");
  });

  const apply = (body: { status?: Schemas["UserStatus"]; role?: Schemas["Role"] }) => {
    update.mutate(body, {
      onSuccess: () => {
        setConfirm(null);
        toast.success("Saved.");
      },
      onError: (e) => {
        setConfirm(null);
        toast.error(describeError(e).words);
      },
    });
  };

  // Suspending or removing admin takes access away: ask first.
  const choose = (body: { status?: Schemas["UserStatus"]; role?: Schemas["Role"] }) => {
    const takesAway =
      body.status === "suspended" || (body.role === "user" && user.role === "admin");
    if (takesAway) setConfirm(body);
    else apply(body);
  };

  return (
    <Card>
      <CardHeader>
        <CardTitle>Manage</CardTitle>
        {isSelf ? (
          <CardDescription>You cannot change your own role or status.</CardDescription>
        ) : null}
      </CardHeader>
      <CardContent className="grid gap-5">
        <form className="grid gap-3" onSubmit={(e) => void saveProfile(e)} noValidate>
          <div className="grid gap-3 sm:grid-cols-2">
            <TextField
              label="First name"
              value={profileForm.values.first_name}
              onChange={profileForm.set("first_name")}
              error={profileForm.errors.first_name}
            />
            <TextField
              label="Last name"
              value={profileForm.values.last_name}
              onChange={profileForm.set("last_name")}
              error={profileForm.errors.last_name}
            />
          </div>
          <TextareaField
            label="Bio"
            hint={bioHint(profileForm.values.bio)}
            value={profileForm.values.bio}
            onChange={profileForm.set("bio")}
            error={profileForm.errors.bio}
          />
          {profileForm.formError ? <ErrorAlert error={profileForm.formError} /> : null}
          <div className="flex justify-end">
            <Button type="submit" variant="outline" disabled={profileForm.pending || unchanged}>
              Save
            </Button>
          </div>
        </form>

        <div className="grid gap-4 sm:grid-cols-2">
          <Field label="Role">
            {(control) => (
              <Select
                items={ROLE_LABEL}
                value={user.role}
                disabled={isSelf || update.isPending}
                onValueChange={(v) => choose({ role: v as Schemas["Role"] })}
              >
                <SelectTrigger id={control.id} className="w-full">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {Object.entries(ROLE_LABEL).map(([value, label]) => (
                    <SelectItem key={value} value={value}>
                      {label}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            )}
          </Field>
          <Field label="Status">
            {(control) => (
              <Select
                items={STATUS_LABEL}
                value={user.status}
                disabled={isSelf || update.isPending}
                onValueChange={(v) => choose({ status: v as Schemas["UserStatus"] })}
              >
                <SelectTrigger id={control.id} className="w-full">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {Object.entries(STATUS_LABEL).map(([value, label]) => (
                    <SelectItem key={value} value={value}>
                      {label}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            )}
          </Field>
        </div>

        <div className="flex flex-wrap gap-2">
          <Button variant="outline" onClick={() => setPasswordOpen(true)}>
            <KeyRoundIcon />
            Set a new password
          </Button>
          {canGiveCode ? (
            <Button variant="outline" onClick={() => setCodeOpen(true)}>
              <MailIcon />
              Sign in with an email code
            </Button>
          ) : null}
        </div>
      </CardContent>

      <ConfirmModal
        open={confirm !== null}
        onOpenChange={(o) => !o && setConfirm(null)}
        title={
          confirm?.status === "suspended"
            ? `Suspend ${formatName(user)}?`
            : `Remove admin from ${formatName(user)}?`
        }
        description={
          confirm?.status === "suspended"
            ? "They are signed out everywhere and cannot sign in until you make them active again."
            : "They lose every permission that comes with the admin role."
        }
        confirmLabel={confirm?.status === "suspended" ? "Suspend" : "Remove admin"}
        destructive
        pending={update.isPending}
        onConfirm={() => confirm && apply(confirm)}
      />
      <ConfirmModal
        open={codeOpen}
        onOpenChange={setCodeOpen}
        title={`Switch ${formatName(user)} to an email code?`}
        description="Their current sign-in method stops working and they are signed out everywhere."
        confirmLabel="Switch"
        destructive
        pending={toCode.isPending}
        onConfirm={() => toCode.mutate()}
      />
      <SetPasswordModal user={user} open={passwordOpen} onOpenChange={setPasswordOpen} />
    </Card>
  );
}

function SetPasswordModal({
  user,
  open,
  onOpenChange,
}: {
  user: Schemas["UserDto"];
  open: boolean;
  onOpenChange: (o: boolean) => void;
}) {
  const client = useQueryClient();
  const form = useForm({ password: "" }, z.object({ password: passwordSchema }));
  const onSubmit = form.submit(async (v) => {
    await call(api.PUT("/api/users/{id}/password", { params: { path: { id: user.id } }, body: v }));
    await client.invalidateQueries({ queryKey: keys.user(user.id) });
    toast.success(`Password set. ${formatName(user)} is signed out everywhere.`);
    form.reset();
    onOpenChange(false);
  });
  return (
    <Modal
      open={open}
      onOpenChange={(o) => {
        if (!o) form.reset();
        onOpenChange(o);
      }}
      title={`New password for ${formatName(user)}`}
      description={
        user.login_type === "password"
          ? "They are signed out everywhere and log in with this password."
          : "They are signed out everywhere and sign in with this password from now on, instead of their current method."
      }
    >
      <form className="grid gap-4" onSubmit={(e) => void onSubmit(e)} noValidate>
        {form.formError ? <ErrorAlert error={form.formError} /> : null}
        <Field
          label="New password"
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
        <div className="flex justify-end gap-2">
          <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
            Cancel
          </Button>
          <Button type="submit" disabled={form.pending}>
            {form.pending ? <Spinner /> : null}
            Set password
          </Button>
        </div>
      </form>
    </Modal>
  );
}

const ALL_PERMISSIONS = Object.keys(PERMISSION_LABEL) as Permission[];

function PermissionsCard({
  detail,
  editable,
  isSelf,
}: {
  detail: Detail;
  editable: boolean;
  isSelf: boolean;
}) {
  const client = useQueryClient();
  const id = detail.user.id;
  const [draft, setDraft] = useState<Permission[] | null>(null);
  const saved = detail.grants.map((g) => g.permission);
  const grants = draft ?? saved;
  const fromRole = detail.user.role === "admin";
  const save = useMutation({
    mutationFn: (permissions: Permission[]) =>
      call(
        api.PUT("/api/users/{id}/permissions", { params: { path: { id } }, body: { permissions } }),
      ),
    onSuccess: (data) => {
      client.setQueryData(keys.user(id), data);
      setDraft(null);
      toast.success("Permissions saved.");
    },
    onError: (e) => toast.error(describeError(e).words),
  });
  const changed = draft !== null && [...draft].sort().join() !== [...saved].sort().join();

  return (
    <Card>
      <CardHeader>
        <CardTitle>Permissions</CardTitle>
        <CardDescription>
          {fromRole
            ? "Admins have every permission through their role."
            : isSelf
              ? "You cannot change your own permissions."
              : "Grants on top of the user role."}
        </CardDescription>
      </CardHeader>
      <CardContent className="grid gap-3">
        {ALL_PERMISSIONS.map((p) => {
          const has = detail.permissions.includes(p);
          const granted = grants.includes(p);
          const grant = detail.grants.find((g) => g.permission === p);
          return (
            <div key={p} className="flex items-start gap-3">
              <Checkbox
                id={`perm-${p}`}
                checked={fromRole ? has : granted}
                disabled={!editable || fromRole || save.isPending}
                onCheckedChange={(checked) =>
                  setDraft(checked ? [...grants, p] : grants.filter((g) => g !== p))
                }
              />
              <div className="grid gap-0.5">
                <Label htmlFor={`perm-${p}`}>{PERMISSION_LABEL[p].label}</Label>
                <p className="text-xs text-muted-foreground">{PERMISSION_LABEL[p].hint}</p>
                {grant && !fromRole ? (
                  <p className="text-xs text-muted-foreground">
                    Granted by {grant.granted_by ?? "a removed account"},{" "}
                    {formatDateTime(grant.granted_at)}
                  </p>
                ) : null}
              </div>
            </div>
          );
        })}
        {editable && !fromRole ? (
          <div className="flex justify-end gap-2">
            {changed ? (
              <Button variant="ghost" onClick={() => setDraft(null)}>
                Undo
              </Button>
            ) : null}
            <Button disabled={!changed || save.isPending} onClick={() => save.mutate(grants)}>
              {save.isPending ? <Spinner /> : null}
              Save permissions
            </Button>
          </div>
        ) : null}
      </CardContent>
    </Card>
  );
}

function ActivityCard({ detail }: { detail: Detail }) {
  return (
    <Card className="content-start">
      <CardHeader>
        <CardTitle>Activity</CardTitle>
      </CardHeader>
      <CardContent>
        {detail.audit.length === 0 ? (
          <p className="text-muted-foreground">Nothing recorded yet.</p>
        ) : (
          <ol className="grid gap-3">
            {detail.audit.map((e) => (
              <li key={e.id} className="grid gap-0.5">
                <p className="font-medium">{actionLabel(e.action)}</p>
                <p className="text-xs text-muted-foreground">
                  {e.actor_email ?? "System"} ·{" "}
                  <time dateTime={e.created_at}>{formatRelative(e.created_at)}</time>
                </p>
                <Changes data={e.data} />
              </li>
            ))}
          </ol>
        )}
      </CardContent>
    </Card>
  );
}

/** The before/after of an "account changed" event, one line per field. */
function Changes({ data }: { data: unknown }) {
  const changes = (data as { changes?: Record<string, { before: unknown; after: unknown }> })
    ?.changes;
  if (!changes) return null;
  return (
    <ul className="text-xs text-muted-foreground">
      {Object.entries(changes).map(([field, c]) => (
        <li key={field}>
          {field}: {String(c.before)} → {String(c.after)}
        </li>
      ))}
    </ul>
  );
}
