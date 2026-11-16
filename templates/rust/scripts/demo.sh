#!/usr/bin/env bash
set -euo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

usage() {
  cat <<USAGE
Usage: scripts/demo.sh [--ui gui|tui|none] [--keep] [--dry-run]

Builds $APP_NAME, creates a scratch database under dist/demo, loads the labeled
demo data and opens the application on it. Your own database and saved
choices are not touched: the demo uses its own data and config directory.

  --ui gui|tui|none  Front end to open (default: gui when the dashboard can be
                     built here, otherwise tui; none only prints the overview)
  --keep             Keep an existing scratch database instead of recreating it
  --dry-run          Print the commands instead of running them
  --help             Show this help
USAGE
}

ui=""
keep=0
while [ "$#" -gt 0 ]; do
  case "$1" in
    --ui)
      [ "$#" -ge 2 ] || die "--ui needs gui, tui or none"
      ui="$2"
      shift
      ;;
    --keep) keep=1 ;;
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

if [ -z "$ui" ]; then
  if gui_ready; then
    ui="gui"
  else
    ui="tui"
  fi
fi
case "$ui" in
  gui | tui | none) ;;
  *) die "--ui must be gui, tui or none" ;;
esac

suffix="$(exe_suffix)"
if [ "$ui" = "gui" ]; then
  run make --no-print-directory build
  binary="dist/bin/$APP_SLUG$suffix"
else
  run make --no-print-directory build-nogui
  binary="dist/bin/$APP_SLUG-nogui$suffix"
fi

demo_dir="$PROJECT_ROOT/dist/demo"
database="$demo_dir/$APP_SLUG-demo.db"
if [ "$keep" != "1" ]; then
  run rm -rf "$demo_dir"
fi
run mkdir -p "$demo_dir/config"

export TEMPLATEAPP_CONFIG_DIR="$demo_dir/config"
export TEMPLATEAPP_DB="$database"

run "$binary" db init
run "$binary" demo load
run "$binary" overview
if [ "$ui" != "none" ]; then
  run "$binary" --ui "$ui"
fi
