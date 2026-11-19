import type { Metadata } from "next";

import { AuthCard } from "@/components/AuthCard/AuthCard";
import { AuthGate } from "@/components/AuthGate/AuthGate";
import { PendingView } from "@/components/PendingView/PendingView";

export const metadata: Metadata = { title: "Waiting for approval" };

export default function PendingPage() {
  return (
    <AuthGate allowPending>
      <AuthCard title="Waiting for approval">
        <PendingView />
      </AuthCard>
    </AuthGate>
  );
}
