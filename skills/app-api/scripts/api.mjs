#!/usr/bin/env node
// Calls the app's API as the account in the project's .env (ADMIN_EMAIL, ADMIN_PASSWORD),
// at http://API_BIND, or at APP_API_URL when that is set in the environment.
//
//   node api.mjs METHOD PATH [JSON_BODY]
//   node api.mjs GET /api/users?search=ana
//   node api.mjs PATCH /api/users/<id> '{"status":"suspended"}'
//
// Signs in when there is no session or it has ended, keeps the session cookie in
// ~/.config/app/<project>-<hash>/cookie (mode 600), and prints the status line and the JSON answer.
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, realpathSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { basename, dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = realpathSync(resolve(dirname(fileURLToPath(import.meta.url)), "../../.."));
const envFile = join(root, ".env");
// One folder per checkout, so two projects made from the template never share state.
const tag = createHash("sha256").update(root).digest("hex").slice(0, 8);
const dir = join(homedir(), ".config/app", `${basename(root)}-${tag}`);
const cookieFile = join(dir, "cookie");

const [method = "GET", path = "/api/health", body] = process.argv.slice(2);
if (!path.startsWith("/api/")) fail("PATH must start with /api/");
if (!existsSync(envFile)) fail(`no ${envFile}. Run scripts/setup.sh`);

const env = readEnv(readFileSync(envFile, "utf8"));
const config = {
  apiUrl: (process.env.APP_API_URL || `http://${env.API_BIND || "127.0.0.1:8080"}`).replace(/\/$/, ""),
  email: env.ADMIN_EMAIL ?? "",
  password: env.ADMIN_PASSWORD ?? "",
};
let cookie = existsSync(cookieFile) ? readFileSync(cookieFile, "utf8").trim() : "";

// KEY=value, KEY='value' (literal) and KEY="value"; # starts a comment line.
function readEnv(text) {
  const out = {};
  for (const raw of text.split(/\r?\n/)) {
    const line = raw.trim();
    const m = line.match(/^(?:export\s+)?([A-Za-z_][A-Za-z0-9_]*)=(.*)$/);
    if (!m) continue;
    let value = m[2].trim();
    const q = value[0];
    if ((q === "'" || q === '"') && value.length > 1 && value.endsWith(q)) value = value.slice(1, -1);
    else value = value.replace(/\s+#.*$/, "");
    out[m[1]] = value;
  }
  return out;
}

function fail(message) {
  console.error(`error: ${message}`);
  process.exit(2);
}

async function send(m, p, b) {
  const res = await fetch(config.apiUrl + p, {
    method: m,
    headers: {
      "content-type": "application/json",
      ...(cookie ? { cookie } : {}),
    },
    body: b === undefined ? undefined : typeof b === "string" ? b : JSON.stringify(b),
  }).catch((e) => fail(`cannot reach ${config.apiUrl}: ${e.cause?.code ?? e.message}`));
  const set = res.headers.getSetCookie?.() ?? [];
  for (const c of set) {
    const pair = c.split(";")[0];
    if (pair.endsWith("=")) cookie = "";
    else cookie = pair;
    mkdirSync(dir, { recursive: true, mode: 0o700 });
    writeFileSync(cookieFile, cookie, { mode: 0o600 });
  }
  return res;
}

async function login() {
  const missing = ["ADMIN_EMAIL", "ADMIN_PASSWORD"].filter((k) => !env[k]);
  if (missing.length) fail(`${missing.join(" and ")} empty in ${envFile}. Fill ${missing.length > 1 ? "them" : "it"} in, or run scripts/setup.sh on a new database`);
  const res = await send("POST", "/api/auth/login", { email: config.email, password: config.password });
  if (!res.ok) {
    const text = await res.text();
    fail(`sign-in failed (${res.status}): ${text}. Check ADMIN_EMAIL and ADMIN_PASSWORD in ${envFile}`);
  }
}

if (method !== "GET" && body !== undefined) {
  try {
    JSON.parse(body);
  } catch {
    fail("JSON_BODY is not valid JSON");
  }
}

const isAuthRoute = path.startsWith("/api/auth/login") || path === "/api/health" || path === "/api/config";
if (!cookie && !isAuthRoute) await login();
let res = await send(method, path, method === "GET" ? undefined : (body ?? "{}"));
if (res.status === 401 && !isAuthRoute) {
  await login();
  res = await send(method, path, method === "GET" ? undefined : (body ?? "{}"));
}

const text = await res.text();
console.log(`${res.status} ${res.statusText}`);
try {
  console.log(JSON.stringify(JSON.parse(text), null, 2));
} catch {
  if (text) console.log(text);
}
process.exit(res.ok ? 0 : 1);
