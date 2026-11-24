#!/usr/bin/env bash
set -euo pipefail

if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
  printf 'Usage: check.sh\nRuns fmt-check, clippy, and cargo test.\n'
  exit 0
fi

make check
