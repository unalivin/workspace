#!/usr/bin/env bash
set -euo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

usage() {
  cat <<USAGE
Usage: scripts/fuzz.sh [--time <duration>] [--list] [--dry-run]

Runs every fuzz target in fuzz/fuzz_targets, one after the other, for the given
duration each (default 5s) with cargo fuzz $CARGO_FUZZ_VERSION. The pinned stable
compiler ($RUST_VERSION) is used with RUSTC_BOOTSTRAP=1, so no nightly toolchain
is needed. libFuzzer runs on Linux and macOS.

  --time     Duration per target, for example 5s or 2m
  --list     Print the targets and exit
  --dry-run  Print the commands instead of running them
  --help     Show this help
USAGE
}

duration="5s"
list_only=0
while [ "$#" -gt 0 ]; do
  case "$1" in
    --time)
      [ "$#" -ge 2 ] || die "--time needs a duration"
      duration="$2"
      shift
      ;;
    --list) list_only=1 ;;
    --dry-run) DRY_RUN=1 ;;
    --help | -h)
      usage
      exit 0
      ;;
    *) die "unknown argument: $1 (see --help)" ;;
  esac
  shift
done

case "$duration" in
  *[!0-9sm]* | "" | [sm]*) die "--time must look like 5s or 2m" ;;
  *m) seconds=$((${duration%m} * 60)) ;;
  *s) seconds="${duration%s}" ;;
  *) seconds="$duration" ;;
esac
case "$seconds" in
  *[!0-9]* | "") die "--time must look like 5s or 2m" ;;
esac

cd "$PROJECT_ROOT"
targets=()
for file in fuzz/fuzz_targets/*.rs; do
  [ -f "$file" ] || continue
  name="$(basename "$file" .rs)"
  targets+=("$name")
done
[ "${#targets[@]}" -gt 0 ] || die "no fuzz targets were found"

if [ "$list_only" = "1" ]; then
  printf '%s\n' "${targets[@]}"
  exit 0
fi

if [ "$DRY_RUN" != "1" ]; then
  [ "$(host_os)" != "windows" ] || die "the fuzz targets run on Linux and macOS"
  have_cargo_tool fuzz || die "cargo fuzz is not installed: run scripts/dev-setup.sh --install"
fi
for name in "${targets[@]}"; do
  run env RUSTC_BOOTSTRAP=1 cargo fuzz run "$name" -- "-max_total_time=$seconds"
done
