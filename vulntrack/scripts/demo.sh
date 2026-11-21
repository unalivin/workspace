#!/usr/bin/env bash
set -euo pipefail

if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
  printf 'Usage: demo.sh\nSeeds demo data and opens VulnTrack.\n'
  exit 0
fi

go run ./cmd/vulntrack demo
go run ./cmd/vulntrack --ui tui
