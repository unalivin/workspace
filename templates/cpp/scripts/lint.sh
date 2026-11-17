#!/usr/bin/env bash
set -euo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

usage() {
  cat <<USAGE
Usage: scripts/lint.sh [--gui] [--dry-run]

Static analysis with warnings as errors.

  without --gui  cppcheck $CPPCHECK_VERSION on every C++ file, then clang-tidy
                 $CLANG_TIDY_VERSION on the files of the build without the dashboard
                 (preset nogui)
  --gui          clang-tidy on the files that only the dashboard build has
                 (preset dev)

clang-tidy reads the compile commands that the Ninja and Makefile generators
write, so it runs on Linux and macOS. On Windows the compiler warnings of the
build (/W4 /WX) and cppcheck are the checks. Set LINT_TIDY=0 to leave
clang-tidy out.

  --gui      Analyze the dashboard files
  --dry-run  Print the commands instead of running them
  --help     Show this help
USAGE
}

gui=0
while [ "$#" -gt 0 ]; do
  case "$1" in
    --gui) gui=1 ;;
    --dry-run) DRY_RUN=1 ;;
    --help | -h)
      usage
      exit 0
      ;;
    *) die "unknown argument: $1 (see --help)" ;;
  esac
  shift
done

cd "$PROJECT_ROOT"
gui_pattern='^(src/gui/|tests/gui/|tests/guikit/|tools/screenshots\.cpp$)'

run_cppcheck() {
  local directories
  read -r -a directories <<<"$SOURCE_DIRECTORIES"
  if [ "$DRY_RUN" != "1" ]; then
    have cppcheck || die "cppcheck is not installed: run scripts/dev-setup.sh --install"
  fi
  run cppcheck --enable=warning,performance,portability --std=c++20 --language=c++ -I src -I tools -I tests -I fuzz \
    --suppress=missingIncludeSystem --suppress=missingInclude --quiet -j "$(job_count)" --error-exitcode=1 \
    "--template={file}:{line}: {severity}: {message} [{id}]" "${directories[@]}"
}

tidy_files() {
  if [ "$gui" = "1" ]; then
    source_files | grep '\.cpp$' | grep -E "$gui_pattern" || true
  else
    source_files | grep '\.cpp$' | grep -Ev "$gui_pattern" || true
  fi
}

run_tidy() {
  local preset="$1" directory
  directory="$(preset_dir "$preset")"
  if [ "${LINT_TIDY:-1}" = "0" ]; then
    note "clang-tidy was left out because LINT_TIDY=0"
    return 0
  fi
  if [ "$(host_os)" = "windows" ]; then
    note "clang-tidy does not run on Windows: the Visual Studio generator writes no compile commands"
    return 0
  fi
  if [ "$DRY_RUN" != "1" ]; then
    have clang-tidy || die "clang-tidy is not installed: run scripts/dev-setup.sh --install"
  fi
  build_preset "$preset"
  if [ "$DRY_RUN" = "1" ]; then
    note "dry-run: clang-tidy -p $directory --quiet <files>"
    return 0
  fi
  [ -f "$directory/compile_commands.json" ] || die "$directory/compile_commands.json is missing: use the Ninja or Makefile generator"
  local arguments=(-p "$directory" --quiet) sdk=""
  if [ "$(host_os)" = "macos" ] && have xcrun; then
    sdk="$(xcrun --show-sdk-path 2>/dev/null || true)"
    if [ -n "$sdk" ]; then
      arguments+=("--extra-arg=-isysroot" "--extra-arg=$sdk")
    fi
  fi
  tidy_files | xargs -P "$(job_count)" -n 4 clang-tidy "${arguments[@]}"
}

if [ "$gui" = "1" ]; then
  run_tidy dev
else
  run_cppcheck
  run_tidy nogui
fi
