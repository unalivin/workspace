#!/usr/bin/env bash
set -euo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

usage() {
  cat <<USAGE
Usage: scripts/format.sh [--write] [--dry-run]

Checks that every C++ file under ${SOURCE_DIRECTORIES// /, } follows
.clang-format. clang-format $CLANG_FORMAT_VERSION is the version CI uses; another
version can format a few lines differently.

  --write    Rewrite the files instead of reporting the differences
  --dry-run  Print the command instead of running it
  --help     Show this help
USAGE
}

write=0
while [ "$#" -gt 0 ]; do
  case "$1" in
    --write) write=1 ;;
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
if [ "$DRY_RUN" != "1" ]; then
  have clang-format || die "clang-format is not installed: run scripts/dev-setup.sh --install"
fi
files=()
while IFS= read -r file; do
  files+=("$file")
done < <(source_files)
[ "${#files[@]}" -gt 0 ] || die "no C++ files were found"

if [ "$write" = "1" ]; then
  run clang-format -i "${files[@]}"
else
  run clang-format --dry-run --Werror "${files[@]}"
fi
