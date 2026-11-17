#!/usr/bin/env bash

REAL_PROJECT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
NOREPLY_DOMAIN="users.noreply.github.com"

make_sandbox() {
  SANDBOX="$(mktemp -d)"
  export GIT_CONFIG_GLOBAL="$SANDBOX/gitconfig"
  export GIT_CONFIG_NOSYSTEM=1
  unset GITHUB_ACTIONS DRY_RUN PROJECT_ROOT
  git config --global user.name "Project Tester"
  git config --global user.email "tester@${NOREPLY_DOMAIN}"
  git config --global init.defaultBranch main
  git config --global advice.detachedHead false
}

remove_sandbox() {
  if [ -n "${SANDBOX:-}" ] && [ -d "$SANDBOX" ]; then
    rm -rf "$SANDBOX"
  fi
}

copy_project_scripts() {
  local target="$1"
  mkdir -p "$target/scripts" "$target/docs"
  cp "$REAL_PROJECT"/scripts/*.sh "$target/scripts/"
  cp "$REAL_PROJECT/LICENSE" "$target/LICENSE"
  printf 'notes\n' >"$target/docs/RELEASE_NOTES.md"
}

project_value() {
  bash -c 'source "$1/scripts/lib.sh"; printf "%s" "${!2}"' project_value "$REAL_PROJECT" "$1"
}

make_release_repo() {
  RELEASE_REPO="$SANDBOX/repo"
  copy_project_scripts "$RELEASE_REPO"
  git -C "$RELEASE_REPO" init --quiet
  git -C "$RELEASE_REPO" add -A
  git -C "$RELEASE_REPO" commit --quiet -m "chore: initial public release"
  RELEASE_COMMIT="$(git -C "$RELEASE_REPO" rev-parse HEAD)"
  export RELEASE_COMMIT
  APP_SLUG="$(project_value APP_SLUG)"
  APP_VERSION="$(project_value APP_VERSION)"
  RELEASE_TAG="v$APP_VERSION"
  git -C "$RELEASE_REPO" tag "$RELEASE_TAG"
  git -C "$RELEASE_REPO" tag "$APP_SLUG/$RELEASE_TAG"
  RELEASE_FILES="$SANDBOX/files"
  mkdir -p "$RELEASE_FILES"
  local file
  for file in "$APP_SLUG-linux-x64.tar.gz" "$APP_SLUG-macos-arm64.tar.gz" "$APP_SLUG-windows-x64.exe"; do
    printf 'content\n' >"$RELEASE_FILES/$file"
  done
}

fake_gh() {
  mkdir -p "$SANDBOX/bin"
  cat >"$SANDBOX/bin/gh" <<'GH'
#!/usr/bin/env bash
printf '%s\n' "$*" >>"$GH_LOG"
case "$1 $2" in
  "api user") printf '%s\n' "${GH_LOGIN:-unalivin}" ;;
  "repo view") exit "${GH_REPO_EXISTS:-1}" ;;
  "release view") exit "${GH_RELEASE_EXISTS:-1}" ;;
esac
GH
  chmod +x "$SANDBOX/bin/gh"
  export GH_LOG="$SANDBOX/gh.log"
  : >"$GH_LOG"
  PATH="$SANDBOX/bin:$PATH"
}

refute() {
  if "$@"; then
    printf 'expected failure: %s\n' "$*" >&2
    return 1
  fi
}
