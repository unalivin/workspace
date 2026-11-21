#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$script_dir/lib.sh"

usage() {
  cat <<'EOF'
Usage: gen-workflows.sh [--dry-run] [project...]

Generates .github/workflows/<project>-ci.yml, <project>-release.yml, and PR templates.
EOF
}

dry_run=0
targets=()
while [[ $# -gt 0 ]]; do
  case "$1" in
    --help|-h)
      usage
      exit 0
      ;;
    --dry-run)
      dry_run=1
      shift
      ;;
    *)
      targets+=("$1")
      shift
      ;;
  esac
done

root="$(repo_root)"
wf_dir="$root/.github/workflows"
pr_dir="$root/.github/PULL_REQUEST_TEMPLATE"

if [[ "$dry_run" -eq 0 ]]; then
  mkdir -p "$wf_dir" "$pr_dir"
fi

render_template() {
  local src="$1"
  local dst="$2"
  local project="$3"
  local app="$4"
  local owner="$5"
  local content
  content="$(sed \
    -e "s|{{PROJECT}}|$project|g" \
    -e "s|{{APP}}|$app|g" \
    -e "s|{{OWNER}}|$owner|g" \
    "$src")"
  if [[ "$dry_run" -eq 1 ]]; then
    printf 'Would write %s\n' "$dst"
  else
    mkdir -p "$(dirname "$dst")"
    printf '%s\n' "$content" > "$dst"
  fi
}

while IFS=$'\t' read -r folder app language _desc destination _copy _status; do
  if [[ ${#targets[@]} -gt 0 ]]; then
    match=0
    for t in "${targets[@]}"; do
      if [[ "$t" == "$folder" ]]; then
        match=1
        break
      fi
    done
    if [[ "$match" -eq 0 ]]; then
      continue
    fi
  fi

  owner="$(owner_for_destination "$destination")"
  render_template "$root/templates/workflows/${language}-ci.yml" "$wf_dir/${folder}-ci.yml" "$folder" "$app" "$owner"
  render_template "$root/templates/workflows/${language}-release.yml" "$wf_dir/${folder}-release.yml" "$folder" "$app" "$owner"
  render_template "$root/templates/pull-request/PULL_REQUEST_TEMPLATE.md" "$pr_dir/${folder}.md" "$folder" "$app" "$owner"

  if [[ -d "$root/$folder" ]]; then
    render_template "$root/templates/pull-request/PULL_REQUEST_TEMPLATE.md" "$root/$folder/.github/PULL_REQUEST_TEMPLATE.md" "$folder" "$app" "$owner"
  fi
done < <(read_registry)
