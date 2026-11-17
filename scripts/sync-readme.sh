#!/usr/bin/env bash
set -euo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

usage() {
  cat <<'USAGE'
Usage: scripts/sync-readme.sh [--check] [--dry-run] [projects|destinations]

Rewrites the generated tables from the registry in scripts/projects.tsv:

  projects      The project table in README.md, between the projects:start
                and projects:end markers
  destinations  The table of project, language and destination in README.md,
                docs/WORKSPACE.md and docs/ROADMAP.md, between the
                destinations:start and destinations:end markers

Without a name both are handled.

  --check    Exit with status 1 if a file would change, write nothing
  --dry-run  Print the tables that would be written, write nothing
  --help     Show this help
USAGE
}

check=0
blocks=()
while [ "$#" -gt 0 ]; do
  case "$1" in
    --check) check=1 ;;
    --dry-run) DRY_RUN=1 ;;
    --help | -h)
      usage
      exit 0
      ;;
    projects | destinations) blocks+=("$1") ;;
    *) die "unknown argument: $1 (see --help)" ;;
  esac
  shift
done
if [ "${#blocks[@]}" -eq 0 ]; then
  blocks=(projects destinations)
fi

table="$(mktemp)"
candidate="$(mktemp)"
trap 'rm -f "$table" "$candidate"' EXIT
stale=0

sync_block() {
  local relative="$1" marker="$2" format="$3" file="$WORKSPACE_ROOT/$1"
  [ -f "$file" ] || die "$relative not found"
  grep -q "<!-- $marker:start -->" "$file" || die "$relative has no $marker:start marker"
  grep -q "<!-- $marker:end -->" "$file" || die "$relative has no $marker:end marker"
  "$WORKSPACE_ROOT/scripts/list-projects.sh" "$format" >"$table"
  awk -v table="$table" -v first="<!-- $marker:start -->" -v last="<!-- $marker:end -->" '
    index($0, first) {
      print
      while ((getline row < table) > 0) {
        print row
      }
      close(table)
      skipping = 1
      next
    }
    index($0, last) { skipping = 0 }
    !skipping { print }
  ' "$file" >"$candidate"
  if cmp -s "$file" "$candidate"; then
    note "$relative: the $marker table is up to date"
    return 0
  fi
  if [ "$check" = "1" ]; then
    printf '%s: the %s table does not match the registry\n' "$relative" "$marker" >&2
    diff -u "$file" "$candidate" >&2 || true
    stale=1
    return 0
  fi
  if [ "$DRY_RUN" = "1" ]; then
    note "dry-run: would rewrite the $marker table in $relative:"
    cat "$table"
    return 0
  fi
  cat "$candidate" >"$file"
  note "$relative: the $marker table was updated"
}

for block in "${blocks[@]}"; do
  case "$block" in
    projects) sync_block README.md projects --markdown ;;
    destinations)
      for document in README.md docs/WORKSPACE.md docs/ROADMAP.md; do
        sync_block "$document" destinations --destinations
      done
      ;;
  esac
done

if [ "$stale" = "1" ]; then
  exit 1
fi
