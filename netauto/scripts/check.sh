#!/usr/bin/env bash
set -euo pipefail

if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
  printf 'Usage: check.sh\nRuns gofmt, go vet, and go test.\n'
  exit 0
fi

make check
