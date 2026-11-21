#!/usr/bin/env bash
set -euo pipefail

if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
  printf 'Usage: demo.sh\nSeeds demo data and opens NetAuto.\n'
  exit 0
fi

go run ./cmd/netauto demo
go run ./cmd/netauto --ui tui
