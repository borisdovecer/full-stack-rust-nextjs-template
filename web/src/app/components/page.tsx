import type { Metadata } from "next";
import { notFound } from "next/navigation";

import { ComponentsShowcase } from "@/components/ComponentsShowcase/ComponentsShowcase";

export const metadata: Metadata = { title: "Components" };

/** Development only: a production build answers 404. */
export default function ComponentsPage() {
  if (process.env.NODE_ENV !== "development") notFound();
  return <ComponentsShowcase />;
}
