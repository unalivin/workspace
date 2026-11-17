#!/usr/bin/env bash
set -euo pipefail

PROJECT_ROOT="${PROJECT_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
APP_SLUG="secretsentry"
APP_NAME="SecretSentry"
APP_PUBLISHER="Unalivin"
APP_ID="io.github.unalivin.secretsentry"
APP_VERSION="0.1.0-dev"
APP_REPOSITORY_URL="https://github.com/unalivin/workspace"
APP_NUMERIC_VERSION="0.1.0"
RUST_VERSION="1.100.0"
CARGO_AUDIT_VERSION="0.23.0"
CARGO_DENY_VERSION="0.20.2"
CARGO_FUZZ_VERSION="0.13.2"
CARGO_BUNDLE_VERSION="0.12.0"
NOGUI_PACKAGES="$APP_SLUG-core $APP_SLUG-tui $APP_SLUG-app"
DRY_RUN="${DRY_RUN:-0}"

export APP_SLUG APP_NAME APP_PUBLISHER APP_ID APP_VERSION APP_REPOSITORY_URL APP_NUMERIC_VERSION RUST_VERSION CARGO_AUDIT_VERSION \
  CARGO_DENY_VERSION CARGO_FUZZ_VERSION CARGO_BUNDLE_VERSION NOGUI_PACKAGES

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

host_triple() {
  command -v rustc >/dev/null 2>&1 || return 0
  rustc -vV 2>/dev/null | sed -n 's/^host: //p'
}

exe_suffix() {
  if [ "$(host_os)" = "windows" ]; then
    printf '.exe'
  fi
}

have_cargo_tool() {
  cargo "$1" --version >/dev/null 2>&1
}

gui_ready() {
  command -v cargo >/dev/null 2>&1 || return 1
  case "$(host_os)" in
    linux | macos) command -v cc >/dev/null 2>&1 ;;
    windows) return 0 ;;
    *) return 1 ;;
  esac
}
