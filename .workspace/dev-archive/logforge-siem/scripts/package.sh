#!/usr/bin/env bash
set -euo pipefail

target=""
output=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --help|-h)
      printf 'Usage: package.sh --target <linux-x64|macos-arm64|windows-x64> --output <path>\n'
      exit 0
      ;;
    --target)
      target="${2:-}"
      shift 2
      ;;
    --output)
      output="${2:-}"
      shift 2
      ;;
    *)
      printf 'Unknown argument: %s\n' "$1" >&2
      exit 1
      ;;
  esac
done

if [[ -z "$target" || -z "$output" ]]; then
  printf 'Both --target and --output are required\n' >&2
  exit 1
fi

mkdir -p "$(dirname "$output")"
cargo build --release -p logforge-app

case "$target" in
  linux-x64|macos-arm64)
    stage="$(mktemp -d)"
    trap 'rm -rf "$stage"' EXIT
    cp target/release/logforge-app "$stage/logforge-siem"
    cp LICENSE THIRD_PARTY_NOTICES.md README.md docs/USAGE.md "$stage/"
    tar -czf "$output" -C "$stage" .
    ;;
  windows-x64)
    cp target/release/logforge-app.exe "$output"
    ;;
  *)
    printf 'Unsupported target: %s\n' "$target" >&2
    exit 1
    ;;
esac
