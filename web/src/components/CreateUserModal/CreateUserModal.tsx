"use client";

import { useQueryClient } from "@tanstack/react-query";
import type { Route } from "next";
import { useRouter } from "next/navigation";
import { toast } from "sonner";
import { z } from "zod";

import { ErrorAlert } from "@/components/ErrorAlert/ErrorAlert";
import { Field, TextField, TextareaField } from "@/components/Field/Field";
import { Modal } from "@/components/Modal/Modal";
import { PasswordInput } from "@/components/PasswordInput/PasswordInput";
import { Spinner } from "@/components/Spinner/Spinner";
import { Button } from "@/components/ui/button";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { api, call, type Schemas } from "@/lib/api/client";
import { LOGIN_TYPE_LABEL, ROLE_LABEL } from "@/lib/labels";
import { useMe, usePublicConfig } from "@/lib/queries";
import { useForm } from "@/lib/use-form";
import { formatName } from "@/lib/format";
import {
  PASSWORD_MIN,
  bioHint,
  bioSchema,
  emailSchema,
  nameSchema,
  passwordSchema,
} from "@/lib/validation";

const schema = z
  .object({
    first_name: nameSchema,
    last_name: nameSchema,
    bio: bioSchema,
    email: emailSchema,
    method: z.string(),
    password: z.string(),
  })
  .superRefine((v, ctx) => {
    if (v.method !== "password") return;
    const problem = passwordSchema.safeParse(v.password).error?.issues[0];
    if (problem) ctx.addIssue({ code: "custom", path: ["password"], message: problem.message });
  });

export function CreateUserModal({
  open,
  onOpenChange,
}: {
  open: boolean;
  onOpenChange: (o: boolean) => void;
}) {
  const client = useQueryClient();
  const router = useRouter();
  const { data: me } = useMe();
  const { data: config } = usePublicConfig();
  const methods = config?.admin_login_methods ?? ["password"];
  const form = useForm(
    {
      first_name: "",
      last_name: "",
      bio: "",
      email: "",
      method: "password",
      password: "",
      role: "user",
    },
    schema,
  );
  const usesPassword = form.values.method === "password";
  const isAdmin = me?.user.role === "admin";

  const onSubmit = form.submit(async (v) => {
    const created = await call(
      api.POST("/api/users", {
        body: {
          first_name: v.first_name,
          last_name: v.last_name,
          bio: v.bio.trim() || null,
          email: v.email,
          method: v.method as Schemas["LoginType"],
          password: usesPassword ? v.password : null,
          role: v.role as Schemas["Role"],
        },
      }),
    );
    await client.invalidateQueries({ queryKey: ["users"] });
    toast.success(`${formatName(created.user)} can now log in.`);
    form.reset();
    onOpenChange(false);
    router.push(`/users/${created.user.id}` as Route);
  });

  return (
    <Modal
      open={open}
      onOpenChange={(o) => {
        if (!o) form.reset();
        onOpenChange(o);
      }}
      title="Create a user"
      description={
        usesPassword
          ? "Give them the password yourself."
          : "They sign in with a code sent to their email."
      }
    >
      <form className="grid gap-4" onSubmit={(e) => void onSubmit(e)} noValidate>
        {form.formError ? <ErrorAlert error={form.formError} /> : null}
        <div className="grid gap-4 sm:grid-cols-2">
          <TextField
            label="First name"
            value={form.values.first_name}
            onChange={form.set("first_name")}
            error={form.errors.first_name}
          />
          <TextField
            label="Last name"
            value={form.values.last_name}
            onChange={form.set("last_name")}
            error={form.errors.last_name}
          />
        </div>
        <TextField
          label="Email"
          type="email"
          value={form.values.email}
          onChange={form.set("email")}
          error={form.errors.email}
        />
        {methods.length > 1 ? (
          <Field label="Sign-in method">
            {(control) => (
              <Select
                items={Object.fromEntries(methods.map((m) => [m, LOGIN_TYPE_LABEL[m]]))}
                value={form.values.method}
                onValueChange={(v) => form.setValues((s) => ({ ...s, method: String(v) }))}
              >
                <SelectTrigger id={control.id} className="w-full">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {methods.map((m) => (
                    <SelectItem key={m} value={m}>
                      {LOGIN_TYPE_LABEL[m]}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            )}
          </Field>
        ) : null}
        {usesPassword ? (
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
        {isAdmin ? (
          <Field label="Role">
            {(control) => (
              <Select
                items={ROLE_LABEL}
                value={form.values.role}
                onValueChange={(v) => form.setValues((s) => ({ ...s, role: String(v) }))}
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
        ) : null}
        <div className="flex justify-end gap-2 pt-2">
          <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
            Cancel
          </Button>
          <Button type="submit" disabled={form.pending}>
            {form.pending ? <Spinner /> : null}
            Create user
          </Button>
        </div>
      </form>
    </Modal>
  );
}
