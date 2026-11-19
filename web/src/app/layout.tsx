import type { Metadata, Viewport } from "next";

import { Providers } from "@/components/Providers/Providers";

import "./globals.css";

export const metadata: Metadata = {
  title: { default: "App", template: "%s · App" },
  description: "A full-stack app on Rust, Next.js and PostgreSQL.",
};

export const viewport: Viewport = {
  themeColor: [
    { media: "(prefers-color-scheme: light)", color: "white" },
    { media: "(prefers-color-scheme: dark)", color: "black" },
  ],
};

export default function RootLayout({ children }: LayoutProps<"/">) {
  return (
    // next-themes sets the theme class before React hydrates; the mismatch is expected.
    <html lang="en" suppressHydrationWarning>
      <body>
        <Providers>{children}</Providers>
      </body>
    </html>
  );
}
