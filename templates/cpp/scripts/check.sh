#!/usr/bin/env bash
set -euo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

usage() {
  cat <<'USAGE'
Usage: scripts/check.sh [--require-gui] [--require-all] [--dry-run]

Runs the full local quality suite and prints a summary:

  format, static analysis, tests and the sanitizers on the build without the
  dashboard, generated documents (command tables, icons, notices), the
  release build without the dashboard, shell script checks, the supply chain
  checks, and the dashboard build with its tests and analysis when the
  window system headers are installed.

A check whose tool is not installed is reported as skipped with the reason.
The exit status is 1 when a check fails.

  --require-gui  Fail instead of skipping when the dashboard cannot be built
  --require-all  Fail when any check had to be skipped
  --dry-run      Print the commands instead of running them
  --help         Show this help
USAGE
}

require_gui=0
require_all=0
while [ "$#" -gt 0 ]; do
  case "$1" in
    --require-gui) require_gui=1 ;;
    --require-all) require_all=1 ;;
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

passed=()
failed=()
skipped=()

step() {
  local target="$1"
  note "==> make $target"
  if run make --no-print-directory "$target"; then
    passed+=("$target")
  else
    failed+=("$target")
  fi
}

skip() {
  note "==> skipped $1: $2"
  skipped+=("$1 ($2)")
}

if have clang-format || [ "$DRY_RUN" = "1" ]; then
  step fmt-check
else
  skip fmt-check "clang-format is not installed"
fi
if have cppcheck || [ "$DRY_RUN" = "1" ]; then
  step lint
else
  skip lint "cppcheck is not installed"
fi
step test-nogui
step race-or-sanitize
step docs-check
step assets-check
step notices-check
step build-nogui
if have shellcheck && have shfmt && have bats; then
  step shell
else
  skip shell "shellcheck, shfmt or bats is not installed"
fi
if have curl || [ "$DRY_RUN" = "1" ]; then
  step vuln
else
  skip vuln "curl is not installed"
fi

if gui_ready; then
  step lint-gui
  step test
  step build
elif [ "$require_gui" = "1" ]; then
  failed+=("dashboard build (the window system headers are missing)")
else
  skip "dashboard build" "the window system headers are missing, see scripts/dev-setup.sh"
fi

note ""
note "passed:  ${#passed[@]}"
note "failed:  ${#failed[@]}"
note "skipped: ${#skipped[@]}"
if [ "${#skipped[@]}" -gt 0 ]; then
  printf '  skipped %s\n' "${skipped[@]}"
fi
if [ "${#failed[@]}" -gt 0 ]; then
  printf '  failed %s\n' "${failed[@]}" >&2
  exit 1
fi
if [ "$require_all" = "1" ] && [ "${#skipped[@]}" -gt 0 ]; then
  die "checks were skipped and --require-all was given"
fi
