#!/usr/bin/env bash
# First run on a new machine. Safe to run again: every step checks before it acts.
#
#   1. checks the tools
#   2. creates .env and web/.env.local from their examples
#   3. creates the database on the local Postgres
#   4. builds the API and runs the migrations
#   5. creates the first admin, if there is none, and saves its sign-in to .env for skills/app-api
#   6. installs the web dependencies

set -euo pipefail

cd "$(dirname "$0")/.."
ROOT=$(pwd)

say() { printf '\033[1m==> %s\033[0m\n' "$*"; }
die() { printf '\033[31merror:\033[0m %s\n' "$*" >&2; exit 1; }

# 1. Tools
say "Checking tools"
need() { command -v "$1" >/dev/null 2>&1 || die "$1 is not installed. $2"; }
need cargo "Install Rust from https://rustup.rs"
need psql "Install the PostgreSQL client (psql)"
if [[ -d web ]]; then
  need node "Install Node 26 (nvm: https://github.com/nvm-sh/nvm, then 'nvm install' in web/)"
  need npm "npm comes with Node"
  node_major=$(node -p 'process.versions.node.split(".")[0]')
  if (( node_major < 26 )); then
    die "Node $node_major is too old; this project needs Node 26. Run 'nvm use' in web/."
  fi
fi

# 2. Env files
if [[ ! -f .env ]]; then
  cp .env.example .env
  chmod 600 .env
  say "Created .env from .env.example"
fi
if [[ -f web/.env.example && ! -f web/.env.local ]]; then
  cp web/.env.example web/.env.local
  say "Created web/.env.local from web/.env.example"
fi

set -a
# shellcheck disable=SC1091
. ./.env
set +a

[[ -n "${DATABASE_URL:-}" ]] || die "DATABASE_URL is empty in .env"
if [[ "$DATABASE_URL" == *"USER:PASSWORD"* ]]; then
  die "Edit DATABASE_URL in .env (it still has the USER:PASSWORD placeholder), then run this again."
fi

# 3. Database. Created from template0, which every Postgres has and which avoids a stale
#    collation version on template1 after an OS upgrade.
db_name=${DATABASE_URL##*/}
db_name=${db_name%%\?*}
server_url=${DATABASE_URL%/*}/postgres
[[ "$db_name" =~ ^[a-z_][a-z0-9_]*$ ]] || die "The database name '$db_name' must be lowercase letters, digits and _"

if psql "$server_url" -Atqc "SELECT 1 FROM pg_database WHERE datname = '$db_name'" 2>/dev/null | grep -q 1; then
  say "Database $db_name exists"
else
  say "Creating database $db_name"
  psql "$server_url" -qc "CREATE DATABASE \"$db_name\" TEMPLATE template0" >/dev/null \
    || die "Cannot create the database. Check DATABASE_URL in .env and that Postgres is running."
fi

# 4. Build and migrate
say "Building the API"
cargo build -q -p app-api
API="$ROOT/target/debug/app-api"

say "Running migrations"
"$API" migrate

# 5. First admin
has_admin=$(psql "$DATABASE_URL" -Atqc "SELECT EXISTS (SELECT 1 FROM users WHERE role = 'admin')")
if [[ "$has_admin" == "t" ]]; then
  say "An admin exists"
  if [[ -z "${ADMIN_EMAIL:-}" || -z "${ADMIN_PASSWORD:-}" ]]; then
    say "To use skills/app-api, fill ADMIN_EMAIL and ADMIN_PASSWORD in .env by hand"
  fi
else
  say "Creating the first admin"
  read -r -p "Admin email: " admin_email
  read -rs -p "Admin password: " admin_password
  echo
  read -rs -p "Repeat it: " admin_password_again
  echo
  [[ -n "$admin_password" ]] || die "The password must not be empty"
  [[ "$admin_password" == "$admin_password_again" ]] || die "The passwords do not match"
  # .env values are single-quoted so bash and dotenvy both read them literally.
  [[ "$admin_password" != *"'"* && "$admin_email" != *"'"* ]] \
    || die "The email and password must not contain '"
  printf '%s\n' "$admin_password" | "$API" create-admin --password-stdin \
    --email "$admin_email" --first-name admin --last-name dev

  set_env() {
    local key=$1 value=$2 tmp
    tmp=$(mktemp "$ROOT/.env.XXXXXX")
    grep -v "^$key=" .env >"$tmp" || true
    printf "%s='%s'\n" "$key" "$value" >>"$tmp"
    chmod 600 "$tmp"
    mv "$tmp" .env
  }
  set_env ADMIN_EMAIL "$admin_email"
  set_env ADMIN_PASSWORD "$admin_password"
  unset admin_password admin_password_again
  say "Saved the admin's sign-in to .env for skills/app-api"
fi

# 6. Web
if [[ -f web/package.json ]]; then
  say "Installing web dependencies"
  (cd web && npm ci --no-audit --no-fund)
fi

say "Done. Start everything with scripts/dev.sh"
