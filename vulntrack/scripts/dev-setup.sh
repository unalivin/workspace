#!/usr/bin/env bash
set -euo pipefail

if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
  printf 'Usage: dev-setup.sh\nChecks Go toolchain availability.\n'
  exit 0
fi

command -v go >/dev/null 2>&1 || { printf 'go not found\n' >&2; exit 1; }
printf 'Go toolchain ready: %s\n' "$(go version)"
