#!/usr/bin/env bash
set -euo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

usage() {
  cat <<'USAGE'
Usage: scripts/gen-workflows.sh [--check] [--dry-run] [--only workflows|pull-requests]
       scripts/gen-workflows.sh --standalone <project> --output <directory>

Generates .github/workflows from templates/workflows:

  root-ci.yml                  repository-wide checks
  template-<language>-ci.yml   one per language template that exists
  <project>-ci.yml             one per project folder that exists
  <project>-release.yml        one per project folder that exists

Each generated workflow has a path filter so a change to one project only
runs that project's jobs. Workflow files that are no longer generated are
removed.

Generates the pull request templates:

  <project>/.github/PULL_REQUEST_TEMPLATE.md   created from the template of
                                               the project's language when
                                               it is missing; afterwards the
                                               project owns and edits it
  .github/PULL_REQUEST_TEMPLATE/<project>.md   exact copy of the project's
                                               template, selectable with
                                               ?template=<project>.md
  .github/PULL_REQUEST_TEMPLATE.md             root default from
                                               templates/pull-request/root.md,
                                               listing every project template

  --check        Exit with status 1 if a committed file is missing or differs
  --dry-run      Print the files that would be written or removed
  --only         Handle only the workflows or only the pull request templates
  --standalone   Write ci.yml and release.yml for a project that lives at the
                 root of its own repository (used by scripts/split-project.sh)
  --output       Target directory for --standalone
  --help         Show this help
USAGE
}

check=0
standalone=""
output=""
only=""
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
    --only)
      [ "$#" -ge 2 ] || die "--only needs workflows or pull-requests"
      case "$2" in
        workflows | pull-requests) only="$2" ;;
        *) die "unknown --only value: $2 (use workflows or pull-requests)" ;;
      esac
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
changed=0

report() {
  local action="$1" relative="$2"
  changed=1
  if [ "$check" = "1" ]; then
    case "$action" in
      write) printf 'out of date: %s\n' "$relative" >&2 ;;
      create) printf 'missing: %s\n' "$relative" >&2 ;;
      remove) printf 'not generated: %s\n' "$relative" >&2 ;;
    esac
    return 1
  fi
  if [ "$DRY_RUN" = "1" ]; then
    case "$action" in
      remove) note "dry-run: would remove $relative" ;;
      *) note "dry-run: would write $relative" ;;
    esac
    return 1
  fi
  return 0
}

sync_file() {
  local generated="$1" relative="$2" target="$WORKSPACE_ROOT/$2"
  if [ -f "$target" ] && cmp -s "$generated" "$target"; then
    return 0
  fi
  if report write "$relative"; then
    mkdir -p "$(dirname "$target")"
    cp "$generated" "$target"
    note "wrote $relative"
  fi
}

sync_directory() {
  local source_dir="$1" relative_dir="$2" extension="$3" generated existing name
  for generated in "$source_dir"/*."$extension"; do
    [ -e "$generated" ] || continue
    sync_file "$generated" "$relative_dir/${generated##*/}"
  done
  for existing in "$WORKSPACE_ROOT/$relative_dir"/*."$extension"; do
    [ -e "$existing" ] || continue
    name="${existing##*/}"
    if [ ! -f "$source_dir/$name" ] && report remove "$relative_dir/$name"; then
      rm -f "$existing"
      note "removed $relative_dir/$name"
    fi
  done
}

generate_workflows() {
  local stage="$staging/workflows" language project app
  mkdir -p "$stage"
  if [ -f "$templates_dir/root-ci.yml" ]; then
    cp "$templates_dir/root-ci.yml" "$stage/root-ci.yml"
  fi
  while IFS= read -r language; do
    render "$templates_dir/$language-ci.yml" "$stage/template-$language-ci.yml" \
      "template-$language" "Template App" "templates/$language" "templates/$language/**" "template-$language/v*" \
      ".github/workflows/template-$language-ci.yml"
  done < <(existing_templates)
  while IFS= read -r project; do
    language="$(project_language "$project")"
    app="$(project_app "$project")"
    render "$templates_dir/$language-ci.yml" "$stage/$project-ci.yml" \
      "$project" "$app" "$project" "$project/**" "$project/v*" ".github/workflows/$project-ci.yml"
    render "$templates_dir/$language-release.yml" "$stage/$project-release.yml" \
      "$project" "$app" "$project" "$project/**" "$project/v*" ".github/workflows/$project-release.yml"
  done < <(existing_projects)
  mkdir -p "$workflows_dir"
  sync_directory "$stage" ".github/workflows" yml
}

generate_pull_request_templates() {
  local stage="$staging/pull-requests" project language source own tokens list root
  mkdir -p "$stage"
  list="$staging/pull-request-list"
  : >"$list"
  while IFS= read -r project; do
    language="$(project_language "$project")"
    own="$project/.github/PULL_REQUEST_TEMPLATE.md"
    if [ ! -f "$WORKSPACE_ROOT/$own" ]; then
      source="$WORKSPACE_ROOT/templates/$language/.github/PULL_REQUEST_TEMPLATE.md"
      [ -f "$source" ] || die "templates/$language has no .github/PULL_REQUEST_TEMPLATE.md to create $own from"
      if report create "$own"; then
        tokens="$staging/tokens-$project"
        project_token_script "$project" >"$tokens"
        mkdir -p "$WORKSPACE_ROOT/$project/.github"
        cp "$source" "$WORKSPACE_ROOT/$own"
        apply_token_script "$tokens" "$WORKSPACE_ROOT/$own"
        note "wrote $own"
      fi
    fi
    if [ -f "$WORKSPACE_ROOT/$own" ]; then
      cp "$WORKSPACE_ROOT/$own" "$stage/$project.md"
    fi
    printf -- "- \`%s\` (%s): \`?template=%s.md\`\n" "$project" "$(project_app "$project")" "$project" >>"$list"
  done < <(existing_projects)
  if [ ! -s "$list" ]; then
    printf -- '- No project folder exists yet.\n' >"$list"
  fi
  mkdir -p "$WORKSPACE_ROOT/.github/PULL_REQUEST_TEMPLATE"
  sync_directory "$stage" ".github/PULL_REQUEST_TEMPLATE" md
  root="$WORKSPACE_ROOT/templates/pull-request/root.md"
  [ -f "$root" ] || die "templates/pull-request/root.md not found"
  grep -q '^__PROJECT_TEMPLATES__$' "$root" || die "templates/pull-request/root.md has no __PROJECT_TEMPLATES__ line"
  awk -v list="$list" '
    $0 == "__PROJECT_TEMPLATES__" {
      while ((getline row < list) > 0) {
        print row
      }
      close(list)
      next
    }
    { print }
  ' "$root" >"$staging/root-default.md"
  sync_file "$staging/root-default.md" ".github/PULL_REQUEST_TEMPLATE.md"
}

if [ -z "$only" ] || [ "$only" = "workflows" ]; then
  generate_workflows
fi
if [ -z "$only" ] || [ "$only" = "pull-requests" ]; then
  generate_pull_request_templates
fi

if [ "$check" = "1" ] && [ "$changed" = "1" ]; then
  exit 1
fi
if [ "$changed" = "0" ]; then
  note "workflows and pull request templates are up to date"
fi
