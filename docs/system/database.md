# Database

Last checked against the code on 2026-11-18.

PostgreSQL 16 or newer, with the `pg_trgm` extension (ships with Postgres; created by migration `0001_init`). One database per environment, named after the project (`app` until `scripts/rename.sh` runs). Only the API connects to it. Code: `migrations/`, `crates/db/`, and the API's repositories (`services/api/src/auth/repository/`, `auth/oauth/repository.rs`, `shared/repository/`).

## Conventions

- Primary keys are UUIDs (`gen_random_uuid()`), except append-only logs, which use `bigserial`.
- Every time is `timestamptz`. Mutable tables have `created_at` and `updated_at`; a trigger (`set_updated_at`) keeps `updated_at` current.
- Fixed value sets are Postgres enums: `user_role`, `user_status`, `theme`, `login_type`, `login_code_purpose`.
- Names: tables plural snake case, constraints and indexes `<table>_<what>_<kind>` (`users_email_key`, `user_sessions_user_id_idx`).

## Tables

### users

| Column | Type | Notes |
|---|---|---|
| id | uuid PK | |
| email | varchar(255) | unique (`users_email_key`); stored lowercased and trimmed, enforced by a check |
| first_name | varchar(255) | not blank (`users_first_name_not_blank`) |
| last_name | varchar(255) | not blank (`users_last_name_not_blank`) |
| bio | text null | at most 1000 characters (`users_bio_length`); blank is stored as null |
| login_type | login_type | how the account signs in: `password` / `wallet` / `code` / `external`, default `password` |
| wallet | varchar(64) null | the Solana address (base58) of a `wallet` account; unique where set (`users_wallet_key`) |
| password_hash | varchar(255) null | Argon2id PHC string; null for an account without a password |
| role | user_role | `user` / `admin`, default `user` |
| status | user_status | `pending` / `active` / `suspended`, default `active` |
| last_login_at | timestamptz null | |
| created_at, updated_at | timestamptz | |

Check `users_login_type_credentials`: a `password` account has a `password_hash`, a `wallet` account has a `wallet`. The columns of methods an account does not use may stay null.

Indexes besides the keys: `created_at DESC` (default list order), `(role, status)`, trigram indexes on `email` and on `first_name || ' ' || last_name` (the user search, "anywhere in the email or name"), `(lower(first_name), lower(last_name), id)` and `(last_login_at DESC NULLS LAST, id)` for the list's sorts. The list counts matches only up to 10,000 (`users::COUNT_CAP`) and never reads a page past that, so a page costs at most 10,000 rows however many users there are. Beyond it the API answers `total_capped: true` and the web shows "10,000+"; search and filters reach the rest.

### user_preferences

One row per user, created in the same statement as the user.

| Column | Type | Notes |
|---|---|---|
| user_id | uuid PK, FK users | cascade on delete |
| theme | theme | `system` / `light` / `dark`, default `system` |
| updated_at | timestamptz | |

A new preference is a new column with a default.

### user_permissions

Grants on top of the role. The database does not know the permission list, so adding one needs no migration.

| Column | Type | Notes |
|---|---|---|
| user_id | uuid FK users | PK with `permission` |
| permission | varchar(64) | a name from `crates/auth/src/permission.rs` |
| granted_by | uuid FK users null | who granted it; shown in the user detail |
| granted_at | timestamptz | shown in the user detail |

### user_sessions

| Column | Type | Notes |
|---|---|---|
| id | uuid PK | |
| user_id | uuid FK users | cascade on delete |
| token_hash | bytea | SHA-256 of the cookie token, unique |
| created_at, last_seen_at, expires_at | timestamptz | |
| revoked_at | timestamptz null | |
| ip, user_agent | text null | shown in Settings |

Valid while `revoked_at IS NULL AND expires_at > now()`. Ended sessions are deleted a day later.

### auth_login_attempts

| Column | Type | Notes |
|---|---|---|
| id | bigserial PK | |
| email | varchar(255) | normalised, whether or not an account has it |
| method | login_type | the method tried, default `password`; failures are counted per email and method |
| ip | text null | |
| success | boolean | |
| attempted_at | timestamptz | indexed with email and method, and with ip |

Read for throttling; rows older than 30 days are deleted. Every attempt is written as a failure before its proof is checked and turned into a success after, so attempts in flight already count (see [auth](auth.md)).

### login_codes

The codes the `code` sign-in method mails. One row per code asked for, whether or not an account uses the address.

| Column | Type | Notes |
|---|---|---|
| id | uuid PK | chosen by the API; also the hash's salt |
| email | varchar(255) | normalised |
| purpose | login_code_purpose | `login`, `register`, `change`; a code works only for its purpose |
| code_hash | bytea | SHA-256 of the id and the code |
| attempts | integer | wrong tries so far |
| ip | text null | who asked; limits requests per IP |
| expires_at | timestamptz | |
| used_at | timestamptz null | spent, or retired by a newer code |
| created_at | timestamptz | indexed with email and purpose, and with ip |

Deleted a day after they expire.

### wallet_nonces

Challenges a Solana wallet signs to sign in.

| Column | Type | Notes |
|---|---|---|
| nonce | varchar(64) PK | random, base58 |
| address | varchar(64) | the address it was issued for; only that address can spend it |
| ip | text null | who asked; limits challenges per IP |
| issued_at, expires_at | timestamptz | |
| used_at | timestamptz null | spent before the signature is checked |

Deleted a day after they expire.

### user_identities

The Google or GitHub account an `external` account signs in with.

| Column | Type | Notes |
|---|---|---|
| user_id | uuid FK users | cascade on delete; unique: one per account |
| provider | varchar(32) | `google`, `github`; PK with `subject` |
| subject | varchar(255) | the provider's own user id |
| email | varchar(255) | the verified email when tied; a record for support, not read by the API |
| created_at | timestamptz | |

Deleted when the account changes its sign-in method.

### oauth_signups

A provider's verified answer waiting for the registration form.

| Column | Type | Notes |
|---|---|---|
| token_hash | bytea PK | SHA-256 of the token in the browser's sign-up cookie |
| provider, subject, email | | as above |
| first_name, last_name | varchar(255) null | what the provider gave |
| expires_at, created_at | timestamptz | 30 minutes; deleted on use and a day after expiry |

### audit_events

| Column | Type | Notes |
|---|---|---|
| id | bigserial PK | |
| actor_id | uuid FK users null | null = the system or the CLI |
| subject_id | uuid FK users null | the user acted on |
| action | varchar(64) | `user.created`, `user.registered`, `user.updated`, `user.password_set`, `user.password_changed`, `user.login_method_changed`, `user.permissions_changed` |
| data | jsonb | before and after values; never a password, hash or token |
| created_at | timestamptz | |

Written in the same transaction as the change it records. `audit_events_created_at_idx` is kept for a global audit-log page ("everything, newest first"); the user detail reads by `subject_id`.

## Migrations

Rules: `docs/rules/database.md`. Files: `migrations/NNNN_name.up.sql` and `.down.sql`.

`0001_init` creates the whole schema above: the `pg_trgm` extension, the enums, `set_updated_at()`, every table and every index. Its tables are empty when it runs, so it builds indexes without `CONCURRENTLY`; an index added later to a table with rows goes in a migration of its own (rules above). The next migration is `0002`.

The migrations are built into the API binary: `app-api migrate` applies them, `app-api migrate --revert-last` undoes the newest. The API refuses to start, and `/api/health` answers `503`, while a migration is pending, failed or was edited after it ran.

## Clean-up

Once an hour the API deletes ended sessions (a day after they end), login attempts older than 30 days, and codes, wallet challenges and provider sign-ups a day after they expire (`services/api/src/jobs/housekeeping.rs`). It deletes 5,000 rows per statement until none are left, so a backlog never holds one long lock, and it takes an advisory lock first, so with several API instances only one runs it.

## Pool

Each API process has a pool of `DATABASE_MAX_CONNECTIONS` (10) connections. A request waits up to `DATABASE_ACQUIRE_TIMEOUT_SECONDS` (5) for one, then answers `503 unavailable`. Postgres ends a statement after `DATABASE_STATEMENT_TIMEOUT_SECONDS` (30); `app-api migrate` connects without that limit. See [config](config.md).

## Code

The API owns every table. Its SQL is in `services/api/src/auth/repository/` (one file per table: `users`, `sessions`, `permissions`, `preferences`, `identities`, `login_codes`, `login_attempts`, `wallet_nonces`, and the row types in `models.rs`), `auth/oauth/repository.rs` (`oauth_signups`) and `shared/repository/audit.rs` (`audit_events`). `crates/db` holds the pool, the migrations and shared helpers. Functions that run one statement take any executor (`&PgPool` or a transaction); functions that run several take `&mut PgConnection`, so the caller owns the transaction. Rows are never serialised to clients: the API maps them to DTOs.
