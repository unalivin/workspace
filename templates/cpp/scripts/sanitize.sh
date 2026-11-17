#!/usr/bin/env bash
set -euo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

usage() {
  cat <<'USAGE'
Usage: scripts/sanitize.sh [--only address|thread] [--dry-run]

Builds the tests of the build without the dashboard with runtime checks that
the normal test run does not have, and runs them.

  Linux, macOS  1. AddressSanitizer with UndefinedBehaviorSanitizer (preset
                   asan); leak detection is on where the platform supports it
                   (Linux). The compiled-in SQLite is instrumented too.
                2. ThreadSanitizer (preset tsan).
  Windows       The sanitizer runtimes are not used with the static runtime
                library, so the tests run as an optimized build with
                assertions, checked containers and /sdl (preset checked).

Each preset has its own directory under build/, so it does not disturb the
normal build. The seed inputs of every fuzz target are replayed in the same
run.

  --only address|thread  Run one of the two sanitizers (Linux, macOS)
  --dry-run              Print the commands instead of running them
  --help                 Show this help
USAGE
}

only=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    --only)
      [ "$#" -ge 2 ] || die "--only needs address or thread"
      only="$2"
      shift
      ;;
    --dry-run) DRY_RUN=1 ;;
    --help | -h)
      usage
      exit 0
      ;;
    *) die "unknown argument: $1 (see --help)" ;;
  esac
  shift
done
case "$only" in
  "" | address | thread) ;;
  *) die "--only must be address or thread" ;;
esac

cd "$PROJECT_ROOT"
case "$(host_os)" in
  linux | macos)
    if [ "$only" != "thread" ]; then
      leaks=0
      if [ "$(host_os)" = "linux" ]; then
        leaks=1
      fi
      build_preset asan
      export ASAN_OPTIONS="detect_leaks=$leaks:strict_string_checks=1:detect_stack_use_after_return=1:abort_on_error=0"
      export UBSAN_OPTIONS="print_stacktrace=1:halt_on_error=1"
      test_preset asan
    fi
    if [ "$only" != "address" ]; then
      build_preset tsan
      export TSAN_OPTIONS="halt_on_error=1:second_deadlock_stack=1"
      test_preset tsan
    fi
    ;;
  windows)
    build_preset checked
    test_preset checked
    ;;
  *) die "unsupported operating system: $(uname -s)" ;;
esac
