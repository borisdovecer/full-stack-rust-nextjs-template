import { AppShell } from "@/components/AppShell/AppShell";
import { AuthGate } from "@/components/AuthGate/AuthGate";

export default function SignedInLayout({ children }: LayoutProps<"/">) {
  return (
    <AppShell>
      <AuthGate>{children}</AuthGate>
    </AppShell>
  );
}
