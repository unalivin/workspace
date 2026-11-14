#!/usr/bin/env bash
set -euo pipefail

PROJECT_ROOT="${PROJECT_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
APP_SLUG="templateapp"
APP_NAME="Template App"
APP_ID="io.github.nexzadev.templateapp"
APP_VERSION="0.1.0-dev"
APP_REPOSITORY_URL="https://github.com/NexzaDev/workspace"
APP_NUMERIC_VERSION="0.1.0"
GO_VERSION="1.27.3"
STATICCHECK_VERSION="v0.8.2"
GOVULNCHECK_VERSION="v1.9.0"
FYNE_TOOLS_VERSION="v1.7.4"
BASE_TAGS="sqlite_fts5"
GUI_TAGS="$BASE_TAGS no_animations"
NOGUI_TAGS="$BASE_TAGS nogui"
DRY_RUN="${DRY_RUN:-0}"

export APP_SLUG APP_NAME APP_ID APP_VERSION APP_REPOSITORY_URL APP_NUMERIC_VERSION GO_VERSION STATICCHECK_VERSION GOVULNCHECK_VERSION \
  FYNE_TOOLS_VERSION BASE_TAGS GUI_TAGS NOGUI_TAGS

annotate() {
  if [ "${GITHUB_ACTIONS:-}" = "true" ]; then
    printf '::error title=%s::%s\n' "$1" "$2"
  fi
}

die() {
  printf 'error: %s\n' "$*" >&2
  annotate "$APP_SLUG" "$*"
  exit 1
}

note() {
  printf '%s\n' "$*"
}

run() {
  if [ "$DRY_RUN" = "1" ]; then
    printf 'dry-run:'
    printf ' %q' "$@"
    printf '\n'
    return 0
  fi
  "$@"
}

host_os() {
  case "$(uname -s)" in
    Linux*) printf 'linux' ;;
    Darwin*) printf 'macos' ;;
    MINGW* | MSYS* | CYGWIN*) printf 'windows' ;;
    *) printf 'unknown' ;;
  esac
}

host_arch() {
  case "$(uname -m)" in
    x86_64 | amd64) printf 'x64' ;;
    arm64 | aarch64) printf 'arm64' ;;
    *) uname -m ;;
  esac
}

gui_ready() {
  command -v go >/dev/null 2>&1 || return 1
  [ "$(go env CGO_ENABLED 2>/dev/null)" = "1" ] || return 1
  case "$(host_os)" in
    linux)
      [ -f /usr/include/X11/Xlib.h ] && [ -f /usr/include/GL/gl.h ] && [ -f /usr/include/xkbcommon/xkbcommon.h ] &&
        [ -f /usr/include/wayland-client-core.h ]
      ;;
    macos | windows) command -v "$(go env CC)" >/dev/null 2>&1 ;;
    *) return 1 ;;
  esac
}
