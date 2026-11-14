#!/usr/bin/env bats

load helpers

setup() {
  make_sandbox
  fake_gh
  STANDALONE="$SANDBOX/standalone"
  copy_project_scripts "$STANDALONE"
  sed -i.bak \
    -e 's|^description=.*|description="A local tool used by the script tests."|' \
    -e 's|^topics=.*|topics="one,two,three,four,five,six,seven"|' \
    "$STANDALONE/scripts/publish-github.sh"
  rm -f "$STANDALONE/scripts/publish-github.sh.bak"
  git -C "$STANDALONE" init --quiet
  git -C "$STANDALONE" add -A
  git -C "$STANDALONE" commit --quiet -m "chore: initial public release"
}

teardown() {
  remove_sandbox
}

publish() {
  "$STANDALONE/scripts/publish-github.sh" "$@"
}

@test "publish --dry-run prints the plan and changes nothing" {
  run publish --dry-run
  [ "$status" -eq 0 ]
  [[ "$output" == *"dry-run: gh repo create"* ]]
  [[ "$output" == *"names\\[\\]=seven"* ]]
  [[ "$output" == *"refs/heads/main:refs/heads/main"* ]]
  [[ "$output" == *"private-vulnerability-reporting"* ]]
  refute grep -q "repo create" "$GH_LOG"
}

@test "publish skips creation when the repository exists" {
  GH_REPO_EXISTS=0 run publish --dry-run
  [ "$status" -eq 0 ]
  [[ "$output" == *"already exists"* ]]
  [[ "$output" != *"gh repo create"* ]]
}

@test "publish refuses to run inside a larger repository" {
  mkdir -p "$SANDBOX/mono"
  copy_project_scripts "$SANDBOX/mono/project"
  git -C "$SANDBOX/mono" init --quiet
  run "$SANDBOX/mono/project/scripts/publish-github.sh" --dry-run
  [ "$status" -ne 0 ]
  [[ "$output" == *"inside a larger repository"* ]]
}

@test "publish refuses another gh user" {
  GH_LOGIN=someone-else run publish --dry-run
  [ "$status" -ne 0 ]
  [[ "$output" == *"is not the owner"* ]]
}

@test "publish refuses a personal address in the git identity" {
  git -C "$STANDALONE" config user.email "person@invalid"
  run publish --dry-run
  [ "$status" -ne 0 ]
  [[ "$output" == *"not a GitHub noreply address"* ]]
}

@test "publish requires the first commit subject" {
  git -C "$STANDALONE" commit --quiet --amend -m "feat: something else"
  run publish --dry-run
  [ "$status" -ne 0 ]
  [[ "$output" == *"first commit on main must be"* ]]
}

@test "publish requires exactly seven topics" {
  sed -i.bak -e 's|^topics=.*|topics="one,two"|' "$STANDALONE/scripts/publish-github.sh"
  run publish --dry-run
  [ "$status" -ne 0 ]
  [[ "$output" == *"exactly seven topics"* ]]
}

@test "publish refuses a tag that does not exist" {
  run publish --dry-run --tag v0.0.0
  [ "$status" -ne 0 ]
  [[ "$output" == *"tag does not exist locally"* ]]
}
