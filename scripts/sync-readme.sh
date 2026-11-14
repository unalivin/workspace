#!/usr/bin/env bash
set -euo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

usage() {
  cat <<'USAGE'
Usage: scripts/sync-readme.sh [--check] [--dry-run]

Rewrites the project table in README.md between the projects:start and
projects:end markers from the registry in scripts/projects.tsv.

  --check    Exit with status 1 if README.md would change, write nothing
  --dry-run  Print the table that would be written, write nothing
  --help     Show this help
USAGE
}

check=0
while [ "$#" -gt 0 ]; do
  case "$1" in
    --check) check=1 ;;
    --dry-run) DRY_RUN=1 ;;
    --help | -h)
      usage
      exit 0
      ;;
    *) die "unknown argument: $1 (see --help)" ;;
  esac
  shift
done

readme="$WORKSPACE_ROOT/README.md"
[ -f "$readme" ] || die "README.md not found"
grep -q '<!-- projects:start -->' "$readme" || die "README.md has no projects:start marker"
grep -q '<!-- projects:end -->' "$readme" || die "README.md has no projects:end marker"

table="$(mktemp)"
candidate="$(mktemp)"
trap 'rm -f "$table" "$candidate"' EXIT
"$WORKSPACE_ROOT/scripts/list-projects.sh" --markdown >"$table"

awk -v table="$table" '
  /<!-- projects:start -->/ {
    print
    while ((getline row < table) > 0) {
      print row
    }
    close(table)
    skipping = 1
    next
  }
  /<!-- projects:end -->/ { skipping = 0 }
  !skipping { print }
' "$readme" >"$candidate"

if cmp -s "$readme" "$candidate"; then
  note "README.md project table is up to date"
  exit 0
fi
if [ "$check" = "1" ]; then
  diff -u "$readme" "$candidate" >&2 || true
  exit 1
fi
if [ "$DRY_RUN" = "1" ]; then
  cat "$table"
  exit 0
fi
cat "$candidate" >"$readme"
note "README.md project table updated"
