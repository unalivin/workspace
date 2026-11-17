#!/usr/bin/env bash
set -euo pipefail

PROJECT_ROOT="${PROJECT_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
APP_SLUG="templateapp"
APP_NAME="Template App"
APP_PUBLISHER="Unalivin"
APP_ID="io.github.unalivin.templateapp"
APP_VERSION="0.1.0-dev"
APP_REPOSITORY_URL="https://github.com/unalivin/workspace"
APP_NUMERIC_VERSION="0.1.0"
CMAKE_MINIMUM_VERSION="3.25"
CMAKE_VERSION="4.4.5"
NINJA_VERSION="1.13.2"
CLANG_FORMAT_VERSION="23.1.5"
CLANG_TIDY_VERSION="23.1.4"
CPPCHECK_VERSION="2.22.0"
CPPCHECK_WHEEL_VERSION="1.5.3"
VCPKG_COMMIT="0df93a05b9de080129e653ec4776d7e14a1b2e4f"
VCPKG_REPOSITORY_URL="https://github.com/microsoft/vcpkg"
VCPKG_DIR="${PROJECT_VCPKG_DIR:-$PROJECT_ROOT/.tools/vcpkg}"
SOURCE_DIRECTORIES="src tests tools fuzz"
DRY_RUN="${DRY_RUN:-0}"

export APP_SLUG APP_NAME APP_PUBLISHER APP_ID APP_VERSION APP_REPOSITORY_URL APP_NUMERIC_VERSION CMAKE_MINIMUM_VERSION \
  CMAKE_VERSION NINJA_VERSION CLANG_FORMAT_VERSION CLANG_TIDY_VERSION CPPCHECK_VERSION CPPCHECK_WHEEL_VERSION \
  VCPKG_COMMIT VCPKG_REPOSITORY_URL VCPKG_DIR SOURCE_DIRECTORIES

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

have() {
  command -v "$1" >/dev/null 2>&1
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

exe_suffix() {
  if [ "$(host_os)" = "windows" ]; then
    printf '.exe'
  fi
}

native_path() {
  if [ "$(host_os)" = "windows" ] && have cygpath; then
    cygpath -m "$1"
  else
    printf '%s' "$1"
  fi
}

job_count() {
  local count=""
  if have nproc; then
    count="$(nproc 2>/dev/null || true)"
  elif have sysctl; then
    count="$(sysctl -n hw.ncpu 2>/dev/null || true)"
  fi
  case "$count" in
    "" | *[!0-9]*) count="${NUMBER_OF_PROCESSORS:-2}" ;;
  esac
  printf '%s' "$count"
}

version_at_least() {
  local found="$1" wanted="$2" first
  first="$(printf '%s\n%s\n' "$wanted" "$found" | sort -t . -k 1,1n -k 2,2n -k 3,3n | head -n 1)"
  [ "$first" = "$wanted" ]
}

cmake_ready() {
  have cmake || return 1
  version_at_least "$(cmake --version | sed -n '1s/[^0-9]*\([0-9][0-9.]*\).*/\1/p')" "$CMAKE_MINIMUM_VERSION"
}

vcpkg_root() {
  if [ -f "$VCPKG_DIR/scripts/buildsystems/vcpkg.cmake" ]; then
    printf '%s' "$VCPKG_DIR"
  elif [ -n "${VCPKG_ROOT:-}" ] && [ -f "$VCPKG_ROOT/scripts/buildsystems/vcpkg.cmake" ]; then
    printf '%s' "$VCPKG_ROOT"
  fi
}

gui_ready() {
  cmake_ready || return 1
  case "$(host_os)" in
    macos | windows) return 0 ;;
    linux)
      if [ "${PROJECT_GUI_READY:-}" = "1" ]; then
        return 0
      fi
      have pkg-config || return 1
      pkg-config --exists x11 xrandr xinerama xcursor xi gl 2>/dev/null
      ;;
    *) return 1 ;;
  esac
}

preset_config() {
  case "$1" in
    dev | nogui | asan | tsan) printf 'Debug' ;;
    release | release-nogui) printf 'Release' ;;
    checked | fuzz) printf 'RelWithDebInfo' ;;
    *) return 1 ;;
  esac
}

preset_dir() {
  printf 'build/%s' "$1"
}

configure_preset() {
  local preset="$1" root command
  preset_config "$preset" >/dev/null || die "unknown build preset: $preset"
  command=(cmake --preset "$preset")
  if [ "$(host_os)" != "windows" ] && { have ninja || [ "$DRY_RUN" = "1" ]; }; then
    command+=(-G Ninja)
  fi
  root="$(vcpkg_root)"
  if [ -n "$root" ]; then
    export VCPKG_DISABLE_METRICS=1
    if [ -z "${VCPKG_DEFAULT_BINARY_CACHE:-}" ]; then
      VCPKG_DEFAULT_BINARY_CACHE="$(native_path "$PROJECT_ROOT/.tools/vcpkg-cache")"
      export VCPKG_DEFAULT_BINARY_CACHE
    fi
    run mkdir -p "$VCPKG_DEFAULT_BINARY_CACHE"
    command+=("-DCMAKE_TOOLCHAIN_FILE=$(native_path "$root/scripts/buildsystems/vcpkg.cmake")" "-DVCPKG_INSTALL_OPTIONS=--clean-after-build")
  fi
  (cd "$PROJECT_ROOT" && run "${command[@]}")
}

build_preset() {
  local preset="$1" config command
  shift
  config="$(preset_config "$preset")" || die "unknown build preset: $preset"
  configure_preset "$preset"
  command=(cmake --build "$(preset_dir "$preset")" --config "$config" --parallel "$(job_count)")
  if [ "$#" -gt 0 ]; then
    command+=(--target "$@")
  fi
  (cd "$PROJECT_ROOT" && run "${command[@]}")
}

test_preset() {
  local preset="$1" config
  config="$(preset_config "$preset")" || die "unknown build preset: $preset"
  (cd "$PROJECT_ROOT" && run ctest --test-dir "$(preset_dir "$preset")" -C "$config" --output-on-failure)
}

built_program() {
  printf '%s/bin/%s%s' "$(preset_dir "$1")" "$2" "$(exe_suffix)"
}

source_files() {
  local directory directories
  read -r -a directories <<<"$SOURCE_DIRECTORIES"
  for directory in "${directories[@]}"; do
    [ -d "$PROJECT_ROOT/$directory" ] || continue
    (cd "$PROJECT_ROOT" && find "$directory" -type f \( -name '*.cpp' -o -name '*.hpp' \)) | LC_ALL=C sort
  done
}

fuzz_targets() {
  local file
  for file in "$PROJECT_ROOT"/fuzz/targets/*.cpp; do
    [ -f "$file" ] || continue
    basename "$file" .cpp
  done
}
