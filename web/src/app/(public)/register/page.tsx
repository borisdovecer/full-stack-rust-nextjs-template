import type { Metadata } from "next";

import { AuthCard } from "@/components/AuthCard/AuthCard";
import { RegisterForm } from "@/components/RegisterForm/RegisterForm";

export const metadata: Metadata = { title: "Register" };

export default function RegisterPage() {
  return (
    <AuthCard title="Create an account">
      <RegisterForm />
    </AuthCard>
  );
}
