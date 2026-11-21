#!/usr/bin/env bash
set -euo pipefail

repo_root() {
  local dir
  dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
  printf '%s\n' "$dir"
}

registry_path() {
  printf '%s/scripts/projects.tsv\n' "$(repo_root)"
}

read_registry() {
  local reg
  reg="$(registry_path)"
  if [[ ! -f "$reg" ]]; then
    printf 'Registry not found: %s\n' "$reg" >&2
    return 1
  fi
  tail -n +2 "$reg"
}

lookup_project() {
  local target="$1"
  local line folder app language description destination copyright_holder status
  while IFS=$'\t' read -r folder app language description destination copyright_holder status; do
    if [[ "$folder" == "$target" ]]; then
      printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
        "$folder" "$app" "$language" "$description" "$destination" "$copyright_holder" "$status"
      return 0
    fi
  done < <(read_registry)
  return 1
}

owner_for_destination() {
  local destination="$1"
  if [[ "$destination" == "sonix-devs" ]]; then
    printf 'sonix-devs\n'
  else
    printf 'unalivin\n'
  fi
}
