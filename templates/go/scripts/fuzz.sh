#!/usr/bin/env bash
set -euo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

usage() {
  cat <<'USAGE'
Usage: scripts/fuzz.sh [--time <duration>] [--list] [--dry-run]

Runs every Go fuzz target of the project, one after the other, for the given
duration each (default 5s). Targets are found by scanning the test files.

  --time     Duration per target, for example 5s or 2m
  --list     Print the targets and exit
  --dry-run  Print the commands instead of running them
  --help     Show this help
USAGE
}

duration="5s"
list_only=0
while [ "$#" -gt 0 ]; do
  case "$1" in
    --time)
      [ "$#" -ge 2 ] || die "--time needs a duration"
      duration="$2"
      shift
      ;;
    --list) list_only=1 ;;
    --dry-run) DRY_RUN=1 ;;
    --help | -h)
      usage
      exit 0
      ;;
    *) die "unknown argument: $1 (see --help)" ;;
  esac
  shift
done

cd "$PROJECT_ROOT"

targets=()
while IFS= read -r line; do
  targets+=("$line")
done < <(grep -rEo --include='*_test.go' '^func Fuzz[A-Za-z0-9_]+' internal tests 2>/dev/null |
  sed -E 's|/[^/]+_test\.go:func |\t|' | sort -u)

[ "${#targets[@]}" -gt 0 ] || die "no fuzz targets were found"

for target in "${targets[@]}"; do
  package="./${target%%$'\t'*}"
  name="${target##*$'\t'}"
  if [ "$list_only" = "1" ]; then
    printf '%s %s\n' "$package" "$name"
    continue
  fi
  run go test -tags "$NOGUI_TAGS" "$package" -run '^$' -fuzz "^${name}\$" -fuzztime "$duration"
done
