#!/usr/bin/env bash
set -euo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

usage() {
  cat <<'USAGE'
Usage: scripts/gen-workflows.sh [--check] [--dry-run]
       scripts/gen-workflows.sh --standalone <project> --output <directory>

Generates .github/workflows from templates/workflows:

  root-ci.yml                  repository-wide checks
  template-<language>-ci.yml   one per language template that exists
  <project>-ci.yml             one per project folder that exists
  <project>-release.yml        one per project folder that exists

Each generated workflow has a path filter so a change to one project only
runs that project's jobs. Workflow files that are no longer generated are
removed.

  --check        Exit with status 1 if the committed workflows differ
  --dry-run      Print the files that would be written or removed
  --standalone   Write ci.yml and release.yml for a project that lives at the
                 root of its own repository (used by scripts/split-project.sh)
  --output       Target directory for --standalone
  --help         Show this help
USAGE
}

check=0
standalone=""
output=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    --check) check=1 ;;
    --dry-run) DRY_RUN=1 ;;
    --standalone)
      [ "$#" -ge 2 ] || die "--standalone needs a project name"
      standalone="$2"
      shift
      ;;
    --output)
      [ "$#" -ge 2 ] || die "--output needs a directory"
      output="$2"
      shift
      ;;
    --help | -h)
      usage
      exit 0
      ;;
    *) die "unknown argument: $1 (see --help)" ;;
  esac
  shift
done

templates_dir="$WORKSPACE_ROOT/templates/workflows"
workflows_dir="$WORKSPACE_ROOT/.github/workflows"

escape_replacement() {
  printf '%s' "$1" | sed -e 's/[\\&|]/\\&/g'
}

render() {
  local template="$1" target="$2" name="$3" app="$4" workdir="$5" path_filter="$6" tag_pattern="$7" self="$8"
  [ -f "$template" ] || die "workflow template not found: ${template#"$WORKSPACE_ROOT"/}"
  sed \
    -e "s|__NAME__|$(escape_replacement "$name")|g" \
    -e "s|__APP_NAME__|$(escape_replacement "$app")|g" \
    -e "s|__WORKDIR__|$(escape_replacement "$workdir")|g" \
    -e "s|__PATH_FILTER__|$(escape_replacement "$path_filter")|g" \
    -e "s|__TAG_PATTERN__|$(escape_replacement "$tag_pattern")|g" \
    -e "s|__SELF__|$(escape_replacement "$self")|g" \
    "$template" >"$target"
  if grep -n '__[A-Z_]*__' "$target" >&2; then
    die "unresolved token in ${target##*/}"
  fi
}

if [ -n "$standalone" ]; then
  [ -n "$output" ] || die "--standalone needs --output <directory>"
  registry_has "$standalone" || die "unknown project: $standalone"
  language="$(project_language "$standalone")"
  app="$(project_app "$standalone")"
  if [ "$DRY_RUN" = "1" ]; then
    note "dry-run: would write $output/ci.yml and $output/release.yml"
    exit 0
  fi
  mkdir -p "$output"
  render "$templates_dir/$language-ci.yml" "$output/ci.yml" "$standalone" "$app" "." "**" "v*" ".github/workflows/ci.yml"
  render "$templates_dir/$language-release.yml" "$output/release.yml" "$standalone" "$app" "." "**" "v*" ".github/workflows/release.yml"
  note "wrote $output/ci.yml"
  note "wrote $output/release.yml"
  exit 0
fi

staging="$(mktemp -d)"
trap 'rm -rf "$staging"' EXIT

if [ -f "$templates_dir/root-ci.yml" ]; then
  cp "$templates_dir/root-ci.yml" "$staging/root-ci.yml"
fi

while IFS= read -r language; do
  render "$templates_dir/$language-ci.yml" "$staging/template-$language-ci.yml" \
    "template-$language" "Template App" "templates/$language" "templates/$language/**" "template-$language/v*" \
    ".github/workflows/template-$language-ci.yml"
done < <(existing_templates)

while IFS= read -r project; do
  language="$(project_language "$project")"
  app="$(project_app "$project")"
  render "$templates_dir/$language-ci.yml" "$staging/$project-ci.yml" \
    "$project" "$app" "$project" "$project/**" "$project/v*" ".github/workflows/$project-ci.yml"
  render "$templates_dir/$language-release.yml" "$staging/$project-release.yml" \
    "$project" "$app" "$project" "$project/**" "$project/v*" ".github/workflows/$project-release.yml"
done < <(existing_projects)

changed=0
mkdir -p "$workflows_dir"
for generated in "$staging"/*.yml; do
  [ -e "$generated" ] || continue
  name="${generated##*/}"
  if [ ! -f "$workflows_dir/$name" ] || ! cmp -s "$generated" "$workflows_dir/$name"; then
    changed=1
    if [ "$check" = "1" ]; then
      printf 'out of date: .github/workflows/%s\n' "$name" >&2
    elif [ "$DRY_RUN" = "1" ]; then
      note "dry-run: would write .github/workflows/$name"
    else
      cp "$generated" "$workflows_dir/$name"
      note "wrote .github/workflows/$name"
    fi
  fi
done
for existing in "$workflows_dir"/*.yml; do
  [ -e "$existing" ] || continue
  name="${existing##*/}"
  if [ ! -f "$staging/$name" ]; then
    changed=1
    if [ "$check" = "1" ]; then
      printf 'not generated: .github/workflows/%s\n' "$name" >&2
    elif [ "$DRY_RUN" = "1" ]; then
      note "dry-run: would remove .github/workflows/$name"
    else
      rm -f "$existing"
      note "removed .github/workflows/$name"
    fi
  fi
done

if [ "$check" = "1" ] && [ "$changed" = "1" ]; then
  exit 1
fi
if [ "$changed" = "0" ]; then
  note "workflows are up to date"
fi
