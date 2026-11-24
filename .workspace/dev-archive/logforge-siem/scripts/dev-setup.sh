#!/usr/bin/env bash
set -euo pipefail

if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
  printf 'Usage: dev-setup.sh\nChecks Rust toolchain availability.\n'
  exit 0
fi

command -v cargo >/dev/null 2>&1 || { printf 'cargo not found\n' >&2; exit 1; }
command -v rustc >/dev/null 2>&1 || { printf 'rustc not found\n' >&2; exit 1; }
printf 'Rust toolchain ready: %s\n' "$(rustc --version)"
