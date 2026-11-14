#!/usr/bin/env bash
set -euo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

subject="chore: initial public release"

usage() {
  cat <<'USAGE'
Usage: scripts/init-main.sh [--dry-run] [--from <ref>] [--replace]

Creates the local branch main as one single commit named
"chore: initial public release" that holds exactly the files of <ref>
(default: HEAD). Earlier history is not carried over. Nothing is checked out
and nothing is pushed; scripts/publish-github.sh pushes main afterwards.

  --dry-run   Print what would happen without creating the branch
  --from      Commit, branch or tag whose files become the first commit
  --replace   Replace an existing local branch main
  --help      Show this help
USAGE
}

from="HEAD"
replace=0
while [ "$#" -gt 0 ]; do
  case "$1" in
    --dry-run) DRY_RUN=1 ;;
    --from)
      [ "$#" -ge 2 ] || die "--from needs a value"
      from="$2"
      shift
      ;;
    --replace) replace=1 ;;
    --help | -h)
      usage
      exit 0
      ;;
    *) die "unknown argument: $1 (see --help)" ;;
  esac
  shift
done

email="$(git -C "$WORKSPACE_ROOT" config user.email || true)"
case "$email" in
  *@users.noreply.github.com) ;;
  *) die "git user.email is not a GitHub noreply address; refusing to create the commit" ;;
esac

tree="$(git -C "$WORKSPACE_ROOT" rev-parse --verify --quiet "$from^{tree}")" || die "not a commit: $from"

if git -C "$WORKSPACE_ROOT" rev-parse --verify --quiet refs/heads/main >/dev/null; then
  [ "$replace" = "1" ] || die "local branch main already exists (use --replace to replace it)"
  if [ "$(git -C "$WORKSPACE_ROOT" symbolic-ref --quiet --short HEAD || true)" = "main" ]; then
    die "main is checked out; switch to another branch before replacing it"
  fi
fi

if [ "$DRY_RUN" = "1" ]; then
  note "dry-run: would create main as one commit \"$subject\" with the files of $from"
  exit 0
fi

commit="$(git -C "$WORKSPACE_ROOT" commit-tree "$tree" -m "$subject")"
git -C "$WORKSPACE_ROOT" update-ref refs/heads/main "$commit"
note "main is now $commit: $subject"
note "next: scripts/publish-github.sh"
