import type { Metadata } from "next";

import { HelpView } from "@/components/HelpView/HelpView";

export const metadata: Metadata = { title: "Help" };

export default async function HelpSlugPage({ params }: PageProps<"/help/[slug]">) {
  const { slug } = await params;
  return <HelpView slug={slug} />;
}
