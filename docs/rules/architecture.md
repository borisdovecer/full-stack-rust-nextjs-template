# Architecture rules

How the workspace is laid out: services, crates, the inside of a service, ports, and how services talk. What runs today: `docs/system/architecture.md`. SQL and migrations: `database.md`.

## Workspace

```
proto/                    one <service>.proto per service that serves gRPC
crates/                   libraries: no main, no ports, no routes
  core/  db/  auth/  mail/
  proto/                  generated gRPC code, built from proto/ on cargo build
  clients/                one typed client per gRPC service, for its callers
services/                 binaries: one folder per service
  api/                    the public HTTP edge
  <name>/                 an internal service: gRPC only
migrations/               one history for the one database
config/<service>.toml     non-secret settings, one file per service (the API's is app.toml)
deploy/app-<service>.service
```

`crates/proto`, `crates/clients` and `proto/` are created with the first service that serves gRPC.

## Services and crates

**A service** is a binary in `services/<name>`. It owns:
- its tables
- its ports
- its config file and `.env` keys
- its systemd unit
- its `docs/system/<name>.md` page

**A crate** is a library:
- It never depends on a service, and has no ports, routes or `AppState`.
- Its errors use `thiserror`.
- It has one purpose, named in its `description`.
- Crates never depend on each other in a circle. `core` sits at the bottom, and `proto` depends on nothing of ours.

**Where code goes:**
- Code that one service uses stays in that service, however generic it looks.
- It moves to a crate when a second service needs it, and is deleted from the first in the same change. No copies.
- The same rule holds inside a service, between a scope and `shared/`.

| Code | Crate |
|---|---|
| Pure logic with no I/O (hashing, signatures, maths) | its own crate, like `app-auth` |
| Plumbing every service needs: config, errors, request id, telemetry, server start-up | `app-core` |
| The pool, the migration runner, query helpers every service uses | `app-db` |
| Calling another service | `app-clients` |
| Generated gRPC code | `app-proto` |

## The database

- There is one database with one migration history in `migrations/`, applied by `app-api migrate`.
- Every table has exactly one owner service. The owner is named in the table's first migration and in `docs/system/database.md`.
- Only the owner reads or writes a table. Any other service asks the owner over gRPC.
- A service's SQL lives in the `repository.rs` of the scope that owns the table, so a service cannot compile a query against a table it does not own.
- `crates/db` holds only the pool, the migration runner and shared helpers.
- A table belongs to the lowest scope in the import order that needs it, so every scope that uses it can reach it. A scope with several tables has a `repository/` folder, one file per table. Another scope reaches them through the owner's re-export: `crate::auth::repository::users::find(...)`.
- A table every scope writes, like `audit_events`, lives in `shared/repository/`.

## Inside a service

Every service has the same layout. Create only the parts it needs.

```
src/
  main.rs  lib.rs  state.rs  server.rs  telemetry.rs  cli.rs
  app.rs          HTTP router: merges every scope's routes(); only in a service that serves HTTP
  grpc/           gRPC server: only in a service that serves gRPC
    mod.rs        builds the server with the token interceptor and the health service
    server.rs     the generated trait: proto in, call a scope's service, proto out
    convert.rs    proto ↔ domain types
  <scope>/        one folder per feature
  shared/         code more than one scope uses: no routes, no imports from a scope
  jobs/           background work on a timer
```

A scope is a feature. In a service that serves HTTP, its folder is named for its URL prefix, which is also its OpenAPI tag (`users/` serves `/api/users/*`, tag `users`). A sub-scope serves a sub-path (`auth/oauth/`).

| File | Holds | Never |
|---|---|---|
| `mod.rs` | private `mod` lines, `pub fn routes() -> OpenApiRouter<AppState>`, `pub use` of what other scopes may use | logic |
| `handlers.rs` | HTTP: read the request, validate, call the service, build the DTO, set cookies | logic another handler or scope needs |
| `dto.rs` | HTTP request and response types, with `ToSchema` or `IntoParams` | database rows; they are never serialised |
| `service.rs`, or `service/` with two or more | business logic: plain inputs, returns `Result<T, _>`, may open a transaction | axum or tonic types, proto messages |
| `repository.rs` | this scope's SQL, for the tables it owns | business rules |
| `validators.rs` | this scope's field checks, built on `shared::validation::FieldErrors` | |
| `errors.rs` | an internal error enum that converts into `AppError` | a new error shape on the wire: every error is an `ErrorCode` |

- **Transports are adapters.** HTTP `handlers.rs` and gRPC `grpc/server.rs` both call the same `service.rs`. That is why a service function takes no transport types.
- **When to add `service.rs`:** when logic is reused, a handler grows past about 40 lines, or it changes several tables or opens a transaction. A plain read stays in the handler.
- **HTTP glue** that is neither a handler nor a service (cookies, an extractor) goes in a file named for what it holds: `auth/session.rs`, `auth/oauth/cookies.rs`.
- **Imports point one way:** `shared` and `state` → `auth` → the other scopes. A scope never imports another feature scope. What two scopes need moves to `shared/`, or to `auth/` when it is about sessions or signing in.
- **Other scopes import only what a scope's `mod.rs` re-exports:** `crate::auth::SessionUser`, never `crate::auth::session::SessionUser`. Inside a scope, use `super::`.
- **`state.rs`** holds long-lived handles: the pool, config, mailer, clients for other services. It may name a scope's handle type and holds no logic.

## Who talks to whom

```
browser ──> Caddy ──> /api/*  ──> api (HTTP, the only edge) ──gRPC──> internal services
                  └─> the rest ──> web
```

- **`api` is the only service the browser reaches.** It holds the sessions, checks permissions, and calls internal services over gRPC. The web never calls another service, and Caddy never routes to one.
- **Services talk to each other only over gRPC.** HTTP is for the browser.
- **No cycles:** if A calls B, B never calls A. When B needs to tell A something, A asks, or subscribes to a stream from B. The call graph is drawn in `docs/system/architecture.md`.

## Ports

| Port | Who |
|---|---|
| `3000` | web |
| `8080` | `api`, HTTP |
| `50051` and up | gRPC, one port per service, in the order services are added |

- The table in `docs/system/architecture.md` is the registry. A new service takes the next free port there in the same change, and no port is ever reused for another service.
- Every address comes from the environment:
  - `<SERVICE>_BIND` for HTTP (the API's is `API_BIND`)
  - `<SERVICE>_GRPC_BIND` for gRPC
  - `<SERVICE>_GRPC_URL` in each caller
- **Internal ports listen on loopback.** When services sit on several hosts, they listen on a private network, never a public address. Production refuses a public bind, as `API_BIND` does today.

## gRPC and proto

**Files**
- One file per service: `proto/<service>.proto`, with `package app.<service>.v1;` and one `service <Service>`.
- Each rpc has its own `<Rpc>Request` and `<Rpc>Response`, even when empty, so fields can be added later.
- Enums start with `<ENUM>_UNSPECIFIED = 0`.
- Ids are UUID strings. Times are `google.protobuf.Timestamp`. Money and amounts are decimal strings, never floats.
- Every rpc and field has a comment: what it is, its unit, when it is empty.

**Compatibility**
- Never reuse or renumber a field. A removed field's number and name become `reserved`.
- Adding a field or an rpc is safe. Changing a field's type or meaning is not.
- A breaking change is a new package `app.<service>.v2`, served next to `v1` until every caller has moved, then `v1` is removed.
- The service that serves an rpc deploys before its callers.

**Code generation**
- `crates/proto/build.rs` compiles every file in `proto/` with `protox` and `tonic-build`.
- There is no separate step and nothing to install: `cargo build` regenerates the code.
- Each package is a module of `app-proto`.

**Clients**
- `crates/clients/src/<service>.rs` wraps the generated client for its callers.
- It adds the metadata, a deadline and the error mapping, and takes and returns domain types.
- The service that serves the rpc maintains its client in the same change.

**Generated types stay at the edge:** proto messages never go past a service's `grpc/` folder or `app-clients`.

**Metadata on every call**

| Key | What | Who checks |
|---|---|---|
| `x-service-token` | the caller's own token | the server's interceptor, in constant time |
| `x-request-id` | the id of the HTTP request that started it | logged by every service, so one id follows a request everywhere |
| `x-acting-user` | the id of the user the edge signed in, when there is one | the callee, for ownership of per-user data |

- **Tokens:**
  - Each caller has its own token: `INTERNAL_TOKEN_<CALLER>` in the callee's `.env`, and `<CALLEE>_TOKEN` in the caller's.
  - A server with no tokens set refuses every call.
  - The log names the caller, and one caller can be revoked alone.
- **Permissions** are checked at the edge, with the same extractors as today. The callee trusts the edge's permission check, but checks that per-user data belongs to `x-acting-user`.

**Errors**
- A failure is a `tonic::Status` with a fitting gRPC code. The `ErrorCode` name goes in the `x-error-code` metadata, with its `details` as JSON in `x-error-details` when there are any.
- `app-clients` turns it back into `AppError`, so the browser gets the same error body as from any route.
- There is no second list of error codes.

**Calls**
- Every call has a deadline: 5 s unless the rpc's comment says otherwise.
- Only idempotent reads are retried, once.
- Rpcs are unary. A server stream is only for a live feed.
- Every internal service answers the standard health check (`grpc.health.v1`). The edge's `/api/health` lists each service it calls.

## Checklists

**A new scope**
1. Create `src/<scope>/` with `mod.rs` and its `routes()`, and add the module to `lib.rs`.
2. Add one `.merge(<scope>::routes())` to `api_routes()` and one tag to `ApiDoc` in `app.rs`.
3. Add the tables, with a migration that names their owner (`database.md`), and their queries in `<scope>/repository.rs`.
4. Add any new `ErrorCode` in `crates/core/src/error.rs`, with words in `web/src/lib/errors.ts` and a row in `docs/system/api.md`.
5. Add tests in `services/<service>/tests/<scope>.rs` with `#[sqlx::test]`.
6. Write the `docs/system/` page, or add to an existing one.
7. Run `npm run api:types` in `web/` and keep the regenerated file.

A route in an existing scope: steps 3 to 7.

**A new crate**
1. Create `crates/<name>` with a `description`.
2. Add it to `members` and `[workspace.dependencies]` in `Cargo.toml`.
3. Add a row in `CLAUDE.md` (Map) and in `docs/system/architecture.md` (Rust workspace).
4. Delete the code it replaces, in the same change.

**A new service**
1. Create `services/<name>` with the layout above, as package `app-<name>`.
2. Take its ports from the registry. Add `config/<name>.toml` and its keys in `.env.example`.
3. Write `proto/<name>.proto`. For the first gRPC service, create `crates/proto` and `crates/clients`. Add its client in `app-clients`.
4. Give it its tables, with the owner named. Give each caller a token.
5. Add it to `scripts/dev.sh` and to `deploy/` as `app-<name>.service`, in the deploy order: callees before callers. If it needs more than `cargo test --workspace`, add the steps to `scripts/check.sh`.
6. Write `docs/system/<name>.md`, list it in `docs/system/README.md`, and add it to the process diagram, ports table and call graph in `docs/system/architecture.md`.
7. Add it to `CLAUDE.md` (Map, Commands).
