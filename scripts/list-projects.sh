#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$script_dir/lib.sh"

usage() {
  cat <<'EOF'
Usage: list-projects.sh [--language go|rust|cpp] [--destination sonix-devs|personal]

Lists projects from scripts/projects.tsv.
EOF
}

filter_lang=""
filter_dest=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --help|-h)
      usage
      exit 0
      ;;
    --language)
      filter_lang="${2:-}"
      shift 2
      ;;
    --destination)
      filter_dest="${2:-}"
      shift 2
      ;;
    *)
      printf 'Unknown argument: %s\n' "$1" >&2
      usage >&2
      exit 1
      ;;
  esac
done

while IFS=$'\t' read -r folder app language description destination copyright_holder status; do
  if [[ -n "$filter_lang" && "$language" != "$filter_lang" ]]; then
    continue
  fi
  if [[ -n "$filter_dest" && "$destination" != "$filter_dest" ]]; then
    continue
  fi
  printf '%s\t%s\t%s\t%s\t%s\n' "$folder" "$app" "$language" "$destination" "$status"
done < <(read_registry)
