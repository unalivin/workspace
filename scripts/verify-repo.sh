#!/usr/bin/env bash
set -euo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

usage() {
  cat <<'USAGE'
Usage: scripts/verify-repo.sh [check...]

Runs repository-wide structural checks. Without arguments every check runs.

  registry       The project registry has 25 valid rows, truthful statuses,
                 17 company and 8 personal destinations and the copyright
                 holder that belongs to each destination
  files          The root, every template and every project has every
                 required file
  emails         No email address appears in any tracked or untracked file
  comments       No Go, Rust, C++, SQL or Bash source file contains a comment
  workflows      .github/workflows matches the output of
                 scripts/gen-workflows.sh
  pull-requests  Every project has its own pull request template, the root
                 copies are identical to them and the root default lists them
  readme         The project table in README.md matches the registry
  destinations   The table of project, language and destination in README.md,
                 docs/WORKSPACE.md and docs/ROADMAP.md matches the registry
  holders        The license, the README, the notices, the publisher shown by
                 the application and the package metadata of every project
                 and template name the copyright holder from the registry
  --help         Show this help
USAGE
}

failures=0

fail() {
  printf 'FAIL %s\n' "$*" >&2
  if [ "${GITHUB_ACTIONS:-}" = "true" ]; then
    printf '::error title=verify-repo::%s\n' "$*"
  fi
  failures=$((failures + 1))
}

repo_files() {
  git -C "$WORKSPACE_ROOT" ls-files -co --exclude-standard
}

root_required="README.md LICENSE CONTRIBUTING.md SECURITY.md CODE_OF_CONDUCT.md CHANGELOG.md .editorconfig .gitattributes .gitignore Makefile scripts/list-projects.sh scripts/check-all.sh scripts/gen-workflows.sh scripts/new-project.sh scripts/split-project.sh scripts/publish-github.sh docs/WORKSPACE.md docs/LANGUAGES.md docs/ADD_PROJECT.md docs/RELEASING.md docs/ROADMAP.md .github/PULL_REQUEST_TEMPLATE.md templates/pull-request/root.md"
project_required_files="README.md CHANGELOG.md CONTRIBUTING.md CODE_OF_CONDUCT.md SECURITY.md LICENSE Makefile .gitignore .gitattributes .env.example THIRD_PARTY_NOTICES.md docs/README.md docs/CONTRIBUTING.md docs/INSTALLATION.md docs/USAGE.md docs/COMMANDS.md docs/CONFIGURATION.md docs/TROUBLESHOOTING.md docs/SECURITY.md docs/THREAT_MODEL.md docs/PLATFORM_SUPPORT.md docs/TESTING.md docs/ARCHITECTURE.md docs/ROADMAP.md docs/RELEASING.md docs/RELEASE_NOTES.md docs/LICENSES.md .github/PULL_REQUEST_TEMPLATE.md scripts/lib.sh scripts/publish-github.sh scripts/dev-setup.sh scripts/check.sh scripts/demo.sh scripts/package.sh"
project_required_dirs="migrations scripts assets testdata docs docs/adr"
project_make_targets="fmt fmt-check lint test race-or-sanitize build build-nogui check clean"
project_readme_sections="Download Install Usage Documentation Security Contributing License"

check_project_dir() {
  local dir="$1" item base="$WORKSPACE_ROOT/$1"
  for item in $project_required_files; do
    [ -f "$base/$item" ] || fail "$dir: missing file $item"
  done
  for item in $project_required_dirs; do
    [ -d "$base/$item" ] || fail "$dir: missing directory $item"
  done
  if ! compgen -G "$base/migrations/0001_*.sql" >/dev/null; then
    fail "$dir: missing migrations/0001_*.sql"
  fi
  if [ -f "$base/Makefile" ]; then
    for item in $project_make_targets; do
      grep -Eq "^${item}:" "$base/Makefile" || fail "$dir: Makefile has no target $item"
    done
  fi
  if [ -f "$base/README.md" ]; then
    for item in $project_readme_sections; do
      grep -Eq "^## ${item}\$" "$base/README.md" || fail "$dir: README.md has no section $item"
    done
  fi
}

check_registry() {
  local count number folder app lang status tagline topics icon description destination holder topic_count expected=1
  count="$(registry_rows | wc -l | tr -d ' ')"
  [ "$count" = "25" ] || fail "registry: expected 25 projects, found $count"
  while IFS=$'\t' read -r number folder app lang status tagline topics icon description destination holder; do
    [ "$number" = "$expected" ] || fail "registry: row $expected has number $number"
    expected=$((expected + 1))
    [[ "$folder" =~ ^[a-z][a-z0-9-]*$ ]] || fail "registry: invalid folder name $folder"
    [ -n "$app" ] || fail "registry: $folder has no app name"
    valid_language "$lang" || fail "registry: $folder has unknown language $lang"
    case "$status" in
      "Complete" | "In progress")
        [ -d "$WORKSPACE_ROOT/$folder" ] || fail "registry: $folder is marked $status but its folder does not exist"
        ;;
      "Not started")
        [ ! -d "$WORKSPACE_ROOT/$folder" ] || fail "registry: $folder exists but is marked Not started"
        ;;
      *) fail "registry: $folder has invalid status $status" ;;
    esac
    [ -n "$tagline" ] || fail "registry: $folder has no tagline"
    [ -n "$icon" ] || fail "registry: $folder has no icon concept"
    topic_count="$(printf '%s' "$topics" | awk -F',' '{ print NF }')"
    [ "$topic_count" = "7" ] || fail "registry: $folder has $topic_count topics, expected 7"
    [[ "$topics" =~ ^[a-z0-9,-]+$ ]] || fail "registry: $folder topics must be lower-case and hyphenated"
    if [ "${#description}" -lt 180 ] || [ "${#description}" -gt 300 ]; then
      fail "registry: $folder description has ${#description} characters, expected about 200 to 280"
    fi
    if valid_destination "$destination"; then
      [ "$holder" = "$(destination_holder "$destination")" ] ||
        fail "registry: $folder goes to $destination, so its copyright holder must be $(destination_holder "$destination"), not \"$holder\""
    else
      fail "registry: $folder has unknown destination \"$destination\" (use sonix-devs or personal)"
    fi
  done < <(registry_rows)
  for lang in go:14 rust:8 cpp:3; do
    count="$(registry_rows | awk -F'\t' -v language="${lang%%:*}" '$4 == language' | wc -l | tr -d ' ')"
    [ "$count" = "${lang##*:}" ] || fail "registry: expected ${lang##*:} ${lang%%:*} projects, found $count"
  done
  for destination in sonix-devs:17 personal:8; do
    count="$(registry_rows | awk -F'\t' -v wanted="${destination%%:*}" '$10 == wanted' | wc -l | tr -d ' ')"
    [ "$count" = "${destination##*:}" ] || fail "registry: expected ${destination##*:} projects for ${destination%%:*}, found $count"
  done
}

check_files() {
  local item language folder
  for item in $root_required; do
    [ -f "$WORKSPACE_ROOT/$item" ] || fail "root: missing file $item"
  done
  for item in templates templates/workflows docs .github/workflows .github/ISSUE_TEMPLATE; do
    [ -d "$WORKSPACE_ROOT/$item" ] || fail "root: missing directory $item"
  done
  while IFS= read -r language; do
    check_project_dir "templates/$language"
  done < <(existing_templates)
  while IFS= read -r folder; do
    check_project_dir "$folder"
  done < <(existing_projects)
}

check_emails() {
  local pattern='[A-Za-z0-9._%+-]+@[A-Za-z0-9-]+(\.[A-Za-z0-9-]+)*\.[A-Za-z]{2,}' hits
  hits="$(cd "$WORKSPACE_ROOT" && repo_files | while IFS= read -r file; do
    if [ -f "$file" ]; then
      LC_ALL=C grep -nIEH -- "$pattern" "$file" || true
    fi
  done | cut -d: -f1,2)"
  if [ -n "$hits" ]; then
    while IFS= read -r hit; do
      fail "email address found at $hit"
    done <<<"$hits"
  fi
}

comment_language() {
  case "$1" in
    *.go) printf 'go' ;;
    *.rs) printf 'rust' ;;
    *.c | *.cc | *.cpp | *.cxx | *.h | *.hh | *.hpp) printf 'cpp' ;;
    *.sql) printf 'sql' ;;
    *.sh | *.bats | *.bash) printf 'sh' ;;
    *) return 1 ;;
  esac
}

check_comments() {
  local file language output
  while IFS= read -r file; do
    case "/$file" in
      */testdata/* | */third_party/* | */vendor/*) continue ;;
    esac
    [ -f "$WORKSPACE_ROOT/$file" ] || continue
    language="$(comment_language "$file")" || continue
    if ! output="$(cd "$WORKSPACE_ROOT" && LC_ALL=C awk -v lang="$language" -f scripts/find-comments.awk "$file")"; then
      while IFS= read -r hit; do
        fail "source comment: $hit"
      done <<<"$output"
    fi
  done < <(repo_files)
}

check_workflows() {
  local output
  if ! output="$("$WORKSPACE_ROOT/scripts/gen-workflows.sh" --check --only workflows 2>&1)"; then
    fail "workflows are out of date: run scripts/gen-workflows.sh"
    printf '%s\n' "$output" >&2
  fi
}

check_pull_requests() {
  local output
  if ! output="$("$WORKSPACE_ROOT/scripts/gen-workflows.sh" --check --only pull-requests 2>&1)"; then
    fail "pull request templates are missing or differ: run scripts/gen-workflows.sh"
    printf '%s\n' "$output" >&2
  fi
}

check_readme() {
  local output
  if ! output="$("$WORKSPACE_ROOT/scripts/sync-readme.sh" --check projects 2>&1)"; then
    fail "README.md project table is out of date: run scripts/sync-readme.sh"
    printf '%s\n' "$output" >&2
  fi
}

check_destinations() {
  local output
  if ! output="$("$WORKSPACE_ROOT/scripts/sync-readme.sh" --check destinations 2>&1)"; then
    fail "the destination table disagrees with the registry: run scripts/sync-readme.sh"
    printf '%s\n' "$output" >&2
  fi
}

check_holders() {
  local language folder problem
  while IFS= read -r language; do
    while IFS= read -r problem; do
      fail "holder: $problem"
    done < <(holder_problems "$WORKSPACE_ROOT/templates/$language" "templates/$language" "$PERSONAL_HOLDER")
  done < <(existing_templates)
  while IFS= read -r folder; do
    while IFS= read -r problem; do
      fail "holder: $problem"
    done < <(holder_problems "$WORKSPACE_ROOT/$folder" "$folder" "$(project_copyright_holder "$folder")")
  done < <(existing_projects)
}

checks=()
while [ "$#" -gt 0 ]; do
  case "$1" in
    --help | -h)
      usage
      exit 0
      ;;
    registry | files | emails | comments | workflows | pull-requests | readme | destinations | holders) checks+=("$1") ;;
    *) die "unknown check: $1 (see --help)" ;;
  esac
  shift
done
if [ "${#checks[@]}" -eq 0 ]; then
  checks=(registry files emails comments workflows pull-requests readme destinations holders)
fi

for check in "${checks[@]}"; do
  before="$failures"
  "check_${check//-/_}"
  if [ "$failures" = "$before" ]; then
    note "ok   $check"
  fi
done

if [ "$failures" -gt 0 ]; then
  printf '%s check(s) failed\n' "$failures" >&2
  exit 1
fi
