#!/usr/bin/env bash
set -euo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

usage() {
  cat <<'USAGE'
Usage: scripts/split-project.sh [--dry-run] [--output <directory>] <project>

Extracts one project with its history into a standalone repository using
git subtree split. The standalone repository gets its own .github/workflows
generated from templates/workflows (plain v* release tags), and its
repository links point at <owner>/<project>. The monorepo working tree,
branches and history are left untouched.

  --output   Directory for the standalone repository
             (default: ../<project>-standalone next to the workspace)
  --dry-run  Print what would happen without creating anything
  --help     Show this help
USAGE
}

output=""
project=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    --dry-run) DRY_RUN=1 ;;
    --output)
      [ "$#" -ge 2 ] || die "--output needs a directory"
      output="$2"
      shift
      ;;
    --help | -h)
      usage
      exit 0
      ;;
    -*) die "unknown option: $1 (see --help)" ;;
    *)
      [ -z "$project" ] || die "only one project can be split at a time"
      project="$1"
      ;;
  esac
  shift
done
[ -n "$project" ] || die "expected a project name (see --help)"
registry_has "$project" || die "unknown project: $project"
[ -d "$WORKSPACE_ROOT/$project" ] || die "project folder does not exist: $project"
if [ -z "$output" ]; then
  output="$(dirname "$WORKSPACE_ROOT")/$project-standalone"
fi
[ ! -e "$output" ] || die "output already exists: $output"
if [ -z "$(git -C "$WORKSPACE_ROOT" ls-files -- "$project" | head -n 1)" ]; then
  die "$project has no committed files"
fi
if [ -n "$(git -C "$WORKSPACE_ROOT" status --porcelain -- "$project")" ]; then
  die "$project has uncommitted changes; commit them before splitting"
fi

monorepo_url="https://github.com/$WORKSPACE_OWNER/$WORKSPACE_REPO"
standalone_url="https://github.com/$WORKSPACE_OWNER/$project"

if [ "$DRY_RUN" = "1" ]; then
  note "dry-run: would split $project with git subtree split into $output"
  note "dry-run: would generate ci.yml and release.yml in $output/.github/workflows"
  note "dry-run: would rewrite $monorepo_url to $standalone_url"
  note "dry-run: would rewrite tag references $project/v* to v*"
  exit 0
fi

temporary_branch="split-$project-$$"
cleanup() {
  git -C "$WORKSPACE_ROOT" branch -D "$temporary_branch" >/dev/null 2>&1 || true
}
trap cleanup EXIT

git -C "$WORKSPACE_ROOT" subtree split --prefix="$project" -b "$temporary_branch" >/dev/null
git clone --quiet --no-hardlinks --single-branch --branch "$temporary_branch" "$WORKSPACE_ROOT" "$output"
git -C "$output" checkout --quiet -b main
git -C "$output" branch --quiet -D "$temporary_branch"
git -C "$output" remote remove origin
git -C "$output" config user.name "$(git -C "$WORKSPACE_ROOT" config user.name)"
git -C "$output" config user.email "$(git -C "$WORKSPACE_ROOT" config user.email)"

"$WORKSPACE_ROOT/scripts/gen-workflows.sh" --standalone "$project" --output "$output/.github/workflows" >/dev/null

while IFS= read -r -d '' file; do
  if LC_ALL=C grep -Iq . "$file"; then
    sed -i.bak \
      -e "s|$monorepo_url|$standalone_url|g" \
      -e "s|$project/v\([0-9]\)|v\1|g" \
      "$file"
    rm -f "$file.bak"
  fi
done < <(git -C "$output" ls-files -z | while IFS= read -r -d '' tracked; do printf '%s\0' "$output/$tracked"; done)

git -C "$output" add -A
git -C "$output" commit --quiet -m "ci: restore standalone workflows and repository links"

note "standalone repository created at $output"
note "release tags in the standalone repository are plain v*, for example v0.1.0-dev"
