#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$script_dir/lib.sh"

usage() {
  cat <<'EOF'
Usage: sync-readme.sh [--dry-run]

Regenerates the project and destination tables in README.md from scripts/projects.tsv.
EOF
}

dry_run=0
while [[ $# -gt 0 ]]; do
  case "$1" in
    --help|-h)
      usage
      exit 0
      ;;
    --dry-run)
      dry_run=1
      shift
      ;;
    *)
      printf 'Unknown argument: %s\n' "$1" >&2
      exit 1
      ;;
  esac
done

root="$(repo_root)"
readme="$root/README.md"

if [[ ! -f "$readme" ]]; then
  printf 'README.md not found at %s\n' "$readme" >&2
  exit 1
fi

projects_table="$(
  printf '| Folder | App | Language | Description | Status | Docs |\n'
  printf '|---|---|---|---|---|---|\n'
  while IFS=$'\t' read -r folder app language description _dest _copy status; do
    docs_link="-"
    if [[ -f "$root/$folder/README.md" ]]; then
      docs_link="[docs]($folder/docs/README.md)"
    fi
    printf '| `%s` | %s | %s | %s | %s | %s |\n' \
      "$folder" "$app" "$language" "$description" "$status" "$docs_link"
  done < <(read_registry)
)"

destinations_table="$(
  printf '| Folder | Language | Destination |\n'
  printf '|---|---|---|\n'
  while IFS=$'\t' read -r folder _app language _desc destination _copy _status; do
    dest_label="Personal (unalivin)"
    if [[ "$destination" == "sonix-devs" ]]; then
      dest_label="Sonix-Devs"
    fi
    printf '| `%s` | %s | %s |\n' "$folder" "$language" "$dest_label"
  done < <(read_registry)
)"

tmp_file="$(mktemp)"
trap 'rm -f "$tmp_file"' EXIT

awk -v ptab="$projects_table" -v dtab="$destinations_table" '
  BEGIN { in_p=0; in_d=0 }
  /<!-- PROJECTS_TABLE_START -->/ { print; print ptab; in_p=1; next }
  /<!-- PROJECTS_TABLE_END -->/ { in_p=0; print; next }
  /<!-- DESTINATIONS_TABLE_START -->/ { print; print dtab; in_d=1; next }
  /<!-- DESTINATIONS_TABLE_END -->/ { in_d=0; print; next }
  !in_p && !in_d { print }
' "$readme" > "$tmp_file"

if [[ "$dry_run" -eq 1 ]]; then
  diff -u "$readme" "$tmp_file" || true
else
  cp "$tmp_file" "$readme"
fi
