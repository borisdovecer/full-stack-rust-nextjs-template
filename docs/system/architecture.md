# Architecture

Last checked against the code on 2026-11-19.

How the parts fit together, what runs where, and where state lives.

## Processes

```
browser ──> Caddy (:443) ──┬──> Next.js  web/            127.0.0.1:3000   pages and components
                           └──> /api/*   services/api    127.0.0.1:8080   every rule and all data
                                              │
                                              └──> PostgreSQL           127.0.0.1:5432
```

| Process | Code | Does |
|---|---|---|
| API | `services/api` (binary `app-api`) | HTTP API, sessions, users, permissions, help docs, admin CLI, migrations |
| Web | `web/` (Next.js) | Renders the UI. No route handlers, no server actions, no database, no secrets |
| Caddy | `deploy/Caddyfile` | TLS, one public host name, sends `/api/*` to the API and the rest to the web |
| PostgreSQL | `migrations/` | The only store. Nothing important lives in memory |

In development there is no Caddy: `next.config.ts` rewrites `/api/*` to the API, so the browser still sees one origin.

## Ports

The registry: a new service takes the next free port here (`docs/rules/architecture.md`).

| Port | Service | Protocol | Variable |
|---|---|---|---|
| `3000` | web | HTTP | `next dev` / `deploy/app-web.service` |
| `8080` | `api` | HTTP | `API_BIND` |
| `50051` | next free gRPC port | | |

## Between services

There is one service today, so no service calls another. When there are more, they talk over gRPC only, the browser still reaches only `api`, and the call graph is drawn here.

## Why one origin

The session is an `HttpOnly` cookie set by the API. With the pages and `/api/*` on the same host, the cookie goes along on every API call, there is no CORS to configure, and `SameSite=Lax` plus the JSON-only rule for writes stops cross-site request forgery.

## Rust workspace

| Crate | Path | Holds |
|---|---|---|
| `app-core` | `crates/core` | Config loading and validation, the error type and codes, extractors, the request id |
| `app-db` | `crates/db` | The pool, embedded migrations and shared query helpers. Tables and their SQL live in the service that owns them |
| `app-auth` | `crates/auth` | Password hashing, sign-in codes, Sign-In-With-Solana checks, session tokens, the permission list. No I/O |
| `app-mail` | `crates/mail` | Sends email through Resend, or to the log in development |
| `app-api` | `services/api` | Routes, middleware, OpenAPI, help docs, background clean-up, CLI. One folder per scope (`auth`, `me`, `users`, `help`, `system`); the layout is in `docs/rules/architecture.md` |

A new binary (a worker, a scheduler) is a new folder in `services/` that uses the crates.

## A request through the API

Middleware, outermost first:

1. **Request id**: taken from `x-request-id` when it is short and plain, otherwise made. Returned in the response, put on every log line and every error body.
2. **Compression**, **panic catcher**.
3. **Error normaliser**: any error answer that is not already JSON (unknown route, wrong method, timeout, body too large, panic) becomes the standard error body.
4. **Trace**: one log line per response.
5. **Timeout** (30 s) and **body limit** (64 KiB).
6. **JSON-only writes**: a non-GET request without `Content-Type: application/json` answers `415`.
7. **Session**: reads the cookie, loads the session and its user, extends it when due, clears a cookie that no longer matches. When the lookup fails (database down), the request goes on without a user: public routes still answer and signed-in routes answer `503`. See [auth](auth.md).
8. The route. Routes declare what they need with extractors: `SessionUser`, `CurrentUser`, `Require<UsersRead>`.

## Startup

`app-api` (or `app-api serve`):

1. Loads `.env`, then validates the config. A bad value stops it with the variable's name.
2. Starts Sentry when `SENTRY_DSN` is set, and logging.
3. Connects to the database and checks migrations. Pending, failed or edited migrations stop it.
4. Starts the hourly clean-up task and listens. Every instance runs the timer; an advisory lock lets one of them clean up at a time, in batches.

On SIGINT or SIGTERM it stops taking connections and finishes the requests in flight.

## Where state lives

Everything is in PostgreSQL: users, sessions, login attempts, permissions, preferences, the audit log, mailed sign-in codes, wallet challenges, Google and GitHub identities, and provider sign-ups waiting for the registration form. The API holds no state between requests, so it can restart at any time and more than one can run behind the proxy.
