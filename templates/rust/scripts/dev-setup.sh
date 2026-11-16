#!/usr/bin/env bash
set -euo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

linux_packages=(gcc pkg-config libgl1-mesa-dev xorg-dev libxkbcommon-dev libwayland-dev libegl1-mesa-dev xz-utils)
shellcheck_package="shellcheck-py==0.11.0.1"
shfmt_package="shfmt-py==4.2.0"
bats_package="bats@1.13.0"

usage() {
  cat <<USAGE
Usage: scripts/dev-setup.sh [--install] [--dry-run]

Checks the prerequisites for building $APP_NAME and reports what is missing.

  Go $GO_VERSION with cgo and a C compiler
  Dashboard build: OpenGL and windowing headers
    Linux (Debian, Ubuntu): ${linux_packages[*]}
    macOS: Xcode command line tools
    Windows: a MinGW-w64 gcc on PATH (for example from MSYS2)
  staticcheck $STATICCHECK_VERSION, govulncheck $GOVULNCHECK_VERSION, fyne $FYNE_TOOLS_VERSION (go install)
  shellcheck, shfmt and bats for the scripts

Without options nothing is changed; the exit status is 1 when a required tool
for the no-GUI build is missing.

  --install  Install what is missing: apt packages on Debian or Ubuntu (uses sudo),
             Go tools with go install, shell tools with pip and npm
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

have() {
  command -v "$1" >/dev/null 2>&1
}

go_bin_dir() {
  local dir
  dir="$(go env GOBIN)"
  if [ -z "$dir" ]; then
    dir="$(go env GOPATH)/bin"
  fi
  printf '%s' "$dir"
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

install_system_packages() {
  [ "$(host_os)" = "linux" ] || return 0
  if gui_ready && have xz; then
    return 0
  fi
  if ! have apt-get; then
    note "apt-get was not found: install the equivalents of: ${linux_packages[*]}"
    return 0
  fi
  local sudo_command=()
  if [ "$(id -u)" != "0" ]; then
    sudo_command=(sudo)
  fi
  run "${sudo_command[@]}" apt-get update
  run "${sudo_command[@]}" apt-get install -y --no-install-recommends "${linux_packages[@]}"
}

install_go_tool() {
  local name="$1" package="$2"
  if have "$name"; then
    return 0
  fi
  run go install "$package"
}

install_shell_tools() {
  local python=""
  for candidate in python3 python; do
    if have "$candidate"; then
      python="$candidate"
      break
    fi
  done
  if ! have shellcheck || ! have shfmt; then
    if [ -n "$python" ]; then
      run "$python" -m pip install --user --disable-pip-version-check --break-system-packages "$shellcheck_package" "$shfmt_package" ||
        run "$python" -m pip install --user --disable-pip-version-check "$shellcheck_package" "$shfmt_package"
      if [ "$DRY_RUN" != "1" ]; then
        expose "$("$python" -c 'import sysconfig; print(sysconfig.get_path("scripts", "%s_user" % ("nt" if __import__("os").name == "nt" else "posix")))')"
      fi
    else
      note "python3 with pip was not found: install shellcheck and shfmt another way"
    fi
  fi
  if ! have bats; then
    if have npm; then
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
  have go || die "Go $GO_VERSION is not installed: get it from the Go download page, then run this again"
  install_system_packages
  expose "$(go_bin_dir)"
  install_go_tool staticcheck "honnef.co/go/tools/cmd/staticcheck@$STATICCHECK_VERSION"
  install_go_tool govulncheck "golang.org/x/vuln/cmd/govulncheck@$GOVULNCHECK_VERSION"
  install_go_tool fyne "fyne.io/tools/cmd/fyne@$FYNE_TOOLS_VERSION"
  if [ "$(host_os)" = "linux" ] || [ "${SETUP_SHELL_TOOLS:-0}" = "1" ]; then
    install_shell_tools
  fi
  if [ "$DRY_RUN" = "1" ]; then
    exit 0
  fi
fi

if have go; then
  found="$(go env GOVERSION)"
  if [ "$found" = "go$GO_VERSION" ]; then
    report "ok" "go" "$found"
  else
    report "differs" "go" "$found found, go$GO_VERSION is what CI uses"
    missing_optional=1
  fi
  if [ "$(go env CGO_ENABLED)" = "1" ] && have "$(go env CC)"; then
    report "ok" "cgo" "C compiler $(go env CC)"
  else
    report "missing" "cgo" "a C compiler is required (SQLite is compiled in)"
    missing_required=1
  fi
else
  report "missing" "go" "install Go $GO_VERSION"
  missing_required=1
fi

if gui_ready; then
  report "ok" "dashboard" "windowing and OpenGL headers found"
else
  report "missing" "dashboard" "headers for the dashboard build are missing; the no-GUI build still works"
  missing_optional=1
fi

for tool in staticcheck govulncheck fyne shellcheck shfmt bats; do
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
