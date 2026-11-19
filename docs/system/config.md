# Configuration

Last checked against the code on 2026-11-18.

Secrets and per-machine values come from environment variables (`.env` in development). Everything else is in `config/app.toml`. Both are validated when the API starts: a missing or bad value stops it with a message that names the variable or key, and an unknown key in the TOML file is refused.

## API environment (`.env`)

| Variable | Required | Default | Notes |
|---|---|---|---|
| `APP_ENV` | no | `development` | `production` turns on `Secure` cookies, JSON logs, embedded help docs, and allows a non-loopback `API_BIND` |
| `DATABASE_URL` | yes | | `postgres://user:password@host:5432/app`. Secret |
| `DATABASE_MAX_CONNECTIONS` | no | 10 | the API's pool size. All instances together must stay below Postgres' `max_connections` (100 by default), or put PgBouncer in front |
| `DATABASE_ACQUIRE_TIMEOUT_SECONDS` | no | 5 | how long a request waits for a free connection before `503 unavailable` |
| `DATABASE_STATEMENT_TIMEOUT_SECONDS` | no | 30 | Postgres ends a longer statement; 0 = no limit. `app-api migrate` never has one |
| `API_BIND` | no | `127.0.0.1:8080` | a non-loopback address is refused unless `APP_ENV=production` |
| `APP_CONFIG` | no | `config/app.toml` | path to the settings file |
| `RUST_LOG` | no | `info,sqlx=warn,tower_http=info` | log filter |
| `SENTRY_DSN` | no | | empty = Sentry off |
| `GOOGLE_CLIENT_SECRET` | no | | with `oauth.google.client_id`, turns on "Continue with Google". Secret |
| `GITHUB_CLIENT_SECRET` | no | | with `oauth.github.client_id`, turns on "Continue with GitHub". Secret |
| `RESEND_API_KEY` | no | | Resend's key for the sign-in codes. Empty = codes go to the log, and the email-code method is off in production. Secret. See [mail](mail.md) |
| `ADMIN_EMAIL`, `ADMIN_PASSWORD` | no | | not read by the API. The first admin's sign-in, written by `scripts/setup.sh` when it creates that admin, used by `skills/app-api` in development. Never set on a server. Secret |
| `TELEGRAM_BOT_TOKEN`, `TELEGRAM_CHAT_ID` | no | | not read by the API. The team's report chat for `skills/app-dev`: the bot's token (secret) and the chat's id. Never set on a server |
| `GIT_COMMIT` | build time | `git rev-parse` | the commit `/api/health` reports, for builds outside a checkout |

## Settings (`config/app.toml`)

| Key | Default | Meaning |
|---|---|---|
| `app.name` | `App` | shown in the web header and the welcome page |
| `session.cookie_name` | `app_session` | |
| `session.lifetime_hours` | 168 | how long a session lasts without use |
| `session.extend_after_hours` | 24 | a used session gets a fresh lifetime once this much has passed; must be below the lifetime |
| `session.reauth_minutes` | 10 | changing the sign-in method needs a sign-in this recent |
| `login_throttle.max_failures` | 5 | failed logins per email in the window |
| `login_throttle.max_failures_per_ip` | 20 | failed logins per IP in the window |
| `login_throttle.window_minutes` | 15 | |
| `registration.mode` | `open` | `open`, `approval` or `closed`; see [auth](auth.md) |
| `mail.from` | `App <no-reply@example.com>` | the sender; its domain must be verified at Resend |
| `login_code.lifetime_minutes` | 10 | how long a mailed code works |
| `login_code.max_attempts` | 3 | wrong codes before the address is paused |
| `login_code.pause_minutes` | 15 | |
| `login_code.resend_seconds` | 60 | at most one mail per address this often |
| `login_code.max_requests_per_ip` | 10 | codes one IP may ask for per `lifetime_minutes` |
| `wallet.domain` | `127.0.0.1:3000` | the host people open the app on, exactly as the address bar shows it (`app.example.com`); a signed wallet message must name it, and wallets refuse to sign when it is not the page's host. `localhost:3000` and `127.0.0.1:3000` are different hosts. Set it on every server: production refuses a loopback or `localhost` host |
| `wallet.statement` | `Sign in to App. …` | the sentence the wallet shows; one line |
| `wallet.nonce_minutes` | 5 | how long a wallet challenge may wait to be signed |
| `wallet.max_nonces_per_ip` | 30 | challenges one IP may ask for per `nonce_minutes` |
| `oauth.public_url` | `http://127.0.0.1:3000` | where people open the app, no trailing slash; providers send them back to `{public_url}/api/auth/oauth/{provider}/callback`. With a provider on, production needs `https://` and a host other than this machine |
| `oauth.google.client_id` | empty | Google's OAuth client id; empty = Google off |
| `oauth.github.client_id` | empty | GitHub's OAuth app client id; empty = GitHub off |
| `oauth.<provider>.endpoints` | the provider's | other `authorize_url`, `token_url`, `user_url` (and `emails_url` for GitHub), for GitHub Enterprise or tests |
| `password.max_concurrent_hashes` | 0 | Argon2 hashes that may run at once (about 19 MiB each); more wait. 0 = twice the CPU cores |
| `help.source` | `auto` | `auto` (disk in development, embedded in production), `disk` or `embedded` |

## Web environment (`web/.env.local`)

| Variable | Default | Notes |
|---|---|---|
| `API_URL` | `http://127.0.0.1:8080` | where `/api/*` is forwarded without Caddy. Read at `next dev` start and baked in at `next build` |
| `WEB_PORT` | 3000 | for `npm run dev` and `npm start` |
| `NEXT_PUBLIC_SENTRY_DSN` | | empty = Sentry off in the browser |

Only `NEXT_PUBLIC_*` values reach the browser. The web holds no secrets.

## Ports

API 8080, web 3000, Postgres 5432, all set in the env files. When several projects run on one machine, give each its own range and write it down in its README.
