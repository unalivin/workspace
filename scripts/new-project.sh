#!/usr/bin/env bash
set -euo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

usage() {
  cat <<'USAGE'
Usage: scripts/new-project.sh [--dry-run] <name> <go|rust|cpp>

Creates the project folder <name> from templates/<language> and fills in its
metadata (app name, description, topics, icon concept) from the registry in
scripts/projects.tsv. The name must be listed in the registry with the same
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
tagline="$(project_tagline "$name")"
topics="$(project_topics "$name")"
icon="$(project_icon_concept "$name")"
description="$(project_description "$name")"
prefix="$(env_prefix "$name")"

if [ "$DRY_RUN" = "1" ]; then
  note "dry-run: would copy templates/$language to $name"
  note "dry-run: app name: $app"
  note "dry-run: environment prefix: $prefix"
  note "dry-run: topics: $topics"
  note "dry-run: would mark $name as In progress, refresh README.md and regenerate workflows"
  exit 0
fi

escape_replacement() {
  printf '%s' "$1" | sed -e 's/[\\&|]/\\&/g'
}

mkdir "$target"
(cd "$template" && git ls-files -co --exclude-standard -z | while IFS= read -r -d '' file; do
  if [ -f "$file" ]; then
    mkdir -p "$target/$(dirname "$file")"
    cp -p "$file" "$target/$file"
  fi
done)

while IFS= read -r -d '' file; do
  if LC_ALL=C grep -Iq . "$file"; then
    sed -i.bak \
      -e "s|TEMPLATE_DESCRIPTION|$(escape_replacement "$description")|g" \
      -e "s|TEMPLATE_TAGLINE|$(escape_replacement "$tagline")|g" \
      -e "s|TEMPLATE_TOPICS|$(escape_replacement "$topics")|g" \
      -e "s|TEMPLATE_ICON_CONCEPT|$(escape_replacement "$icon")|g" \
      -e "s|templates/$language|$(escape_replacement "$name")|g" \
      -e "s|Template App|$(escape_replacement "$app")|g" \
      -e "s|TEMPLATEAPP|$(escape_replacement "$prefix")|g" \
      -e "s|templateapp|$(escape_replacement "$name")|g" \
      "$file"
    rm -f "$file.bak"
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
