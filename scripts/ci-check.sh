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
  first_excerpt="$(head -c 3000 "$log_file")"
  last_excerpt="$(tail -c 3000 "$log_file")"
  encode_annotation() {
    printf '%s' "$1" | sed -e 's/%/%25/g' -e ':a' -e 'N' -e '$!ba' -e 's/\n/%0A/g' -e 's/\r/%0D/g'
  }
  first_encoded="$(encode_annotation "$first_excerpt")"
  last_encoded="$(encode_annotation "$last_excerpt")"
  diagnostics="$(grep -E 'error\[[A-Z][0-9]+\]|error:|could not compile|^[[:space:]]*--> ' "$log_file" | tail -n 80 || true)"
  diagnostics_encoded="$(encode_annotation "$diagnostics")"
  printf '::error title=%s (compiler diagnostics)::%s\n' "$label" "$diagnostics_encoded"
  printf '::error title=%s (beginning)::%s\n' "$label" "$first_encoded"
  if [[ "$first_excerpt" != "$last_excerpt" ]]; then
    printf '::error title=%s (last 3000 characters)::%s\n' "$label" "$last_encoded"
  fi
  exit "$status"
fi
cat "$log_file"
