#!/usr/bin/env bash
set -euo pipefail

WORKSPACE_ROOT="${WORKSPACE_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
REGISTRY_FILE="${REGISTRY_FILE:-$WORKSPACE_ROOT/scripts/projects.tsv}"
WORKSPACE_OWNER="${WORKSPACE_OWNER:-NexzaDev}"
WORKSPACE_REPO="${WORKSPACE_REPO:-workspace}"
TEMPLATE_LANGUAGES="go rust cpp"
DRY_RUN="${DRY_RUN:-0}"

die() {
  printf 'error: %s\n' "$*" >&2
  if [ "${GITHUB_ACTIONS:-}" = "true" ]; then
    printf '::error::%s\n' "$*"
  fi
  exit 1
}

note() {
  printf '%s\n' "$*"
}

run() {
  if [ "$DRY_RUN" = "1" ]; then
    printf 'dry-run:'
    printf ' %q' "$@"
    printf '\n'
    return 0
  fi
  "$@"
}

registry_rows() {
  [ -f "$REGISTRY_FILE" ] || die "registry not found: $REGISTRY_FILE"
  tail -n +2 "$REGISTRY_FILE"
}

registry_folders() {
  registry_rows | cut -f2
}

registry_has() {
  registry_folders | grep -Fxq -- "$1"
}

registry_field() {
  local folder="$1" index="$2"
  registry_rows | awk -F'\t' -v folder="$folder" -v index_="$index" '$2 == folder { print $index_; found = 1 } END { exit found ? 0 : 1 }'
}

project_app() { registry_field "$1" 3; }
project_language() { registry_field "$1" 4; }
project_status() { registry_field "$1" 5; }
project_tagline() { registry_field "$1" 6; }
project_topics() { registry_field "$1" 7; }
project_icon_concept() { registry_field "$1" 8; }
project_description() { registry_field "$1" 9; }

existing_projects() {
  local folder
  while IFS= read -r folder; do
    if [ -d "$WORKSPACE_ROOT/$folder" ]; then
      printf '%s\n' "$folder"
    fi
  done < <(registry_folders)
}

existing_templates() {
  local language
  for language in $TEMPLATE_LANGUAGES; do
    if [ -f "$WORKSPACE_ROOT/templates/$language/Makefile" ]; then
      printf '%s\n' "$language"
    fi
  done
}

valid_language() {
  case "$1" in
    go | rust | cpp) return 0 ;;
    *) return 1 ;;
  esac
}

language_label() {
  case "$1" in
    go) printf 'Go' ;;
    rust) printf 'Rust' ;;
    cpp) printf 'C++' ;;
    *) return 1 ;;
  esac
}

env_prefix() {
  printf '%s' "$1" | tr '[:lower:]-' '[:upper:]_'
}
