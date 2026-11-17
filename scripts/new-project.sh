#!/usr/bin/env bash
set -euo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

usage() {
  cat <<'USAGE'
Usage: scripts/new-project.sh [--dry-run] <name> <go|rust|cpp>

Creates the project folder <name> from templates/<language> and fills in its
metadata (app name, description, topics, icon concept, repository owner and
copyright holder of its destination) from the registry in scripts/projects.tsv.
The folder gets its own .github/PULL_REQUEST_TEMPLATE.md from the template. The name must be listed in the registry with the same
primary language. Afterwards the registry status becomes "In progress", the
README table is refreshed and the workflows are regenerated.

  --dry-run  Print what would happen without creating anything
  --help     Show this help
USAGE
}

args=()
while [ "$#" -gt 0 ]; do
  case "$1" in
    --dry-run) DRY_RUN=1 ;;
    --help | -h)
      usage
      exit 0
      ;;
    -*) die "unknown option: $1 (see --help)" ;;
    *) args+=("$1") ;;
  esac
  shift
done
[ "${#args[@]}" -eq 2 ] || die "expected <name> and <language> (see --help)"
name="${args[0]}"
language="${args[1]}"

[[ "$name" =~ ^[a-z][a-z0-9-]*$ ]] || die "invalid project name: $name (lower-case letters, digits and hyphens)"
valid_language "$language" || die "unknown language: $language (use go, rust or cpp)"
registry_has "$name" || die "$name is not in scripts/projects.tsv; add a registry row first (docs/ADD_PROJECT.md)"
expected="$(project_language "$name")"
[ "$expected" = "$language" ] || die "$name is registered as a $expected project, not $language"
template="$WORKSPACE_ROOT/templates/$language"
[ -f "$template/Makefile" ] || die "template templates/$language does not exist yet"
target="$WORKSPACE_ROOT/$name"
[ ! -e "$target" ] || die "$name already exists"

app="$(project_app "$name")"
topics="$(project_topics "$name")"
prefix="$(env_prefix "$name")"
destination="$(project_destination "$name")"
valid_destination "$destination" || die "$name has no valid destination in the registry"
owner="$(project_owner "$name")"
holder="$(project_copyright_holder "$name")"

if [ "$DRY_RUN" = "1" ]; then
  note "dry-run: would copy templates/$language to $name"
  note "dry-run: app name: $app"
  note "dry-run: environment prefix: $prefix"
  note "dry-run: topics: $topics"
  note "dry-run: destination: $(destination_label "$destination"), repository owner $owner"
  note "dry-run: copyright holder: $holder"
  note "dry-run: would mark $name as In progress, refresh README.md and regenerate workflows"
  exit 0
fi

mkdir "$target"
(cd "$template" && git ls-files -co --exclude-standard -z | while IFS= read -r -d '' file; do
  if [ -f "$file" ]; then
    mkdir -p "$target/$(dirname "$file")"
    cp -p "$file" "$target/$file"
  fi
done)

tokens="$(mktemp)"
trap 'rm -f "$tokens"' EXIT
project_token_script "$name" >"$tokens"
while IFS= read -r -d '' file; do
  if is_text_file "$file"; then
    apply_token_script "$tokens" "$file"
  fi
done < <(find "$target" -type f -print0)

while IFS= read -r path; do
  renamed="$(dirname "$path")/$(basename "$path" | sed -e "s|templateapp|$name|g")"
  mv "$path" "$renamed"
done < <(find "$target" -depth -name '*templateapp*')

awk -F'\t' -v OFS='\t' -v folder="$name" '$2 == folder { $5 = "In progress" } { print }' "$REGISTRY_FILE" >"$REGISTRY_FILE.tmp"
mv "$REGISTRY_FILE.tmp" "$REGISTRY_FILE"

"$WORKSPACE_ROOT/scripts/sync-readme.sh" >/dev/null
"$WORKSPACE_ROOT/scripts/gen-workflows.sh" >/dev/null

note "created $name from templates/$language"
note "next: make -C $name check"
