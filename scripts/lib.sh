#!/usr/bin/env bash
set -euo pipefail

WORKSPACE_ROOT="${WORKSPACE_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
REGISTRY_FILE="${REGISTRY_FILE:-$WORKSPACE_ROOT/scripts/projects.tsv}"
WORKSPACE_OWNER="${WORKSPACE_OWNER:-unalivin}"
WORKSPACE_REPO="${WORKSPACE_REPO:-workspace}"
TEMPLATE_LANGUAGES="go rust cpp"
COMPANY_OWNER="${COMPANY_OWNER:-Sonix-Devs}"
COMPANY_HOLDER="Sonix Devs"
PERSONAL_HOLDER="Unalivin"
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
project_destination() { registry_field "$1" 10; }
project_copyright_holder() { registry_field "$1" 11; }
project_owner() { destination_owner "$(project_destination "$1")"; }

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

lower_case() {
  printf '%s' "$1" | tr '[:upper:]' '[:lower:]'
}

valid_destination() {
  case "$1" in
    sonix-devs | personal) return 0 ;;
    *) return 1 ;;
  esac
}

destination_owner() {
  case "$1" in
    sonix-devs) printf '%s' "$COMPANY_OWNER" ;;
    personal) printf '%s' "$WORKSPACE_OWNER" ;;
    *) return 1 ;;
  esac
}

destination_holder() {
  case "$1" in
    sonix-devs) printf '%s' "$COMPANY_HOLDER" ;;
    personal) printf '%s' "$PERSONAL_HOLDER" ;;
    *) return 1 ;;
  esac
}

destination_label() {
  case "$1" in
    sonix-devs) printf '%s organization (company)' "$COMPANY_OWNER" ;;
    personal) printf 'Personal profile (%s)' "$WORKSPACE_OWNER" ;;
    *) return 1 ;;
  esac
}

other_holder() {
  if [ "$1" = "$COMPANY_HOLDER" ]; then
    printf '%s' "$PERSONAL_HOLDER"
  else
    printf '%s' "$COMPANY_HOLDER"
  fi
}

escape_replacement() {
  printf '%s' "$1" | sed -e 's/[\\&|]/\\&/g'
}

escape_pattern() {
  printf '%s' "$1" | sed -e 's/[][\\.*^$|]/\\&/g'
}

is_text_file() {
  LC_ALL=C grep -Iq . "$1"
}

project_token_script() {
  local name="$1" language owner holder
  language="$(project_language "$name")"
  owner="$(project_owner "$name")"
  holder="$(project_copyright_holder "$name")"
  printf 's|%s|%s|g\n' \
    "TEMPLATE_DESCRIPTION" "$(escape_replacement "$(project_description "$name")")" \
    "TEMPLATE_TAGLINE" "$(escape_replacement "$(project_tagline "$name")")" \
    "TEMPLATE_TOPICS" "$(escape_replacement "$(project_topics "$name")")" \
    "TEMPLATE_ICON_CONCEPT" "$(escape_replacement "$(project_icon_concept "$name")")" \
    "github\\.com/$(escape_pattern "$WORKSPACE_OWNER")/templateapp" "github.com/$(escape_replacement "$owner")/$name" \
    "io\\.github\\.$(escape_pattern "$(lower_case "$WORKSPACE_OWNER")")\\.templateapp" "io.github.$(escape_replacement "$(lower_case "$owner")").$name" \
    "$(escape_pattern "$PERSONAL_HOLDER")" "$(escape_replacement "$holder")" \
    "templates/$language" "$name" \
    "Template App" "$(escape_replacement "$(project_app "$name")")" \
    "TEMPLATEAPP" "$(escape_replacement "$(env_prefix "$name")")" \
    "templateapp" "$name"
}

apply_token_script() {
  local script="$1" file="$2"
  sed -i.bak -f "$script" "$file"
  rm -f "$file.bak"
}

readme_license_section() {
  awk '/^## License$/ { inside = 1; next } /^## / { inside = 0 } inside { print }' "$1"
}

holder_problems() {
  local base="$1" label="$2" holder="$3" other pattern found file
  other="$(other_holder "$holder")"
  pattern="$(escape_pattern "$holder")"
  if [ ! -f "$base/LICENSE" ] || ! grep -Eq "^Copyright \(c\) [0-9]{4}(-[0-9]{4})? ${pattern}\$" "$base/LICENSE"; then
    printf '%s: LICENSE has no line "Copyright (c) <year> %s"\n' "$label" "$holder"
  fi
  if [ ! -f "$base/README.md" ] || ! readme_license_section "$base/README.md" | grep -Fq -- "$holder"; then
    printf '%s: the License section of README.md does not name %s\n' "$label" "$holder"
  fi
  if [ ! -f "$base/THIRD_PARTY_NOTICES.md" ] || ! head -n 8 "$base/THIRD_PARTY_NOTICES.md" | grep -Fq -- "$holder"; then
    printf '%s: the header of THIRD_PARTY_NOTICES.md does not name %s\n' "$label" "$holder"
  fi
  if [ ! -f "$base/scripts/lib.sh" ] || ! grep -Eq "^APP_PUBLISHER=\"${pattern}\"\$" "$base/scripts/lib.sh"; then
    printf '%s: scripts/lib.sh does not set APP_PUBLISHER="%s"\n' "$label" "$holder"
  fi
  found=0
  while IFS= read -r file; do
    if grep -Eiq "publisher[^\"]*\"${pattern}\"" "$file"; then
      found=1
    fi
  done < <(find "$base" -type f \( -name 'meta.go' -o -name 'meta.rs' -o -name 'meta.hpp' -o -name 'meta.cpp' \) \
    -not -path '*/target/*' -not -path '*/build/*' -not -path '*/dist/*' -not -path '*/.git/*')
  if [ "$found" = "0" ]; then
    printf '%s: no meta source file sets the publisher to "%s"\n' "$label" "$holder"
  fi
  while IFS= read -r file; do
    printf '%s: %s names %s, but the registry says %s\n' "$label" "${file#"$base"/}" "$other" "$holder"
  done < <(grep -rIlF --exclude-dir=.git --exclude-dir=target --exclude-dir=build --exclude-dir=dist \
    --exclude-dir=node_modules -- "$other" "$base" 2>/dev/null | sort)
}
