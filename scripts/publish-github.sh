#!/usr/bin/env bash
set -euo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

description="Monorepo of 25 local-first defensive security and network tools in Go, Rust and C++, each with a desktop GUI, terminal UI and CLI, SQLite storage and tag-triggered pre-releases."
topics="cybersecurity defensive-security network-security monorepo golang rust cpp"
first_commit_subject="chore: initial public release"

usage() {
  cat <<'USAGE'
Usage: scripts/publish-github.sh [--dry-run] [--owner <login>] [--repo <name>] [--remote-url <url>] [--tag <tag>]

Publishes the workspace as a public GitHub repository with gh:

  1. checks that the logged-in gh user is the owner
  2. refuses to continue unless git user.email is a GitHub noreply address
  3. checks that the first commit on main is "chore: initial public release"
  4. creates the public repository if it does not exist
  5. sets the description and exactly seven topics
  6. pushes main
  7. enables private vulnerability reporting
  8. pushes one tag when --tag is given

  --dry-run     Print every command instead of running it
  --owner       GitHub login that owns the repository (default: NexzaDev)
  --repo        Repository name (default: workspace)
  --remote-url  Push target (default: https://github.com/<owner>/<repo>.git)
  --tag         Existing local tag to push after main
  --help        Show this help
USAGE
}

owner="$WORKSPACE_OWNER"
repo="$WORKSPACE_REPO"
remote_url=""
tag=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    --dry-run) DRY_RUN=1 ;;
    --owner | --repo | --remote-url | --tag)
      [ "$#" -ge 2 ] || die "$1 needs a value"
      case "$1" in
        --owner) owner="$2" ;;
        --repo) repo="$2" ;;
        --remote-url) remote_url="$2" ;;
        --tag) tag="$2" ;;
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
if [ -z "$remote_url" ]; then
  remote_url="https://github.com/$owner/$repo.git"
fi

command -v gh >/dev/null 2>&1 || die "gh is not installed"
command -v git >/dev/null 2>&1 || die "git is not installed"

login="$(gh api user --jq .login)" || die "gh is not logged in; run gh auth login"
[ "$login" = "$owner" ] || die "logged-in gh user $login is not the owner $owner"

email="$(git -C "$WORKSPACE_ROOT" config user.email || true)"
case "$email" in
  *@users.noreply.github.com) ;;
  *) die "git user.email is not a GitHub noreply address; refusing to publish" ;;
esac

git -C "$WORKSPACE_ROOT" rev-parse --verify --quiet refs/heads/main >/dev/null || die "local branch main does not exist"
root_subject="$(git -C "$WORKSPACE_ROOT" log --reverse --format=%s main | head -n 1)"
[ "$root_subject" = "$first_commit_subject" ] || die "the first commit on main must be \"$first_commit_subject\", found \"$root_subject\""

if git -C "$WORKSPACE_ROOT" log --format='%ae%n%ce' main | sort -u | grep -v '@users\.noreply\.github\.com$' | grep -q .; then
  die "main contains commits with a non-noreply author or committer address"
fi

if [ -n "$tag" ]; then
  git -C "$WORKSPACE_ROOT" rev-parse --verify --quiet "refs/tags/$tag" >/dev/null || die "tag does not exist locally: $tag"
fi

if gh repo view "$owner/$repo" >/dev/null 2>&1; then
  note "repository $owner/$repo already exists"
else
  run gh repo create "$owner/$repo" --public --description "$description"
fi

run gh repo edit "$owner/$repo" --description "$description"

topic_args=()
for topic in $topics; do
  topic_args+=(-f "names[]=$topic")
done
[ "${#topic_args[@]}" -eq 14 ] || die "exactly seven topics are required"
run gh api --method PUT "repos/$owner/$repo/topics" "${topic_args[@]}"

run git -C "$WORKSPACE_ROOT" push "$remote_url" refs/heads/main:refs/heads/main

run gh api --method PUT "repos/$owner/$repo/private-vulnerability-reporting"

if [ -n "$tag" ]; then
  run git -C "$WORKSPACE_ROOT" push "$remote_url" "refs/tags/$tag"
fi

note "published https://github.com/$owner/$repo"
