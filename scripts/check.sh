#!/usr/bin/env bash
# Every check CI runs. Run it before calling work done.
#   scripts/check.sh          everything
#   scripts/check.sh rust     only the Rust checks
#   scripts/check.sh web      only the web checks
#   scripts/check.sh security dependency advisories and a secret scan, as CI's security.yml
#                             runs them (needs cargo-deny and gitleaks, and the network)
# The Rust tests need Postgres: DATABASE_URL from .env, or TEST_DATABASE_URL when set (sqlx
# creates and drops a database per test, so the user needs CREATEDB).

set -euo pipefail
cd "$(dirname "$0")/.."
what=${1:-all}

step() { printf '\033[1m==> %s\033[0m\n' "$*"; }

if [[ "$what" == all || "$what" == rust ]]; then
  test_db=${TEST_DATABASE_URL:-}
  if [[ -f .env ]]; then set -a; . ./.env; set +a; fi
  if [[ -n "$test_db" ]]; then export DATABASE_URL=$test_db; fi
  step "cargo fmt"
  cargo fmt --all --check
  step "cargo clippy"
  cargo clippy -q --workspace --all-targets -- -D warnings
  step "cargo test"
  cargo test -q --workspace
fi

if [[ "$what" == all || "$what" == web ]]; then
  cd web
  step "OpenAPI types are current"
  before=$(mktemp)
  cp src/lib/api/schema.d.ts "$before"
  npm run -s api:types >/dev/null
  if ! cmp -s "$before" src/lib/api/schema.d.ts; then
    rm -f "$before"
    echo "src/lib/api/schema.d.ts was stale; it is regenerated now: review and keep it." >&2
    exit 1
  fi
  rm -f "$before"
  step "format"
  npm run -s format:check
  step "lint"
  npm run -s lint
  step "typecheck"
  npm run -s typecheck
  step "test"
  npm test --silent
  step "build"
  npm run -s build
fi

if [[ "$what" == security ]]; then
  for tool in cargo-deny gitleaks; do
    command -v "$tool" >/dev/null || { echo "$tool is not installed; see docs/rules/git.md" >&2; exit 1; }
  done
  step "Rust advisories"
  cargo deny check advisories
  step "web advisories"
  (cd web && npm audit --omit=dev --audit-level=high)
  step "secrets"
  if git rev-parse -q --verify HEAD >/dev/null; then
    gitleaks git --redact --no-banner .
  else
    echo "no commits yet: nothing to scan"
  fi
fi

step "All checks passed"
