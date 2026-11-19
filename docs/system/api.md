# API

Last checked against the code on 2026-11-18.

The HTTP API in `services/api`. JSON in and out, everything under `/api`. The full, typed description is the OpenAPI document: `GET /api/openapi.json` (admins only in production) or `cargo run -p app-api -- openapi`.

## Errors

Every error answers with the same body:

```json
{ "error": { "code": "validation_failed", "message": "some fields are invalid", "request_id": "3f2a...", "details": { "fields": { "email": ["invalid_email"] } } } }
```

Decide on `code`, never on `message`: the message is for developers and can change. `details` is present when there are facts to act on:

| Code | Status | details |
|---|---|---|
| `invalid_request` | 400 | |
| `validation_failed` | 422 | `fields`: field → problems (`required`, `too_long`, `too_short`, `invalid`, `invalid_email`, `invalid_address`, `unknown_permission`) |
| `unsupported_media_type` | 415 | |
| `payload_too_large` | 413 | |
| `unauthenticated` | 401 | |
| `invalid_credentials` | 401 | |
| `invalid_code` | 401 | the mailed code is wrong, expired, spent or was never sent |
| `invalid_signature` | 401 | a wallet signature failed a check |
| `email_not_verified` | 401 | Google or GitHub has not verified the email |
| `provider_failed` | 401 | the round trip through Google or GitHub failed or was refused |
| `wrong_password` | 422 | |
| `login_method_unavailable` | 422 | |
| `forbidden` | 403 | |
| `account_pending` | 403 | |
| `account_suspended` | 403 | |
| `registration_closed` | 403 | |
| `reauth_required` | 403 | sign in again: the change needs a recent sign-in |
| `not_found` | 404 | |
| `method_not_allowed` | 405 | |
| `email_taken` | 409 | |
| `wallet_taken` | 409 | |
| `identity_taken` | 409 | the Google or GitHub account belongs to another account |
| `provider_email_mismatch` | 409 | the Google or GitHub account has another email |
| `last_admin` | 409 | |
| `cannot_change_self` | 409 | |
| `login_method_unchanged` | 409 | |
| `too_many_attempts` | 429 | `retry_at` (RFC 3339) |
| `request_timeout` | 408 | |
| `internal` | 500 | |
| `unavailable` | 503 | the database is down or busy; also for a signed-in route while the session cannot be looked up |

The list lives in `crates/core/src/error.rs`. The web has words for every code in `web/src/lib/errors.ts`; its type fails to compile when a code has none. A test (`the_api_docs_list_every_code`) fails when this table misses a code.

## Routes

| Method and path | Needs | What |
|---|---|---|
| `GET /api/health` | | version, commit, environment, database, migrations; `503` when degraded |
| `GET /api/config` | | app name, registration mode, `login_methods`, `admin_login_methods`, `oauth_providers`, version |
| `GET /api/openapi.json` | `users.manage` in production | the OpenAPI document; `404` for anyone else |
| `POST /api/auth/register` | | `method` (default `password`), first_name, last_name, optional bio, email, the method's proof → `201` + `Me`, sets the cookie |
| `POST /api/auth/login` | | `method` (default `password`), email, the method's proof → `Me`, sets the cookie |
| `POST /api/auth/wallet/nonce` | | address → a challenge for the wallet to sign (domain, statement, nonce, times) |
| `GET /api/auth/oauth/{provider}/start` | | `intent` `login`, `register` or `link` → `303` to Google or GitHub; `404` when the provider is off |
| `GET /api/auth/oauth/{provider}/callback` | | the provider's answer → `303` to `/`, `/register?via=external`, `/settings`, or the page with `?error=` |
| `GET /api/auth/oauth/signup` | sign-up cookie | the provider's waiting answer for the registration form |
| `POST /api/auth/code` | | email, `purpose` (`login` or `register`) → always `204`; mails a code when there is someone to send it to |
| `POST /api/auth/logout` | signed in | ends this session, clears the cookie |
| `POST /api/auth/logout-all` | signed in | ends every session of the user |
| `GET /api/me` | signed in (pending too) | `Me`: user, permissions, preferences |
| `PATCH /api/me` | active | first_name, last_name, bio (each optional; an empty bio clears it) |
| `POST /api/me/password` | active | current and new password; ends the other sessions. A wrong current password is throttled like a login (`429`) |
| `POST /api/me/login-code` | active, recent sign-in | mails a code to the user's own address, to switch to codes |
| `PUT /api/me/login-method` | active | `method` and its proof; clears the old method, ends the other sessions |
| `GET /api/me/sessions` | active | live sessions, the current one marked |
| `DELETE /api/me/sessions/{id}` | active | ends one; ending the current one signs out |
| `PATCH /api/me/preferences` | signed in (pending too) | theme |
| `GET /api/users` | `users.read` | `search`, `role`, `status`, `sort`, `page`, `per_page` (max 100). `total` counts up to 10,000; with `total_capped` more match, and pages past 10,000 users are empty |
| `POST /api/users` | `users.manage` | create a user, with `method` `password` (default) or `code` |
| `GET /api/users/{id}` | `users.read` | user, permissions, grants (each with who granted it and when), last 20 audit events |
| `PATCH /api/users/{id}` | `users.manage` | first_name, last_name, bio, role, status |
| `PUT /api/users/{id}/password` | `users.manage` | set a password and make it their method; ends their sessions. Not your own (`409 cannot_change_self`): that is `POST /api/me/password` |
| `PUT /api/users/{id}/login-method` | `users.manage` | recovery: `password` (with a new one) or `code`; ends their sessions |
| `PUT /api/users/{id}/permissions` | `users.manage` | replace their grants |
| `GET /api/help` | signed in | the help index |
| `GET /api/help/{slug}` | signed in | one page as `text/markdown` |

Sort values for `/api/users`: `created_desc` (default), `created_asc`, `name_asc`, `name_desc` (first name, then last name), `email_asc`, `last_login_desc`.

## Adding a route

1. Write the handler in `services/api/src/<scope>/handlers.rs` with a `#[utoipa::path]` that lists its responses and error codes, and its request and response types in `<scope>/dto.rs`. Where things go: `docs/rules/architecture.md`.
2. Register it in the scope's `routes()` in `<scope>/mod.rs`. `api_routes()` in `services/api/src/app.rs` merges every scope, and the router and the OpenAPI document come from that one list, so they cannot drift.
3. Declare access with an extractor (`CurrentUser`, `Require<P>`), never by checking the role inline. `P` is a marker type from `permissions!` in `crates/auth/src/permission.rs`, re-exported by `services/api/src/auth/mod.rs`.
4. Return DTOs, not database rows.
5. Test it in `services/api/tests/` with `#[sqlx::test]` (the `TestApp` helper in `tests/common`).
6. Run `npm run api:types` in `web/` and commit the regenerated `schema.d.ts`; CI fails when it is stale.

## Help docs

`GET /api/help` reads the index in `docs/system/README.md`: each `- [Title](file.md)` line is a page, in order. Only listed files are served. Release builds carry the folder inside the binary; development reads it from disk (`[help] source` in `config/app.toml`).

## Admin CLI

The same binary:

| Command | Does |
|---|---|
| `app-api` / `app-api serve` | runs the server |
| `app-api openapi` | prints the OpenAPI document |
| `app-api migrate [--revert-last]` | applies pending migrations, or undoes the newest |
| `app-api create-admin --email E --first-name F --last-name L [--password-stdin]` | creates an active admin |
| `app-api reset-password --email E [--password-stdin]` | sets a password and ends every session |

Passwords are read from the terminal without echo, or from the first line of stdin with `--password-stdin`. Never from an argument, which would end up in shell history and in `ps`.
