#!/usr/bin/env bash
set -euo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

usage() {
  cat <<'USAGE'
Usage: scripts/ci-step.sh <make-target>...
       scripts/ci-step.sh --command <label> <program> [arguments]

Runs make targets (or one command) from the project root, keeps the complete
output in the log and, when a step fails inside GitHub Actions, prints the
failing tests, compiler errors and last lines of that step as an error
annotation so the failure can be read from the check run without opening the
log.

  --command <label>  Run a program instead of make targets
  --help             Show this help
USAGE
}

tail_lines=12
finding_lines=24
line_width=240

findings() {
  grep -E -A 3 -- '^error(\[E[0-9]+\])?:|^warning: unused|panicked at|^test .* \.\.\. FAILED|^not ok |^Diff in |^Crate:|^ID:|^Solution:|^[[:space:]]+--> |undefined reference|cannot find|ERROR: AddressSanitizer|ERROR: LeakSanitizer' "$1" |
    grep -v -- '^--$' | sed -e 's/[[:space:]]*$//' | grep -v '^$' | cut -c "1-$line_width" | head -n "$finding_lines" || true
}

ending() {
  sed -e 's/[[:space:]]*$//' "$1" | grep -v '^$' | cut -c "1-$line_width" | tail -n "$tail_lines" || true
}

summary() {
  local found
  found="$(findings "$1")"
  if [ -n "$found" ]; then
    printf '%s\n...\n' "$found"
  fi
  ending "$1"
}

escape_annotation() {
  local text="$1"
  text="${text//'%'/%25}"
  text="${text//$'\r'/}"
  text="${text//$'\n'/%0A}"
  printf '%s' "$text"
}

run_step() {
  local label="$1" log status=0
  shift
  log="$(mktemp)"
  note "==> $label"
  set +e
  "$@" 2>&1 | tee "$log"
  status="${PIPESTATUS[0]}"
  set -e
  if [ "$status" != "0" ]; then
    printf 'step failed: %s (exit %s)\n' "$label" "$status" >&2
    if [ "${GITHUB_ACTIONS:-}" = "true" ]; then
      printf '::error title=%s: %s failed (%s)::%s\n' "$APP_SLUG" "$label" "$(host_os)" "$(escape_annotation "$(summary "$log")")"
    fi
    rm -f "$log"
    return "$status"
  fi
  rm -f "$log"
}

if [ "$#" -eq 0 ]; then
  usage >&2
  exit 2
fi

cd "$PROJECT_ROOT"

case "$1" in
  --help | -h)
    usage
    exit 0
    ;;
  --command)
    [ "$#" -ge 3 ] || die "--command needs a label and a program"
    label="$2"
    shift 2
    run_step "$label" "$@"
    ;;
  -*) die "unknown option: $1 (see --help)" ;;
  *)
    for target in "$@"; do
      run_step "make $target" make --no-print-directory "$target"
    done
    ;;
esac
