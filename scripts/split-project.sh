#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$script_dir/lib.sh"

usage() {
  cat <<'EOF'
Usage: split-project.sh [--dry-run] <project> [output-dir]

Splits <project> into a standalone git repository using git subtree split.
EOF
}

dry_run=0
project=""
out_dir=""

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
      if [[ -z "$project" ]]; then
        project="$1"
      else
        out_dir="$1"
      fi
      shift
      ;;
  esac
done

if [[ -z "$project" ]]; then
  usage >&2
  exit 1
fi

row="$(lookup_project "$project")" || {
  printf 'Project not found in registry: %s\n' "$project" >&2
  exit 1
}

IFS=$'\t' read -r folder app language _desc destination copyright_holder _status <<< "$row"
owner="$(owner_for_destination "$destination")"
root="$(repo_root)"

if [[ ! -d "$root/$folder" ]]; then
  printf 'Project folder does not exist: %s\n' "$root/$folder" >&2
  exit 1
fi

if [[ -z "$out_dir" ]]; then
  out_dir="/tmp/${folder}-split"
fi

if [[ "$dry_run" -eq 1 ]]; then
  printf 'Would split %s (owner=%s, copyright=%s) to %s\n' "$folder" "$owner" "$copyright_holder" "$out_dir"
  exit 0
fi

branch="split-${folder}-$$"
git -C "$root" subtree split --prefix="$folder" -b "$branch"
rm -rf "$out_dir"
mkdir -p "$out_dir"
git init "$out_dir"
git -C "$out_dir" pull "$root" "$branch"
git -C "$root" branch -D "$branch"

mkdir -p "$out_dir/.github/workflows"
sed \
  -e "s|{{PROJECT}}|$folder|g" \
  -e "s|{{APP}}|$app|g" \
  -e "s|{{OWNER}}|$owner|g" \
  -e "s|{{PROJECT}}/||g" \
  "$root/templates/workflows/${language}-ci.yml" > "$out_dir/.github/workflows/ci.yml"

sed \
  -e "s|{{PROJECT}}|$folder|g" \
  -e "s|{{APP}}|$app|g" \
  -e "s|{{OWNER}}|$owner|g" \
  -e "s|{{PROJECT}}/v\*|v*|g" \
  "$root/templates/workflows/${language}-release.yml" > "$out_dir/.github/workflows/release.yml"

printf 'Split %s to %s (owner=%s, copyright=%s)\n' "$folder" "$out_dir" "$owner" "$copyright_holder"
