#!/usr/bin/env bash
set -euo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

usage() {
  cat <<'USAGE'
Usage: scripts/release-verify.sh --tag <tag> --commit <sha> --directory <dir>
                                 [--notes-output <file>] [--skip-remote]

Checks that a pre-release may be created. The only file it writes is the one
named by --notes-output:

  1. the tag is <project>/v<version> (or v<version> in a standalone
     repository) and the version equals the application version
  2. the tag points at the given commit
  3. no GitHub release with this tag exists yet
  4. the directory holds exactly the three release files, each non-empty
  5. docs/RELEASE_NOTES.md exists

  --tag          Tag that triggered the release
  --commit       Commit the workflow runs on
  --directory    Directory with the downloaded build results
  --notes-output File that receives docs/RELEASE_NOTES.md with every link to
                 the default branch (/blob/HEAD/, /tree/HEAD/) pointed at the
                 released commit, so the links match the release
  --skip-remote  Do not ask GitHub whether the release exists (for tests)
  --help         Show this help
USAGE
}

tag=""
commit=""
directory=""
notes_output=""
skip_remote=0
while [ "$#" -gt 0 ]; do
  case "$1" in
    --tag | --commit | --directory | --notes-output)
      [ "$#" -ge 2 ] || die "$1 needs a value"
      case "$1" in
        --tag) tag="$2" ;;
        --commit) commit="$2" ;;
        --directory) directory="$2" ;;
        --notes-output) notes_output="$2" ;;
      esac
      shift
      ;;
    --skip-remote) skip_remote=1 ;;
    --help | -h)
      usage
      exit 0
      ;;
    *) die "unknown argument: $1 (see --help)" ;;
  esac
  shift
done
[ -n "$tag" ] || die "--tag is required"
[ -n "$commit" ] || die "--commit is required"
[ -n "$directory" ] || die "--directory is required"

case "$tag" in
  "$APP_SLUG/v$APP_VERSION" | "v$APP_VERSION") ;;
  "$APP_SLUG"/v* | v*) die "tag $tag does not match the application version $APP_VERSION" ;;
  *) die "tag $tag is not a release tag of $APP_SLUG" ;;
esac

tag_commit="$(git -C "$PROJECT_ROOT" rev-parse --verify --quiet "refs/tags/$tag^{commit}")" || die "tag $tag does not exist in this checkout"
workflow_commit="$(git -C "$PROJECT_ROOT" rev-parse --verify --quiet "$commit^{commit}")" || die "commit $commit does not exist in this checkout"
[ "$tag_commit" = "$workflow_commit" ] || die "tag $tag points at $tag_commit, not at the workflow commit $workflow_commit"

if [ "$skip_remote" != "1" ]; then
  command -v gh >/dev/null 2>&1 || die "gh is not installed"
  if gh release view "$tag" >/dev/null 2>&1; then
    die "a release for $tag already exists; releases are never overwritten"
  fi
fi

[ -d "$directory" ] || die "directory not found: $directory"
expected="$(printf '%s\n' "$APP_SLUG-linux-x64.tar.gz" "$APP_SLUG-macos-arm64.tar.gz" "$APP_SLUG-windows-x64.exe" | LC_ALL=C sort)"
found="$(cd "$directory" && find . -type f | sed 's|^\./||' | LC_ALL=C sort)"
if [ "$found" != "$expected" ]; then
  printf 'expected files:\n%s\nfound files:\n%s\n' "$expected" "$found" >&2
  die "the release must contain exactly the three release files"
fi
while IFS= read -r file; do
  [ -s "$directory/$file" ] || die "release file is empty: $file"
done <<<"$expected"

[ -s "$PROJECT_ROOT/docs/RELEASE_NOTES.md" ] || die "docs/RELEASE_NOTES.md is missing or empty"

note "release checks passed for $tag at $workflow_commit"

if [ -n "$notes_output" ]; then
  sed -e "s|/blob/HEAD/|/blob/$workflow_commit/|g" -e "s|/tree/HEAD/|/tree/$workflow_commit/|g" \
    "$PROJECT_ROOT/docs/RELEASE_NOTES.md" >"$notes_output"
  note "release notes written to $notes_output, links point at $workflow_commit"
fi
