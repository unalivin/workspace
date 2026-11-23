#!/usr/bin/env bash
set -euo pipefail

usage() {
  printf 'Usage: scripts/strip-dev-files.sh [--dry-run] logforge-siem\n'
}

dry_run=0
project=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --help|-h) usage; exit 0 ;;
    --dry-run) dry_run=1 ;;
    *)
      if [[ -n "$project" ]]; then
        usage >&2
        exit 2
      fi
      project="$1"
      ;;
  esac
  shift
done

[[ "$project" == "logforge-siem" ]] || { usage >&2; exit 2; }
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
project_dir="$root/$project"
archive="$root/.workspace/dev-archive/$project"
[[ -d "$project_dir" ]] || { printf 'Project folder missing: %s\n' "$project_dir" >&2; exit 2; }

move_item() {
  local relative="$1"
  local source="$project_dir/$relative"
  local destination="$archive/$relative"
  [[ -e "$source" ]] || return 0
  if [[ "$dry_run" == 1 ]]; then
    printf 'Would move %s to %s\n' "$relative" ".workspace/dev-archive/$project/$relative"
    return 0
  fi
  [[ ! -e "$destination" ]] || { printf 'Archive destination exists: %s\n' "$destination" >&2; exit 1; }
  mkdir -p "$(dirname "$destination")"
  mv "$source" "$destination"
  printf 'Moved %s to .workspace/dev-archive/%s/%s\n' "$relative" "$project" "$relative"
}

for relative in \
  Makefile \
  .github/PULL_REQUEST_TEMPLATE.md \
  docs/CONTRIBUTING.md \
  docs/TESTING.md \
  docs/ROADMAP.md \
  docs/THREAT_MODEL.md \
  docs/PLATFORM_SUPPORT.md \
  scripts; do
  move_item "$relative"
done

while IFS= read -r -d '' test_dir; do
  relative="${test_dir#"$project_dir/"}"
  move_item "$relative"
done < <(find "$project_dir/crates" -type d -name tests -print0 2>/dev/null)

while IFS= read -r -d '' source_file; do
  line="$(grep -n -m 1 '^#\[cfg(test)\]' "$source_file" | cut -d: -f1)"
  relative="${source_file#"$project_dir/"}"
  archived="$archive/inline-tests/$relative"
  if [[ "$dry_run" == 1 ]]; then
    printf 'Would archive inline tests from %s starting at line %s\n' "$relative" "$line"
    continue
  fi
  [[ ! -e "$archived" ]] || { printf 'Inline-test archive exists: %s\n' "$archived" >&2; exit 1; }
  mkdir -p "$(dirname "$archived")"
  tail -n "+$line" "$source_file" > "$archived"
  head -n "$((line - 1))" "$source_file" > "$source_file.strip-tmp"
  mv "$source_file.strip-tmp" "$source_file"
  printf 'Archived inline tests from %s\n' "$relative"
done < <(grep -RIl '#\[cfg(test)\]' "$project_dir/crates" --include='*.rs' -Z)

if [[ "$dry_run" == 0 && -f "$project_dir/docs/README.md" ]]; then
  sed -i \
    -e '/- \[Testing\](TESTING.md)/d' \
    -e '/- \[Threat Model\](THREAT_MODEL.md)/d' \
    -e '/- \[Platform Support\](PLATFORM_SUPPORT.md)/d' \
    "$project_dir/docs/README.md"
fi
