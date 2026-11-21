#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$script_dir/lib.sh"

usage() {
  cat <<'EOF'
Usage: check-all.sh [project...]

Runs make check in each specified project folder, or all existing projects.
EOF
}

if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
  usage
  exit 0
fi

root="$(repo_root)"
targets=("$@")

if [[ ${#targets[@]} -eq 0 ]]; then
  while IFS=$'\t' read -r folder _app _lang _desc _dest _copy _status; do
    if [[ -d "$root/$folder" && -f "$root/$folder/Makefile" ]]; then
      targets+=("$folder")
    fi
  done < <(read_registry)
fi

if [[ ${#targets[@]} -eq 0 ]]; then
  printf 'No project folders with a Makefile found yet.\n'
  exit 0
fi

failed=0
for proj in "${targets[@]}"; do
  if [[ ! -d "$root/$proj" ]]; then
    printf 'Project folder not found: %s\n' "$proj" >&2
    failed=1
    continue
  fi
  printf 'Running make check in %s...\n' "$proj"
  if ! make -C "$root/$proj" check; then
    printf 'Check failed in %s\n' "$proj" >&2
    failed=1
  fi
done

exit "$failed"
