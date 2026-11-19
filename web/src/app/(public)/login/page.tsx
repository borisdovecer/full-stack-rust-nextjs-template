import type { Metadata } from "next";

import { AuthCard } from "@/components/AuthCard/AuthCard";
import { LoginForm } from "@/components/LoginForm/LoginForm";

export const metadata: Metadata = { title: "Log in" };

export default function LoginPage() {
  return (
    <AuthCard title="Log in" description="Use the email and password of your account.">
      <LoginForm />
    </AuthCard>
  );
}
