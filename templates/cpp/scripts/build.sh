#!/usr/bin/env bash
set -euo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

usage() {
  cat <<'USAGE'
Usage: scripts/build.sh <preset> [--target <name>]... [--test] [--copy <program> <file>]
                        [--dry-run] [--run <program> [arguments]]

Configures the CMake preset into build/<preset>, builds it and optionally runs
the tests or one of the built programs. The presets are in CMakePresets.json:

  dev            Debug, dashboard and tests
  nogui          Debug, no dashboard, tests
  release        Release, dashboard
  release-nogui  Release, no dashboard
  asan           tests under AddressSanitizer and UndefinedBehaviorSanitizer
  tsan           tests under ThreadSanitizer
  checked        optimized tests with assertions and checked containers
  fuzz           libFuzzer targets, built with Clang

The dependencies come from vcpkg when scripts/dev-setup.sh --install has put it
into .tools/vcpkg (or VCPKG_ROOT names a checkout). Without vcpkg CMake looks
for installed packages, for example under CMAKE_PREFIX_PATH.

  --target <name>      Build only this target (may be repeated)
  --test               Run the tests of the preset with ctest after the build
  --copy <program> <file>
                       Copy a built program to a file
  --run <program> ...  Run a built program with the remaining arguments; this
                       must be the last option
  --dry-run            Print the commands instead of running them
  --help               Show this help
USAGE
}

preset=""
targets=()
run_tests=0
copy_program=""
copy_target=""
program=""
arguments=()
while [ "$#" -gt 0 ]; do
  case "$1" in
    --target)
      [ "$#" -ge 2 ] || die "--target needs a name"
      targets+=("$2")
      shift
      ;;
    --test) run_tests=1 ;;
    --copy)
      [ "$#" -ge 3 ] || die "--copy needs a program and a file"
      copy_program="$2"
      copy_target="$3"
      shift 2
      ;;
    --run)
      [ "$#" -ge 2 ] || die "--run needs a program"
      program="$2"
      shift 2
      arguments=("$@")
      break
      ;;
    --dry-run) DRY_RUN=1 ;;
    --help | -h)
      usage
      exit 0
      ;;
    -*) die "unknown argument: $1 (see --help)" ;;
    *)
      [ -z "$preset" ] || die "unknown argument: $1 (see --help)"
      preset="$1"
      ;;
  esac
  shift
done
[ -n "$preset" ] || die "a preset is required (see --help)"
preset_config "$preset" >/dev/null || die "unknown build preset: $preset (see --help)"
if [ "$DRY_RUN" != "1" ]; then
  cmake_ready || die "CMake $CMAKE_MINIMUM_VERSION or newer is required: run scripts/dev-setup.sh --install"
fi

cd "$PROJECT_ROOT"
build_preset "$preset" ${targets[@]+"${targets[@]}"}
if [ "$run_tests" = "1" ]; then
  test_preset "$preset"
fi
if [ -n "$copy_program" ]; then
  run mkdir -p "$(dirname "$copy_target")"
  run cp "$(built_program "$preset" "$copy_program")" "$copy_target"
fi
if [ -n "$program" ]; then
  run "$(built_program "$preset" "$program")" ${arguments[@]+"${arguments[@]}"}
fi
