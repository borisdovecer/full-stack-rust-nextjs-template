#!/usr/bin/env bash
# Runs the API and the web together. Ctrl-C stops both.
# The API restarts on changes when cargo-watch is installed (cargo install cargo-watch).

set -euo pipefail
cd "$(dirname "$0")/.."

[[ -f .env ]] || { echo "No .env: run scripts/setup.sh first." >&2; exit 1; }

pids=()
cleanup() {
  for pid in "${pids[@]}"; do kill "$pid" 2>/dev/null || true; done
  wait 2>/dev/null || true
}
trap cleanup EXIT INT TERM

if command -v cargo-watch >/dev/null 2>&1; then
  cargo watch -q -w crates -w services -w migrations -w config -x "run -q -p app-api" &
else
  cargo run -q -p app-api &
fi
pids+=($!)

(cd web && npm run dev) &
pids+=($!)

wait -n
