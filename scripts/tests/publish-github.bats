#!/usr/bin/env bats

load helpers

setup() {
  make_workspace
  make_fake_gh
  make_bare_remote
}

teardown() {
  remove_workspace
}

publish() {
  run "$WORKSPACE_ROOT/scripts/publish-github.sh" --remote-url "$REMOTE_DIR" "$@"
}

@test "publish-github prints help" {
  run "$WORKSPACE_ROOT/scripts/publish-github.sh" --help
  [ "$status" -eq 0 ]
  [[ "$output" == *"Usage: scripts/publish-github.sh"* ]]
}

@test "publish-github creates the repository, sets metadata and pushes main" {
  publish
  [ "$status" -eq 0 ]
  grep -q "^repo create NexzaDev/workspace --public --description Monorepo of 25 local-first defensive security and network tools in Go, Rust and C++, each with a desktop GUI, terminal UI and CLI, SQLite storage and tag-triggered pre-releases.$" "$GH_LOG"
  grep -q "^repo edit NexzaDev/workspace --description Monorepo of 25" "$GH_LOG"
  grep -q "^api --method PUT repos/NexzaDev/workspace/topics -f names\[\]=cybersecurity -f names\[\]=defensive-security -f names\[\]=network-security -f names\[\]=monorepo -f names\[\]=golang -f names\[\]=rust -f names\[\]=cpp$" "$GH_LOG"
  grep -q "^api --method PUT repos/NexzaDev/workspace/private-vulnerability-reporting$" "$GH_LOG"
  [ "$(git -C "$REMOTE_DIR" rev-parse refs/heads/main)" = "$(git -C "$WORKSPACE_ROOT" rev-parse refs/heads/main)" ]
}

@test "publish-github does not create a repository that exists" {
  FAKE_GH_REPO_EXISTS=1 publish
  [ "$status" -eq 0 ]
  [[ "$output" == *"already exists"* ]]
  refute grep -q "^repo create" "$GH_LOG"
  git -C "$REMOTE_DIR" rev-parse --verify --quiet refs/heads/main
}

@test "publish-github pushes a tag only when asked" {
  git -C "$WORKSPACE_ROOT" tag certwatch/v0.1.0-dev
  publish
  [ "$status" -eq 0 ]
  refute git -C "$REMOTE_DIR" rev-parse --verify --quiet refs/tags/certwatch/v0.1.0-dev
  publish --tag certwatch/v0.1.0-dev
  [ "$status" -eq 0 ]
  git -C "$REMOTE_DIR" rev-parse --verify --quiet refs/tags/certwatch/v0.1.0-dev
}

@test "publish-github dry run changes nothing" {
  publish --dry-run
  [ "$status" -eq 0 ]
  [[ "$output" == *"dry-run: gh repo create NexzaDev/workspace --public"* ]]
  [[ "$output" == *"dry-run: git -C"* ]]
  [[ "$output" == *"private-vulnerability-reporting"* ]]
  refute git -C "$REMOTE_DIR" rev-parse --verify --quiet refs/heads/main
  refute grep -q "^repo create" "$GH_LOG"
  refute grep -q "^api --method PUT" "$GH_LOG"
}

@test "publish-github refuses a different logged-in user" {
  FAKE_GH_LOGIN=someone-else publish
  [ "$status" -ne 0 ]
  [[ "$output" == *"is not the owner NexzaDev"* ]]
  refute git -C "$REMOTE_DIR" rev-parse --verify --quiet refs/heads/main
}

@test "publish-github accepts another owner when it matches the login" {
  FAKE_GH_LOGIN=octo publish --owner octo --repo tools
  [ "$status" -eq 0 ]
  grep -q "^repo create octo/tools --public" "$GH_LOG"
}

@test "publish-github refuses a git email that is not a noreply address" {
  git -C "$WORKSPACE_ROOT" config user.email "person@${NOREPLY_DOMAIN#users.noreply.}"
  publish
  [ "$status" -ne 0 ]
  [[ "$output" == *"not a GitHub noreply address"* ]]
  [[ "$output" != *"person@"* ]]
  refute grep -q "^repo create" "$GH_LOG"
}

@test "publish-github refuses history with a personal address" {
  printf 'change\n' >>"$WORKSPACE_ROOT/CHANGELOG.md"
  git -C "$WORKSPACE_ROOT" add -A
  GIT_AUTHOR_EMAIL="person@${NOREPLY_DOMAIN#users.noreply.}" git -C "$WORKSPACE_ROOT" commit --quiet -m "docs(workspace): change"
  publish
  [ "$status" -ne 0 ]
  [[ "$output" == *"non-noreply author or committer"* ]]
}

@test "publish-github requires the initial commit subject" {
  rm -rf "$WORKSPACE_ROOT/.git"
  git -C "$WORKSPACE_ROOT" init --quiet
  commit_all "feat: something else"
  publish
  [ "$status" -ne 0 ]
  [[ "$output" == *"first commit on main must be"* ]]
}

@test "publish-github requires main and an existing tag" {
  publish --tag missing/v1
  [ "$status" -ne 0 ]
  [[ "$output" == *"tag does not exist locally"* ]]
  git -C "$WORKSPACE_ROOT" branch -m main trunk
  publish
  [ "$status" -ne 0 ]
  [[ "$output" == *"local branch main does not exist"* ]]
}
