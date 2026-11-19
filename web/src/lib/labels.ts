import type { Schemas } from "@/lib/api/client";

/** Words for values the API sends as codes. */
export const ROLE_LABEL: Record<Schemas["Role"], string> = { admin: "Admin", user: "User" };

export const STATUS_LABEL: Record<Schemas["UserStatus"], string> = {
  active: "Active",
  pending: "Pending",
  suspended: "Suspended",
};

export const LOGIN_TYPE_LABEL: Record<Schemas["LoginType"], string> = {
  password: "Password",
  wallet: "Wallet",
  code: "Email code",
  external: "Google or GitHub",
};

/** How an account signs in, as a sentence about "you". */
export const LOGIN_TYPE_SENTENCE: Record<Schemas["LoginType"], string> = {
  password: "You sign in with your email and a password.",
  wallet: "You sign in with your email and your wallet.",
  code: "You sign in with a code sent to your email.",
  external: "You sign in with Google or GitHub.",
};

/** Names of the `external` providers, as `GET /api/config` lists them. */
export const PROVIDER_LABEL: Record<string, string> = { google: "Google", github: "GitHub" };

export const PERMISSION_LABEL: Record<Schemas["Permission"], { label: string; hint: string }> = {
  "users.read": { label: "See users", hint: "The user list and each user's details." },
  "users.manage": {
    label: "Manage users",
    hint: "Create users, change their role, status, permissions and password.",
  },
};

const ACTION_LABEL: Record<string, string> = {
  "user.created": "Account created",
  "user.registered": "Registered",
  "user.updated": "Account changed",
  "user.password_set": "Password set by an admin",
  "user.password_changed": "Password changed",
  "user.login_method_changed": "Sign-in method changed",
  "user.permissions_changed": "Permissions changed",
};

/** An audit action in words; an action this build does not know shows as it is. */
export function actionLabel(action: string): string {
  return ACTION_LABEL[action] ?? action;
}
