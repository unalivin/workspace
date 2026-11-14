#!/usr/bin/env bash
set -euo pipefail

shellcheck_package="shellcheck-py==0.11.0.1"
shfmt_package="shfmt-py==4.2.0"
bats_package="bats@1.13.0"

usage() {
  cat <<'USAGE'
Usage: scripts/install-shell-tools.sh [--dry-run] [--check]

Installs the pinned shell tooling used by the checks:

  shellcheck 0.11.0   through the shellcheck-py wheel (pip)
  shfmt 3.14.1        through the shfmt-py wheel (pip)
  bats 1.13.0         through npm

pip and npm are used only as download channels for prebuilt developer tools.

  --check    Report which tools are present and exit with status 1 if one is missing
  --dry-run  Print the install commands without running them
  --help     Show this help
USAGE
}

dry_run=0
check=0
while [ "$#" -gt 0 ]; do
  case "$1" in
    --dry-run) dry_run=1 ;;
    --check) check=1 ;;
    --help | -h)
      usage
      exit 0
      ;;
    *)
      printf 'error: unknown argument: %s (see --help)\n' "$1" >&2
      exit 1
      ;;
  esac
  shift
done

if [ "$check" = "1" ]; then
  missing=0
  for tool in shellcheck shfmt bats; do
    if command -v "$tool" >/dev/null 2>&1; then
      printf 'found   %s\n' "$tool"
    else
      printf 'missing %s\n' "$tool"
      missing=1
    fi
  done
  exit "$missing"
fi

run() {
  if [ "$dry_run" = "1" ]; then
    printf 'dry-run:'
    printf ' %q' "$@"
    printf '\n'
    return 0
  fi
  "$@"
}

python_command=""
for candidate in python3 python; do
  if command -v "$candidate" >/dev/null 2>&1; then
    python_command="$candidate"
    break
  fi
done
[ -n "$python_command" ] || {
  printf 'error: python3 with pip is needed to download shellcheck and shfmt\n' >&2
  exit 1
}
command -v npm >/dev/null 2>&1 || {
  printf 'error: npm is needed to download bats\n' >&2
  exit 1
}

pip_flags=(--user --quiet)
if "$python_command" -m pip install --help 2>/dev/null | grep -q -- '--break-system-packages'; then
  pip_flags+=(--break-system-packages)
fi
run "$python_command" -m pip install "${pip_flags[@]}" "$shellcheck_package" "$shfmt_package"

prefix="${SHELL_TOOLS_PREFIX:-$HOME/.local}"
run npm install --global --silent --prefix "$prefix" "$bats_package"

user_bin="$("$python_command" -c 'import os, sysconfig; print(sysconfig.get_path("scripts", os.name + "_user"))')"
for directory in "$user_bin" "$prefix/bin" "$prefix"; do
  if [ -n "${GITHUB_PATH:-}" ] && [ "$dry_run" = "0" ]; then
    printf '%s\n' "$directory" >>"$GITHUB_PATH"
  fi
done
printf 'installed shell tools; make sure these directories are on PATH:\n  %s\n  %s\n' "$user_bin" "$prefix/bin"
