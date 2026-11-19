"use client";

import { usePublicConfig } from "@/lib/queries";

/** The app's name from the API's settings (`config/app.toml`). */
export function AppName() {
  const { data } = usePublicConfig();
  return <>{data?.app_name ?? "App"}</>;
}
