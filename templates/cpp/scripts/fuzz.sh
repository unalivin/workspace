#!/usr/bin/env bash
set -euo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

usage() {
  cat <<'USAGE'
Usage: scripts/fuzz.sh [--time <duration>] [--list] [--dry-run]

Runs every fuzz target in fuzz/targets, one after the other, for the given
duration each (default 5s) with libFuzzer, AddressSanitizer and
UndefinedBehaviorSanitizer (preset fuzz). libFuzzer needs Clang and runs on
Linux. Each target starts from its seed inputs in fuzz/seeds/<target>; inputs
that libFuzzer finds are kept in fuzz/corpus/<target>, and an input that fails
is written to fuzz/artifacts/.

On every system "make test" replays the seed inputs and a few thousand
generated inputs through the same targets without libFuzzer.

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
while IFS= read -r name; do
  targets+=("$name")
done < <(fuzz_targets)
[ "${#targets[@]}" -gt 0 ] || die "no fuzz targets were found"

if [ "$list_only" = "1" ]; then
  printf '%s\n' "${targets[@]}"
  exit 0
fi

if [ "$DRY_RUN" != "1" ]; then
  [ "$(host_os)" = "linux" ] || die "libFuzzer runs on Linux: make test replays the seed inputs on this system"
  have clang++ || die "clang++ is not installed: run scripts/dev-setup.sh --install"
  build_preset fuzz fuzzers
fi
for name in "${targets[@]}"; do
  run mkdir -p "fuzz/corpus/$name" fuzz/artifacts
  run "$(built_program fuzz "fuzz_$name")" "-max_total_time=$seconds" "-artifact_prefix=fuzz/artifacts/$name-" \
    "fuzz/corpus/$name" "fuzz/seeds/$name"
done
