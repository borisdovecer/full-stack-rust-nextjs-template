import type { NextConfig } from "next";

// The web has no server logic. `/api/*` belongs to the Rust API: in production Caddy sends it
// there directly; this rewrite does the same for `npm run dev` and a bare `next start`.
// API_URL is read when the config loads: at `next dev` start, and at build time for a build.
const apiUrl = process.env.API_URL ?? "http://127.0.0.1:8080";

const nextConfig: NextConfig = {
  reactCompiler: true,
  typedRoutes: true,
  poweredByHeader: false,
  // Deploy builds set NEXT_OUTPUT=standalone: a self-contained server in .next/standalone, so the
  // server needs no npm install. Local builds stay normal so `npm start` works as usual.
  output: process.env.NEXT_OUTPUT === "standalone" ? "standalone" : undefined,
  async rewrites() {
    return [{ source: "/api/:path*", destination: `${apiUrl}/api/:path*` }];
  },
};

export default nextConfig;
