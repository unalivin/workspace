#!/usr/bin/env bash
set -euo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

linux_packages=(gcc libc6-dev)
shellcheck_package="shellcheck-py==0.11.0.1"
shfmt_package="shfmt-py==4.2.0"
bats_package="bats@1.13.0"

usage() {
  cat <<USAGE
Usage: scripts/dev-setup.sh [--install] [--dry-run]

Checks the prerequisites for building $APP_NAME and reports what is missing.

  Rust $RUST_VERSION with rustfmt and clippy (rust-toolchain.toml pins it, rustup installs it)
  A C compiler and linker (SQLite is compiled in)
    Linux (Debian, Ubuntu): ${linux_packages[*]}
    macOS: Xcode command line tools
    Windows: Build Tools for Visual Studio with the Windows SDK
  cargo audit $CARGO_AUDIT_VERSION, cargo deny $CARGO_DENY_VERSION, cargo fuzz $CARGO_FUZZ_VERSION (cargo install)
  cargo bundle $CARGO_BUNDLE_VERSION for the macOS application bundle (cargo install)
  shellcheck, shfmt and bats for the scripts

Without options nothing is changed; the exit status is 1 when a required tool
is missing.

  --install  Install what is missing: the pinned toolchain with rustup, apt
             packages on Debian or Ubuntu (uses sudo), cargo tools with
             cargo install, shell tools with pip and npm. The supply chain
             and fuzzing tools and the shell tools are installed on Linux,
             or everywhere with SETUP_ALL_TOOLS=1. cargo bundle is installed
             on macOS only, the one place where it is used.
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

install_toolchain() {
  if ! have rustup && [ "$DRY_RUN" != "1" ]; then
    note "rustup was not found: install Rust $RUST_VERSION with rustfmt and clippy another way"
    return 0
  fi
  run rustup toolchain install "$RUST_VERSION" --profile minimal --component rustfmt --component clippy
}

install_system_packages() {
  [ "$(host_os)" = "linux" ] || return 0
  if have cc; then
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

install_cargo_tool() {
  local subcommand="$1" version="$2"
  if [ "$DRY_RUN" != "1" ] && have_cargo_tool "$subcommand"; then
    return 0
  fi
  run cargo install --locked "cargo-$subcommand" --version "$version"
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
  have cargo || [ "$DRY_RUN" = "1" ] || die "Rust is not installed: get rustup from the Rust download page, then run this again"
  install_toolchain
  install_system_packages
  if all_tools; then
    install_cargo_tool audit "$CARGO_AUDIT_VERSION"
    install_cargo_tool deny "$CARGO_DENY_VERSION"
    install_cargo_tool fuzz "$CARGO_FUZZ_VERSION"
    install_shell_tools
  fi
  if [ "$(host_os)" = "macos" ] || [ "$DRY_RUN" = "1" ]; then
    install_cargo_tool bundle "$CARGO_BUNDLE_VERSION"
  fi
  if [ "$DRY_RUN" = "1" ]; then
    exit 0
  fi
fi

cd "$PROJECT_ROOT"
if have cargo && have rustc; then
  found="$(rustc --version | awk '{ print $2 }')"
  if [ "$found" = "$RUST_VERSION" ]; then
    report "ok" "rust" "$found"
  else
    report "differs" "rust" "$found found, $RUST_VERSION is what CI uses"
    missing_optional=1
  fi
  for component in fmt clippy; do
    if have_cargo_tool "$component"; then
      report "ok" "cargo $component" "$(cargo "$component" --version)"
    else
      report "missing" "cargo $component" "run scripts/dev-setup.sh --install"
      missing_optional=1
    fi
  done
else
  report "missing" "rust" "install Rust $RUST_VERSION with rustup"
  missing_required=1
fi

if gui_ready; then
  report "ok" "linker" "a C toolchain for the compiled-in SQLite and for linking was found"
else
  report "missing" "linker" "a C compiler is required (SQLite is compiled in)"
  missing_required=1
fi

for tool in audit deny fuzz; do
  if have_cargo_tool "$tool"; then
    report "ok" "cargo $tool" "$(cargo "$tool" --version 2>/dev/null | head -n 1)"
  else
    report "missing" "cargo $tool" "run scripts/dev-setup.sh --install"
    missing_optional=1
  fi
done

if [ "$(host_os)" = "macos" ]; then
  if have_cargo_tool bundle; then
    report "ok" "cargo bundle" "$(cargo bundle --version 2>/dev/null | head -n 1)"
  else
    report "missing" "cargo bundle" "run scripts/dev-setup.sh --install"
    missing_optional=1
  fi
fi

for tool in shellcheck shfmt bats; do
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
