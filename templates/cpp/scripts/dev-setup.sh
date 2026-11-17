#!/usr/bin/env bash
set -euo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

linux_packages=(build-essential pkg-config curl zip unzip tar git libxinerama-dev libxcursor-dev xorg-dev libglu1-mesa-dev)
linux_window_packages=(xvfb libgl1-mesa-dri)
shellcheck_package="shellcheck-py==0.11.0.1"
shfmt_package="shfmt-py==4.2.0"
bats_package="bats@1.13.0"

usage() {
  cat <<USAGE
Usage: scripts/dev-setup.sh [--install] [--dry-run]

Checks the prerequisites for building $APP_NAME and reports what is missing.

  A C++20 compiler: GCC 12, Clang 16, Apple Clang 15, Visual Studio 2022 or newer
  CMake $CMAKE_MINIMUM_VERSION or newer, and Ninja on Linux and macOS
  vcpkg at commit $VCPKG_COMMIT in .tools/vcpkg (it builds the
    libraries pinned in vcpkg.json; set PROJECT_VCPKG_DIR to keep it elsewhere)
  Window system headers for the dashboard
    Linux (Debian, Ubuntu): ${linux_packages[*]}
    macOS: Xcode command line tools
    Windows: Build Tools for Visual Studio with the Windows SDK
  clang-format $CLANG_FORMAT_VERSION, clang-tidy $CLANG_TIDY_VERSION and cppcheck $CPPCHECK_VERSION (pip)
  Clang with libFuzzer for the fuzz targets (Linux)
  A virtual display for make window-check (Linux): ${linux_window_packages[*]}
  shellcheck, shfmt and bats for the scripts, curl for the supply chain check

Without options nothing is changed; the exit status is 1 when a required tool
is missing.

  --install  Install what is missing: apt packages on Debian or Ubuntu (uses
             sudo), CMake and Ninja with pip when they are absent or too old,
             the analysis tools with pip, vcpkg with git into .tools/vcpkg.
             The shell tools are installed on Linux, or everywhere with
             SETUP_ALL_TOOLS=1.
  --dry-run  With --install, print the commands instead of running them
  --help     Show this help
USAGE
}

install=0
while [ "$#" -gt 0 ]; do
  case "$1" in
    --install) install=1 ;;
    --dry-run) DRY_RUN=1 ;;
    --help | -h)
      usage
      exit 0
      ;;
    *) die "unknown argument: $1 (see --help)" ;;
  esac
  shift
done

missing_required=0
missing_optional=0

report() {
  local state="$1" name="$2" detail="$3"
  printf '%-8s %-14s %s\n' "$state" "$name" "$detail"
}

expose() {
  local dir="$1"
  case ":$PATH:" in
    *":$dir:"*) ;;
    *) PATH="$dir:$PATH" ;;
  esac
  if [ -n "${GITHUB_PATH:-}" ] && [ "$DRY_RUN" != "1" ]; then
    printf '%s\n' "$dir" >>"$GITHUB_PATH"
  fi
}

all_tools() {
  [ "$(host_os)" = "linux" ] || [ "${SETUP_ALL_TOOLS:-0}" = "1" ]
}

python_command() {
  local candidate
  for candidate in python3 python; do
    if have "$candidate"; then
      printf '%s' "$candidate"
      return 0
    fi
  done
}

pip_install() {
  local python
  python="$(python_command)"
  if [ -z "$python" ]; then
    if [ "$DRY_RUN" != "1" ]; then
      note "python3 with pip was not found: install these another way: $*"
      return 0
    fi
    python="python3"
  fi
  run "$python" -m pip install --user --disable-pip-version-check --break-system-packages "$@" ||
    run "$python" -m pip install --user --disable-pip-version-check "$@"
  if [ "$DRY_RUN" != "1" ]; then
    expose "$("$python" -c 'import sysconfig; print(sysconfig.get_path("scripts", "%s_user" % ("nt" if __import__("os").name == "nt" else "posix")))')"
  fi
}

compiler_name() {
  local candidate
  if [ "$(host_os)" = "windows" ]; then
    local finder="/c/Program Files (x86)/Microsoft Visual Studio/Installer/vswhere.exe"
    if [ -x "$finder" ]; then
      "$finder" -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property displayName 2>/dev/null | tr -d '\r' | head -n 1
    fi
    return 0
  fi
  for candidate in c++ g++ clang++; do
    if have "$candidate"; then
      "$candidate" --version 2>/dev/null | head -n 1
      return 0
    fi
  done
}

tool_version() {
  local output found
  output="$("$1" --version 2>/dev/null || true)"
  found="$(printf '%s\n' "$output" | sed -n 's/.*version \([0-9][0-9.]*\).*/\1/p' | head -n 1)"
  if [ -z "$found" ]; then
    found="$(printf '%s\n' "$output" | sed -n '1s/[^0-9]*\([0-9][0-9.]*\).*/\1/p')"
  fi
  printf '%s' "$found"
}

vcpkg_commit() {
  [ -d "$VCPKG_DIR/.git" ] || return 0
  git -C "$VCPKG_DIR" rev-parse HEAD 2>/dev/null || true
}

vcpkg_program() {
  printf '%s/vcpkg%s' "$VCPKG_DIR" "$(exe_suffix)"
}

install_system_packages() {
  [ "$(host_os)" = "linux" ] || return 0
  if [ "$DRY_RUN" != "1" ] && have c++ && gui_ready && have xvfb-run; then
    return 0
  fi
  if ! have apt-get && [ "$DRY_RUN" != "1" ]; then
    note "apt-get was not found: install the equivalents of: ${linux_packages[*]} ${linux_window_packages[*]}"
    return 0
  fi
  local sudo_command=()
  if [ "$(id -u)" != "0" ]; then
    sudo_command=(sudo)
  fi
  run "${sudo_command[@]}" apt-get update
  run "${sudo_command[@]}" apt-get install -y --no-install-recommends "${linux_packages[@]}" "${linux_window_packages[@]}"
}

install_build_tools() {
  local wanted=()
  if [ "$DRY_RUN" = "1" ] || ! cmake_ready; then
    wanted+=("cmake==$CMAKE_VERSION")
  fi
  if [ "$(host_os)" != "windows" ] && { [ "$DRY_RUN" = "1" ] || ! have ninja; }; then
    wanted+=("ninja==$NINJA_VERSION")
  fi
  if [ "${#wanted[@]}" -gt 0 ]; then
    pip_install "${wanted[@]}"
  fi
}

install_analysis_tools() {
  local wanted=()
  if [ "$DRY_RUN" = "1" ] || [ "$(tool_version clang-format)" != "$CLANG_FORMAT_VERSION" ]; then
    wanted+=("clang-format==$CLANG_FORMAT_VERSION")
  fi
  if [ "$DRY_RUN" = "1" ] || [ "$(tool_version cppcheck)" != "$CPPCHECK_VERSION" ]; then
    wanted+=("cppcheck==$CPPCHECK_WHEEL_VERSION")
  fi
  if [ "$(host_os)" != "windows" ] && { [ "$DRY_RUN" = "1" ] || [ "$(tool_version clang-tidy)" != "$CLANG_TIDY_VERSION" ]; }; then
    wanted+=("clang-tidy==$CLANG_TIDY_VERSION")
  fi
  if [ "${#wanted[@]}" -gt 0 ]; then
    pip_install "${wanted[@]}"
  fi
}

install_vcpkg() {
  if [ "$DRY_RUN" != "1" ] && [ "$(vcpkg_commit)" = "$VCPKG_COMMIT" ] && [ -x "$(vcpkg_program)" ]; then
    return 0
  fi
  if [ "$DRY_RUN" != "1" ]; then
    have git || die "git is required to fetch vcpkg"
  fi
  run mkdir -p "$VCPKG_DIR"
  if [ "$DRY_RUN" = "1" ] || [ ! -d "$VCPKG_DIR/.git" ]; then
    run git -C "$VCPKG_DIR" init --quiet
  fi
  run git -C "$VCPKG_DIR" fetch --quiet --depth 1 "$VCPKG_REPOSITORY_URL" "$VCPKG_COMMIT"
  run git -C "$VCPKG_DIR" -c advice.detachedHead=false checkout --quiet --force FETCH_HEAD
  if [ "$(host_os)" = "windows" ]; then
    if [ "$DRY_RUN" = "1" ]; then
      run cmd.exe //c "bootstrap-vcpkg.bat -disableMetrics"
    else
      (cd "$VCPKG_DIR" && cmd.exe //c "bootstrap-vcpkg.bat -disableMetrics")
    fi
  else
    run "$VCPKG_DIR/bootstrap-vcpkg.sh" -disableMetrics
  fi
}

install_shell_tools() {
  if ! have shellcheck || ! have shfmt || [ "$DRY_RUN" = "1" ]; then
    pip_install "$shellcheck_package" "$shfmt_package"
  fi
  if ! have bats || [ "$DRY_RUN" = "1" ]; then
    if have npm || [ "$DRY_RUN" = "1" ]; then
      local prefix="$HOME/.local/npm"
      run npm install --silent --no-fund --no-audit --prefix "$prefix" "$bats_package"
      if [ "$DRY_RUN" != "1" ]; then
        expose "$prefix/node_modules/.bin"
      fi
    else
      note "npm was not found: install bats another way"
    fi
  fi
}

if [ "$install" = "1" ]; then
  install_system_packages
  install_build_tools
  install_analysis_tools
  install_vcpkg
  if all_tools; then
    install_shell_tools
  fi
  if [ "$DRY_RUN" = "1" ]; then
    exit 0
  fi
fi

cd "$PROJECT_ROOT"
compiler="$(compiler_name)"
if [ -n "$compiler" ]; then
  report "ok" "compiler" "$compiler"
else
  report "missing" "compiler" "install a C++20 compiler (see --help)"
  missing_required=1
fi

if cmake_ready; then
  report "ok" "cmake" "$(tool_version cmake)"
elif have cmake; then
  report "old" "cmake" "$(tool_version cmake) found, $CMAKE_MINIMUM_VERSION or newer is required: run scripts/dev-setup.sh --install"
  missing_required=1
else
  report "missing" "cmake" "run scripts/dev-setup.sh --install"
  missing_required=1
fi

if [ "$(host_os)" != "windows" ]; then
  if have ninja; then
    report "ok" "ninja" "$(ninja --version)"
  else
    report "missing" "ninja" "the default generator is used; run scripts/dev-setup.sh --install for Ninja"
    missing_optional=1
  fi
fi

if [ "$(vcpkg_commit)" = "$VCPKG_COMMIT" ] && [ -x "$(vcpkg_program)" ]; then
  report "ok" "vcpkg" "$VCPKG_DIR at $VCPKG_COMMIT"
elif [ -n "$(vcpkg_root)" ]; then
  report "differs" "vcpkg" "$(vcpkg_root) is used; CI uses commit $VCPKG_COMMIT in .tools/vcpkg"
  missing_optional=1
else
  report "missing" "vcpkg" "run scripts/dev-setup.sh --install, or provide the libraries of vcpkg.json through CMAKE_PREFIX_PATH"
  missing_optional=1
fi

if gui_ready; then
  report "ok" "dashboard" "the window system headers were found"
else
  report "missing" "dashboard" "the dashboard cannot be built; the build without it works (make build-nogui)"
  missing_optional=1
fi

check_pinned() {
  local tool="$1" pinned="$2" found
  if ! have "$tool"; then
    report "missing" "$tool" "run scripts/dev-setup.sh --install"
    missing_optional=1
    return 0
  fi
  found="$(tool_version "$tool")"
  if [ "$found" = "$pinned" ]; then
    report "ok" "$tool" "$found"
  else
    report "differs" "$tool" "$found found, $pinned is what CI uses"
    missing_optional=1
  fi
}

check_pinned clang-format "$CLANG_FORMAT_VERSION"
check_pinned cppcheck "$CPPCHECK_VERSION"
if [ "$(host_os)" != "windows" ]; then
  check_pinned clang-tidy "$CLANG_TIDY_VERSION"
fi

if [ "$(host_os)" = "linux" ]; then
  if have clang++; then
    report "ok" "clang++" "$(clang++ --version | head -n 1)"
  else
    report "missing" "clang++" "needed only for make fuzz"
    missing_optional=1
  fi
  if have xvfb-run; then
    report "ok" "xvfb-run" "$(command -v xvfb-run)"
  else
    report "missing" "xvfb-run" "needed only for make window-check"
    missing_optional=1
  fi
fi

for tool in shellcheck shfmt bats curl; do
  if have "$tool"; then
    report "ok" "$tool" "$(command -v "$tool")"
  else
    report "missing" "$tool" "run scripts/dev-setup.sh --install"
    missing_optional=1
  fi
done

if [ "$missing_required" = "1" ]; then
  die "a required prerequisite is missing"
fi
if [ "$missing_optional" = "1" ]; then
  note "Some optional tools are missing; scripts/check.sh reports which checks it skips."
fi
