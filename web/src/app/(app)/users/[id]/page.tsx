import type { Metadata } from "next";

import { AuthGate } from "@/components/AuthGate/AuthGate";
import { UserDetailView } from "@/components/UserDetailView/UserDetailView";

export const metadata: Metadata = { title: "User" };

export default async function UserPage({ params }: PageProps<"/users/[id]">) {
  const { id } = await params;
  return (
    <AuthGate permission="users.read">
      <UserDetailView id={id} />
    </AuthGate>
  );
}
