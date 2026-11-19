# Git rules

## Branches

- `main` always builds and passes `scripts/check.sh`.
- Work happens on a branch: `fix/<short-title>`, `feat/<short-title>`, `docs/<short-title>`, `chore/<short-title>`, named after the document it implements.
- Merge to `main` through a pull request once CI is green. Prefer a squash merge for a branch of small fixups, a normal merge for a branch of meaningful commits.

## Commits

One finished piece of work per commit: a fix, a feature or one task of it, a refactor on its own, a doc change. Do not mix a refactor into a fix.

Do not commit:

- code that does not build, or failing tests
- secrets: `.env` files, keys, tokens, real `DATABASE_URL`s, logs that contain them
- build output: `target/`, `node_modules/`, `.next/`

Message format:

```
type(scope): short summary in the imperative

What changed and why, in a few lines. The reasoning lives in the document;
link it rather than repeating it.

Refs: docs/bug_report/18-11-2026-high-api-login-throttle-counts-successes.md
```

Types: `feat`, `fix`, `refactor`, `test`, `docs`, `chore`, `ci`. Scopes: `api`, `db`, `auth`, `web`, `deploy`, `docs`.

An agent commits, branches or pushes only when asked.

## Conflicts

- Rebase your branch on `main` before opening a pull request; resolve conflicts on your branch, never on `main`.
- Generated files are not merged by hand: after resolving, regenerate them (`npm run api:types` for `web/src/lib/api/schema.d.ts`, `npm install` for `package-lock.json`).
- Two branches that both add migration `NNNN`: the later one renumbers its file before merging. A migration that already ran anywhere is never renumbered.
- When unsure which side is right, ask the author of the other change.

## Before pushing

Run `scripts/check.sh`. Before pushing to a public repository, also run `scripts/check.sh security`: known vulnerabilities in the Rust and web dependencies, and secrets in every commit. It needs `cargo install --locked cargo-deny` and the `gitleaks` binary from its releases page. CI runs the same checks in `.github/workflows/security.yml` on every push and weekly.
