# Claude Code briefing

A full-stack app: a Rust API (`services/api`, shared crates in `crates/`), a Next.js UI (`web/`, with its own `web/CLAUDE.md`), and PostgreSQL. It starts with accounts, sessions, roles and permissions, an admin user list, light and dark themes, a Help page and deploy files.

## Before any work

1. Read `docs/README.md`, then the `docs/system/` page for the part you are changing.
2. Read the rule file that applies (below) and follow it.
3. When a doc and the code disagree, the code wins and the doc is fixed in the same change.

## Rules

| File | Covers |
|---|---|
| `docs/rules/development.md` | the workflow from a bug report or feature request to finished work, what "done" means |
| `docs/rules/architecture.md` | services and crates, the layout inside a service, ports, gRPC and proto |
| `docs/rules/database.md` | migrations and schema changes |
| `docs/rules/git.md` | branches, commit messages, conflicts |
| `docs/rules/ui.md` | everything in `web/`: components, theme, layout, words, forms |

A new rule the team agrees on goes into the matching file (or a new `<topic>.md`) and a line here.

## Hard rules

- Never commit, branch or push unless asked.
- Never approve a proposal or answer a question for the owner. Write the question into the document with options and a recommendation.
- No code change without an approved bug report or feature request (`docs/rules/development.md`), except typos.
- Never write secrets (passwords, keys, tokens, real `DATABASE_URL`s) into code, docs, tests, logs or commits. They live only in `.env` files.
- Rust is the only backend. The web has no route handlers, no server actions, no database access and no secrets.
- A code comment says only what the code cannot say. No names, no dates, no history.

## Map

| Path | What |
|---|---|
| `crates/core` | config, error codes, extractors, request id |
| `crates/db` | pool, migrations, shared query helpers (tables and their SQL live in the service's scopes) |
| `crates/auth` | passwords, sign-in codes, wallet signatures (Sign-In-With-Solana), session tokens, permission list |
| `crates/mail` | sends email (Resend, or the log in development) |
| `services/api` | one folder per scope in `src/` (`auth`, `me`, `users`, `help`, `system`), `src/shared/`, `src/jobs/`, CLI, tests (`tests/`) |
| `migrations/` | SQL migrations, built into the API binary |
| `config/app.toml` | non-secret settings |
| `web/` | the UI |
| `deploy/` | Caddy, systemd units, backups, server guide |
| `scripts/` | `setup.sh`, `dev.sh`, `check.sh`, `rename.sh` |
| `skills/` | Claude Code skills; `.claude/skills/` links to them |

## Commands

```bash
scripts/setup.sh            # first run: env files, database, migrations, first admin, npm install
scripts/dev.sh              # API on :8080 and web on :3000
scripts/check.sh            # everything CI runs; must pass before work is done
scripts/check.sh security   # dependency advisories and a secret scan (CI: security.yml)
cargo run -p app-api -- migrate
cd web && npm run api:types # after changing an API route or DTO
```

Rust conventions: `thiserror` in crates, `anyhow` in binaries, `tracing` for logs, no `unwrap` outside tests. API errors are `AppError` with an `ErrorCode`; a new code also needs words in `web/src/lib/errors.ts`.

## Skills

`skills/app-api` signs in to a running API and calls it: list and manage users, read help pages, check health. Use it to try a change against the real app.

`skills/app-dev` is the workflow for a bug or a feature from the first note to the archive, and reads and posts to the team's Telegram report chat when asked.
