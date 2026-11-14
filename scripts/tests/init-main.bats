#!/usr/bin/env bats

load helpers

setup() {
  make_workspace
  git -C "$WORKSPACE_ROOT" branch -m main work
  printf 'second\n' >"$WORKSPACE_ROOT/docs/second.md"
  commit_all "feat: second change"
}

teardown() {
  remove_workspace
}

@test "init-main prints help" {
  run "$WORKSPACE_ROOT/scripts/init-main.sh" --help
  [ "$status" -eq 0 ]
  [[ "$output" == *"Usage: scripts/init-main.sh"* ]]
}

@test "init-main creates main as one commit with the files of HEAD" {
  run "$WORKSPACE_ROOT/scripts/init-main.sh"
  [ "$status" -eq 0 ]
  [ "$(git -C "$WORKSPACE_ROOT" rev-list --count main)" = "1" ]
  [ "$(git -C "$WORKSPACE_ROOT" log -1 --format=%s main)" = "chore: initial public release" ]
  [ "$(git -C "$WORKSPACE_ROOT" rev-parse 'main^{tree}')" = "$(git -C "$WORKSPACE_ROOT" rev-parse 'work^{tree}')" ]
  [ "$(git -C "$WORKSPACE_ROOT" symbolic-ref --short HEAD)" = "work" ]
}

@test "init-main output is accepted by publish-github" {
  make_fake_gh
  make_bare_remote
  "$WORKSPACE_ROOT/scripts/init-main.sh"
  run "$WORKSPACE_ROOT/scripts/publish-github.sh" --remote-url "$REMOTE_DIR"
  [ "$status" -eq 0 ]
  [ "$(git -C "$REMOTE_DIR" rev-list --count refs/heads/main)" = "1" ]
}

@test "init-main dry run creates nothing" {
  run "$WORKSPACE_ROOT/scripts/init-main.sh" --dry-run
  [ "$status" -eq 0 ]
  [[ "$output" == *"dry-run: would create main"* ]]
  refute git -C "$WORKSPACE_ROOT" rev-parse --verify --quiet refs/heads/main
}

@test "init-main keeps an existing main unless asked to replace it" {
  "$WORKSPACE_ROOT/scripts/init-main.sh"
  first="$(git -C "$WORKSPACE_ROOT" rev-parse main)"
  printf 'third\n' >"$WORKSPACE_ROOT/docs/third.md"
  commit_all "feat: third change"
  run "$WORKSPACE_ROOT/scripts/init-main.sh"
  [ "$status" -ne 0 ]
  [[ "$output" == *"already exists"* ]]
  [ "$(git -C "$WORKSPACE_ROOT" rev-parse main)" = "$first" ]
  run "$WORKSPACE_ROOT/scripts/init-main.sh" --replace
  [ "$status" -eq 0 ]
  [ "$(git -C "$WORKSPACE_ROOT" rev-list --count main)" = "1" ]
  [ "$(git -C "$WORKSPACE_ROOT" rev-parse 'main^{tree}')" = "$(git -C "$WORKSPACE_ROOT" rev-parse 'work^{tree}')" ]
}

@test "init-main refuses a personal address, an unknown ref and an unknown option" {
  run "$WORKSPACE_ROOT/scripts/init-main.sh" --from no-such-ref
  [ "$status" -ne 0 ]
  [[ "$output" == *"not a commit"* ]]
  run "$WORKSPACE_ROOT/scripts/init-main.sh" --wrong
  [ "$status" -ne 0 ]
  git config --global user.email "person@${NOREPLY_DOMAIN#users.noreply.}"
  run "$WORKSPACE_ROOT/scripts/init-main.sh"
  [ "$status" -ne 0 ]
  [[ "$output" == *"noreply"* ]]
}
