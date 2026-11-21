#!/usr/bin/env bash
set -uo pipefail

if [[ $# -lt 2 ]]; then
  printf 'Usage: ci-check.sh <step name> <command> [args...]\n' >&2
  exit 2
fi
label="$1"
shift
log_file="${RUNNER_TEMP:-${TMPDIR:-/tmp}}/ci-check-$$.log"
"$@" >"$log_file" 2>&1
status=$?
if [[ $status -ne 0 ]]; then
  excerpt="$(tail -c 3000 "$log_file")"
  encoded="$(printf '%s' "$excerpt" | sed -e 's/%/%25/g' -e ':a' -e 'N' -e '$!ba' -e 's/\n/%0A/g' -e 's/\r/%0D/g')"
  printf '::error title=%s::%s\n' "$label" "$encoded"
  exit "$status"
fi
cat "$log_file"
