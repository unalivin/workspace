#!/usr/bin/env bash
set -euo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

usage() {
  cat <<USAGE
Usage: scripts/window-check.sh [--seconds <n>] [--dry-run]

Builds $APP_NAME with the dashboard, starts it with --ui gui on a virtual X
display (Xvfb) with a scratch database that holds the demo records, and fails
unless the window is still open after the waiting time. This runs the window
host, the OpenGL backend and the event loop, which the tests and the
screenshots do not. It runs on Linux only and needs Xvfb and a software
OpenGL driver (packages xvfb and libgl1-mesa-dri on Debian and Ubuntu).

  --seconds <n>  How long the window has to stay open (default 8)
  --dry-run      Print the commands instead of running them
  --help         Show this help
USAGE
}

seconds=8
inside=0
while [ "$#" -gt 0 ]; do
  case "$1" in
    --seconds)
      [ "$#" -ge 2 ] || die "--seconds needs a number"
      seconds="$2"
      shift
      ;;
    --inside) inside=1 ;;
    --dry-run) DRY_RUN=1 ;;
    --help | -h)
      usage
      exit 0
      ;;
    *) die "unknown argument: $1 (see --help)" ;;
  esac
  shift
done

case "$seconds" in
  "" | *[!0-9]* | 0) die "--seconds must be a whole number above zero" ;;
esac

cd "$PROJECT_ROOT"
binary="$(built_program release "$APP_SLUG")"

if [ "$inside" = "1" ]; then
  scratch="$(mktemp -d)"
  trap 'rm -rf "$scratch"' EXIT
  export TEMPLATEAPP_CONFIG_DIR="$scratch/config"
  export TEMPLATEAPP_DB="$scratch/window-check.db"
  "$binary" demo load >/dev/null
  "$binary" --ui gui >"$scratch/output.log" 2>&1 &
  pid="$!"
  sleep "$seconds"
  if kill -0 "$pid" 2>/dev/null; then
    kill "$pid"
    wait "$pid" 2>/dev/null || true
    note "the dashboard window stayed open for $seconds seconds on $DISPLAY"
    exit 0
  fi
  status=0
  wait "$pid" || status="$?"
  cat "$scratch/output.log" >&2
  die "the dashboard window closed by itself with exit status $status"
fi

if [ "$DRY_RUN" != "1" ]; then
  [ "$(host_os)" = "linux" ] || die "the window check runs on Linux: it needs a virtual X display"
  have xvfb-run || die "xvfb-run is not installed: run scripts/dev-setup.sh --install"
fi
build_preset release "$APP_SLUG"
run xvfb-run --auto-servernum --server-args="-screen 0 1400x900x24" "$PROJECT_ROOT/scripts/window-check.sh" --inside --seconds "$seconds"
