#!/usr/bin/env bash
set -euo pipefail

if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
  printf 'Usage: demo.sh\nSeeds demo data and opens LogForge-SIEM.\n'
  exit 0
fi

cargo run -p logforge-app -- demo
cargo run -p logforge-app -- --ui tui
