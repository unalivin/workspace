#!/usr/bin/env bash
set -euo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

usage() {
  cat <<'USAGE'
Usage: scripts/check-all.sh [--list] [--dry-run] [--fail-fast] [target...]

Runs "make check" in every target and prints a summary.
Without arguments the targets are every language template that exists
(templates/go, templates/rust, templates/cpp) followed by every project
folder that exists. A target is a project folder name or a template path.

  --list       Print the targets that would be checked and exit
  --dry-run    Print the commands without running them
  --fail-fast  Stop at the first failing target
  --help       Show this help
USAGE
}

list_only=0
fail_fast=0
targets=()
while [ "$#" -gt 0 ]; do
  case "$1" in
    --list) list_only=1 ;;
    --dry-run) DRY_RUN=1 ;;
    --fail-fast) fail_fast=1 ;;
    --help | -h)
      usage
      exit 0
      ;;
    -*) die "unknown option: $1 (see --help)" ;;
    *) targets+=("$1") ;;
  esac
  shift
done

if [ "${#targets[@]}" -eq 0 ]; then
  while IFS= read -r language; do
    targets+=("templates/$language")
  done < <(existing_templates)
  while IFS= read -r folder; do
    targets+=("$folder")
  done < <(existing_projects)
fi

for target in "${targets[@]}"; do
  case "$target" in
    /* | *..*) die "target must be a path inside the workspace: $target" ;;
  esac
  [ -f "$WORKSPACE_ROOT/$target/Makefile" ] || die "target has no Makefile: $target"
done

if [ "$list_only" = "1" ]; then
  if [ "${#targets[@]}" -gt 0 ]; then
    printf '%s\n' "${targets[@]}"
  fi
  exit 0
fi

if [ "${#targets[@]}" -eq 0 ]; then
  note "no templates or projects exist yet; nothing to check"
  exit 0
fi

passed=()
failed=()
for target in "${targets[@]}"; do
  note "==> $target"
  if run make -C "$WORKSPACE_ROOT/$target" check; then
    passed+=("$target")
  else
    failed+=("$target")
    if [ "${GITHUB_ACTIONS:-}" = "true" ]; then
      printf '::error title=check-all::make check failed in %s\n' "$target"
    fi
    if [ "$fail_fast" = "1" ]; then
      break
    fi
  fi
done

note ""
note "passed: ${#passed[@]}"
note "failed: ${#failed[@]}"
if [ "${#failed[@]}" -gt 0 ]; then
  printf 'failed target: %s\n' "${failed[@]}" >&2
  exit 1
fi
