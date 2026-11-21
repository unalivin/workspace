#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$script_dir/lib.sh"

usage() {
  cat <<'EOF'
Usage: new-project.sh [--dry-run] <project>

Scaffolds shared docs, PR template, and language template (if present) for <project>.
EOF
}

dry_run=0
project=""
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
      project="$1"
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
proj_dir="$root/$folder"

if [[ "$dry_run" -eq 1 ]]; then
  printf 'Would scaffold %s (%s, %s, owner=%s)\n' "$folder" "$app" "$language" "$owner"
  exit 0
fi

mkdir -p "$proj_dir/docs" "$proj_dir/.github" "$proj_dir/scripts" "$proj_dir/migrations" "$proj_dir/assets" "$proj_dir/testdata"

if [[ -d "$root/templates/$language" ]]; then
  cp -rn "$root/templates/$language/." "$proj_dir/" || true
fi

for doc in CONTRIBUTING.md CODE_OF_CONDUCT.md SECURITY.md; do
  sed \
    -e "s|{{PROJECT}}|$folder|g" \
    -e "s|{{APP}}|$app|g" \
    -e "s|{{OWNER}}|$owner|g" \
    -e "s|{{COPYRIGHT_HOLDER}}|$copyright_holder|g" \
    -e "s|{{TEST_CMD}}|make check|g" \
    "$root/templates/docs/$doc" > "$proj_dir/$doc"
done

cp "$proj_dir/CONTRIBUTING.md" "$proj_dir/docs/CONTRIBUTING.md"

sed \
  -e "s|{{PROJECT}}|$folder|g" \
  -e "s|{{APP}}|$app|g" \
  -e "s|{{OWNER}}|$owner|g" \
  "$root/templates/docs/PLATFORM_SUPPORT.md" > "$proj_dir/docs/PLATFORM_SUPPORT.md"

"$script_dir/gen-workflows.sh" "$folder"
