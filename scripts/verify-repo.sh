#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$script_dir/lib.sh"

usage() {
  cat <<'EOF'
Usage: verify-repo.sh

Verifies repository invariants: no email addresses, no process notes in projects,
workflows and PR templates match templates, and README tables match scripts/projects.tsv.
EOF
}

if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
  usage
  exit 0
fi

root="$(repo_root)"
errors=0

if grep -rnE '[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}' "$root" \
  --exclude-dir=.git --exclude-dir=target --exclude-dir=build --exclude-dir=dist >/dev/null 2>&1; then
  printf 'Email address found in repository:\n' >&2
  grep -rnE '[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}' "$root" \
    --exclude-dir=.git --exclude-dir=target --exclude-dir=build --exclude-dir=dist >&2 || true
  errors=1
fi

readme_diff="$("$script_dir/sync-readme.sh" --dry-run)"
if [[ -n "$readme_diff" ]]; then
  printf 'README.md is out of sync with scripts/projects.tsv. Run scripts/sync-readme.sh.\n' >&2
  errors=1
fi

while IFS=$'\t' read -r folder _app _lang _desc _dest _copy _status; do
  if [[ ! -f "$root/.github/workflows/${folder}-ci.yml" ]]; then
    printf 'Missing workflow: .github/workflows/%s-ci.yml\n' "$folder" >&2
    errors=1
  fi
  if [[ ! -f "$root/.github/workflows/${folder}-release.yml" ]]; then
    printf 'Missing workflow: .github/workflows/%s-release.yml\n' "$folder" >&2
    errors=1
  fi
  if [[ ! -f "$root/.github/PULL_REQUEST_TEMPLATE/${folder}.md" ]]; then
    printf 'Missing PR template: .github/PULL_REQUEST_TEMPLATE/%s.md\n' "$folder" >&2
    errors=1
  fi
  if [[ -d "$root/$folder" ]]; then
    if [[ -d "$root/$folder/.workspace" ]]; then
      printf 'Forbidden .workspace directory inside %s\n' "$folder" >&2
      errors=1
    fi
    if [[ ! -f "$root/$folder/.github/PULL_REQUEST_TEMPLATE.md" ]]; then
      printf 'Missing %s/.github/PULL_REQUEST_TEMPLATE.md\n' "$folder" >&2
      errors=1
    elif ! cmp -s "$root/$folder/.github/PULL_REQUEST_TEMPLATE.md" "$root/.github/PULL_REQUEST_TEMPLATE/${folder}.md"; then
      printf 'PR template mismatch for %s\n' "$folder" >&2
      errors=1
    fi
  fi
done < <(read_registry)

exit "$errors"
