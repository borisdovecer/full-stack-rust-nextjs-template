import type { Metadata } from "next";

import { SettingsView } from "@/components/SettingsView/SettingsView";

export const metadata: Metadata = { title: "Settings" };

export default function SettingsPage() {
  return <SettingsView />;
}
