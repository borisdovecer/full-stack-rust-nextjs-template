#!/usr/bin/env bash
# Renames the template to your project. Run once, right after cloning.
#
#   scripts/rename.sh my-project [--allow-dirty]
#
# The name: lowercase letters, digits and dashes, starting with a letter. It becomes
#   my-project-api, my-project-web, my-project-core ...   binaries, services, crates
#   my_project                                             database, system user, paths, Rust paths
#   My Project                                             the display name (config/app.toml)
#
# Only the template's own placeholders are replaced, by pattern; words like `src/app/` or
# `AppState` are left alone.

set -euo pipefail
cd "$(dirname "$0")/.."

name=${1:-}
allow_dirty=${2:-}
die() { printf '\033[31merror:\033[0m %s\n' "$*" >&2; exit 1; }

[[ -n "$name" ]] || die "usage: scripts/rename.sh <name> [--allow-dirty]"
[[ "$name" =~ ^[a-z][a-z0-9]*(-[a-z0-9]+)*$ ]] || die "'$name' must be lowercase letters, digits and single dashes, starting with a letter"
(( ${#name} <= 40 )) || die "'$name' is longer than 40 characters"
[[ "$name" != "app" ]] || die "pick a name other than the placeholder"
grep -q '^name = "app-core"' crates/core/Cargo.toml || die "the placeholder is gone: this copy was renamed already"
command -v perl >/dev/null || die "perl is needed"

if git rev-parse --git-dir >/dev/null 2>&1 && [[ -n "$(git status --porcelain)" && "$allow_dirty" != "--allow-dirty" ]]; then
  die "the working tree has changes; commit them first, or pass --allow-dirty"
fi

dash=$name
snake=${name//-/_}
title=$(printf '%s' "$name" | perl -pe 's/(^|-)([a-z])/$1\u$2/g; s/-/ /g')
export DASH=$dash SNAKE=$snake TITLE=$title

# Every text file in the project, minus build output, a license, the lockfile and this script.
list=$(mktemp)
trap 'rm -f "$list"' EXIT
list_files() {
  { git ls-files --cached --others --exclude-standard 2>/dev/null ||
    find . -type f -not -path './.git/*' | sed 's|^\./||'; } |
    grep -vE '^(target|web/node_modules|web/\.next)/' |
    grep -vE '^(LICENSE-APACHE|scripts/rename\.sh|web/package-lock\.json)$' |
    while IFS= read -r f; do
      [[ -f "$f" && ! -L "$f" ]] && file --mime "$f" | grep -qE 'charset=(utf-8|us-ascii)' && printf '%s\0' "$f"
    done > "$list"
}
list_files

# 1. Prefixed names, paths and database URLs, everywhere.
xargs -0 perl -pi -e '
  s/(?<![\w-])app-(api|web|core|db|auth|backup|activate|deploy)(?!\w)/$ENV{DASH}-$1/g;
  s/(?<![\w])app_(api|core|db|auth|session)\b/$ENV{SNAKE}_$1/g;
  s{(/opt|/etc|/var/backups|\.config)/app\b}{$1/$ENV{SNAKE}}g;
  s{5432/app\b}{5432/$ENV{SNAKE}}g;
  s/POSTGRES_DB: app\b/POSTGRES_DB: $ENV{SNAKE}/g;
  s/sudoers-app-deploy/sudoers-$ENV{DASH}-deploy/g;
  s/full-stack-rust-nextjs-template/$ENV{DASH}/g;
' < "$list"

# 2. The display name, in the files that carry it.
title_files=(config/app.toml crates/core/src/config.rs services/api/src/app.rs
  web/src/app/layout.tsx web/src/components/AppName/AppName.tsx
  deploy/app-api.service deploy/app-web.service)
title_files+=(services/api/tests/*.rs)
perl -pi -e 's/\bApp\b/$ENV{TITLE}/g' "${title_files[@]}"

# 3. In the deploy files the bare word is the system user and the database.
deploy_files=(deploy/* docs/system/deploy.md .github/workflows/deploy.yml)
perl -pi -e 's/(?<![\w.-])app\b(?![.\w-]*\.toml)/$ENV{SNAKE}/g' "${deploy_files[@]}"

# 4. Files whose names carry the placeholder.
move() {
  if git ls-files --error-unmatch "$1" >/dev/null 2>&1; then git mv "$1" "$2"; else mv "$1" "$2"; fi
}
for f in deploy/app-api.service deploy/app-web.service deploy/app-backup.service \
         deploy/app-backup.timer deploy/app-backup deploy/app-activate deploy/sudoers-app-deploy; do
  [[ -e "$f" ]] || continue
  base=$(basename "$f")
  new=${base/app-/$dash-}
  move "$f" "deploy/$new"
done
move skills/app-api "skills/$dash-api"
rm -f .claude/skills/app-api
ln -s "../../skills/$dash-api" ".claude/skills/$dash-api"

# 5. Check the result builds.
echo "Checking the Rust workspace..."
cargo check -q --workspace
if [[ -d web/node_modules ]]; then
  echo "Checking the web..."
  (cd web && npm run -s typecheck >/dev/null)
fi

list_files
left=$(xargs -0 perl -ne 'print "$ARGV:$.: $_" if /(?<![\w-])app[-_](api|web|core|db|auth|session|backup|activate)\b/; close ARGV if eof' < "$list" 2>/dev/null || true)
if [[ -n "$left" ]]; then
  echo "These still use the placeholder; review them:"
  echo "$left"
fi

cat <<DONE
Renamed to $dash.
  crates and binaries  $dash-api, $dash-core, ...
  database and user    $snake
  display name         $title (config/app.toml)
Next: edit DATABASE_URL in .env to end in /$snake (or run scripts/setup.sh), then commit.
DONE
