#!/usr/bin/env bash
set -euo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

usage() {
  cat <<'USAGE'
Usage: scripts/split-project.sh [--dry-run] [--owner <owner>] [--output <directory>] <project>

Extracts one project with its history into a standalone repository using
git subtree split. The monorepo working tree, branches and history are left
untouched. In the standalone repository:

  - .github/workflows is generated from templates/workflows, with plain v*
    release tags
  - repository links, the module path and the application identifier point
    at <owner>/<project>
  - the copyright holder from the registry is set and then verified in the
    license, the README, the notices, the publisher shown by the application
    and the package metadata
  - the project's own .github/PULL_REQUEST_TEMPLATE.md is the pull request
    template

The owner comes from the destination in scripts/projects.tsv: Sonix-Devs for
the company projects, the personal profile for the others.

  --owner    GitHub owner of the standalone repository
             (default: the owner of the project's destination)
  --output   Directory for the standalone repository
             (default: ../<project>-standalone next to the workspace)
  --dry-run  Print what would happen without creating anything
  --help     Show this help
USAGE
}

output=""
project=""
owner=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    --dry-run) DRY_RUN=1 ;;
    --output)
      [ "$#" -ge 2 ] || die "--output needs a directory"
      output="$2"
      shift
      ;;
    --owner)
      [ "$#" -ge 2 ] || die "--owner needs a GitHub owner"
      owner="$2"
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

destination="$(project_destination "$project")"
valid_destination "$destination" || die "$project has no valid destination in the registry"
holder="$(project_copyright_holder "$project")"
[ "$holder" = "$(destination_holder "$destination")" ] || die "$project goes to $destination, but its copyright holder in the registry is \"$holder\""
if [ -z "$owner" ]; then
  owner="$(destination_owner "$destination")"
fi
[[ "$owner" =~ ^[A-Za-z0-9][A-Za-z0-9-]*$ ]] || die "invalid owner: $owner"

monorepo_url="https://github.com/$WORKSPACE_OWNER/$WORKSPACE_REPO"
standalone_url="https://github.com/$owner/$project"
identifier="io.github.$(lower_case "$owner").$project"

if [ "$DRY_RUN" = "1" ]; then
  note "dry-run: would split $project with git subtree split into $output"
  note "dry-run: destination: $(destination_label "$destination")"
  note "dry-run: would generate ci.yml and release.yml in $output/.github/workflows"
  note "dry-run: would rewrite $monorepo_url to $standalone_url"
  note "dry-run: would set the module path to github.com/$owner/$project and the application identifier to $identifier"
  note "dry-run: would set and verify the copyright holder $holder"
  note "dry-run: would keep $project/.github/PULL_REQUEST_TEMPLATE.md as the pull request template"
  note "dry-run: would rewrite tag references $project/v* to v*"
  exit 0
fi

[ -f "$WORKSPACE_ROOT/$project/.github/PULL_REQUEST_TEMPLATE.md" ] ||
  die "$project has no .github/PULL_REQUEST_TEMPLATE.md; run scripts/gen-workflows.sh and commit it"

temporary_branch="split-$project-$$"
rewrite="$(mktemp)"
finished=0
cleanup() {
  git -C "$WORKSPACE_ROOT" branch -D "$temporary_branch" >/dev/null 2>&1 || true
  rm -f "$rewrite"
  if [ "$finished" = "0" ]; then
    rm -rf "$output"
  fi
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

{
  printf 's|%s|%s|g\n' "$(escape_pattern "$monorepo_url")" "$(escape_replacement "$standalone_url")"
  for known in "$WORKSPACE_OWNER" "$COMPANY_OWNER"; do
    printf 's|github\\.com/%s/%s|github.com/%s/%s|g\n' "$(escape_pattern "$known")" "$project" "$(escape_replacement "$owner")" "$project"
    printf 's|io\\.github\\.%s\\.%s|%s|g\n' "$(escape_pattern "$(lower_case "$known")")" "$project" "$(escape_replacement "$identifier")"
  done
  printf 's|%s|%s|g\n' "$(escape_pattern "$(other_holder "$holder")")" "$(escape_replacement "$holder")"
  printf 's|%s/v\\([0-9]\\)|v\\1|g\n' "$project"
} >"$rewrite"

while IFS= read -r -d '' file; do
  if is_text_file "$file"; then
    apply_token_script "$rewrite" "$file"
  fi
done < <(git -C "$output" ls-files -z | while IFS= read -r -d '' tracked; do printf '%s\0' "$output/$tracked"; done)

problems="$(holder_problems "$output" "$project" "$holder")"
if [ -n "$problems" ]; then
  printf '%s\n' "$problems" >&2
  die "the standalone repository does not name the copyright holder $holder everywhere; fix $project in the workspace first"
fi
[ -f "$output/.github/PULL_REQUEST_TEMPLATE.md" ] || die "the standalone repository has no pull request template"

git -C "$output" add -A
git -C "$output" commit --quiet -m "ci: restore standalone workflows and repository links"
finished=1

note "standalone repository created at $output"
note "repository: $standalone_url, copyright holder: $holder"
note "release tags in the standalone repository are plain v*, for example v0.1.0-dev"
