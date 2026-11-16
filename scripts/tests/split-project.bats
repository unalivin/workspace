#!/usr/bin/env bats

load helpers

setup() {
  make_workspace
  make_fake_template go
  "$WORKSPACE_ROOT/scripts/new-project.sh" certwatch go >/dev/null
  commit_all "feat(certwatch): create project from template"
  printf 'more\n' >>"$WORKSPACE_ROOT/certwatch/CHANGELOG.md"
  commit_all "docs(certwatch): extend changelog"
}

teardown() {
  remove_workspace
}

@test "split-project prints help" {
  run "$WORKSPACE_ROOT/scripts/split-project.sh" --help
  [ "$status" -eq 0 ]
  [[ "$output" == *"Usage: scripts/split-project.sh"* ]]
}

@test "split-project creates a standalone repository with the project history" {
  run "$WORKSPACE_ROOT/scripts/split-project.sh" --output "$TEST_ROOT/standalone" certwatch
  [ "$status" -eq 0 ]
  [ -f "$TEST_ROOT/standalone/README.md" ]
  [ -f "$TEST_ROOT/standalone/cmd/certwatch/metadata.txt" ]
  [ ! -e "$TEST_ROOT/standalone/certwatch" ]
  [ ! -e "$TEST_ROOT/standalone/scripts/projects.tsv" ]
  run git -C "$TEST_ROOT/standalone" log --format=%s
  [ "${lines[0]}" = "ci: restore standalone workflows and repository links" ]
  [ "${lines[1]}" = "docs(certwatch): extend changelog" ]
  [ "${lines[2]}" = "feat(certwatch): create project from template" ]
  [ "${#lines[@]}" -eq 3 ]
  run git -C "$TEST_ROOT/standalone" rev-parse --abbrev-ref HEAD
  [ "$output" = "main" ]
  run git -C "$TEST_ROOT/standalone" status --porcelain
  [ -z "$output" ]
  run git -C "$TEST_ROOT/standalone" remote
  [ -z "$output" ]
}

@test "split-project restores standalone workflows with plain tags" {
  "$WORKSPACE_ROOT/scripts/split-project.sh" --output "$TEST_ROOT/standalone" certwatch
  local ci="$TEST_ROOT/standalone/.github/workflows/ci.yml"
  local release="$TEST_ROOT/standalone/.github/workflows/release.yml"
  [ -f "$ci" ]
  [ -f "$release" ]
  grep -q "working-directory: \." "$ci"
  grep -q "'v\*'" "$release"
  refute grep -q "certwatch/v" "$release"
}

@test "split-project rewrites repository links and tag references" {
  "$WORKSPACE_ROOT/scripts/split-project.sh" --output "$TEST_ROOT/standalone" certwatch
  grep -q "^Repository: https://github.com/unalivin/certwatch$" "$TEST_ROOT/standalone/README.md"
  grep -q "^Release tag: v0.1.0-dev$" "$TEST_ROOT/standalone/README.md"
  refute grep -rq "unalivin/workspace" "$TEST_ROOT/standalone" --exclude-dir=.git
}

@test "split-project leaves the monorepo untouched" {
  local head branches
  head="$(git -C "$WORKSPACE_ROOT" rev-parse HEAD)"
  branches="$(git -C "$WORKSPACE_ROOT" for-each-ref --format='%(refname)')"
  "$WORKSPACE_ROOT/scripts/split-project.sh" --output "$TEST_ROOT/standalone" certwatch
  [ "$(git -C "$WORKSPACE_ROOT" rev-parse HEAD)" = "$head" ]
  [ "$(git -C "$WORKSPACE_ROOT" for-each-ref --format='%(refname)')" = "$branches" ]
  [ -z "$(git -C "$WORKSPACE_ROOT" status --porcelain)" ]
  grep -q "unalivin/workspace" "$WORKSPACE_ROOT/certwatch/README.md"
}

@test "split-project dry run creates nothing" {
  run "$WORKSPACE_ROOT/scripts/split-project.sh" --dry-run --output "$TEST_ROOT/standalone" certwatch
  [ "$status" -eq 0 ]
  [[ "$output" == *"dry-run: would split certwatch"* ]]
  [ ! -e "$TEST_ROOT/standalone" ]
}

@test "split-project rejects bad input" {
  run "$WORKSPACE_ROOT/scripts/split-project.sh"
  [ "$status" -ne 0 ]
  run "$WORKSPACE_ROOT/scripts/split-project.sh" nope
  [ "$status" -ne 0 ]
  [[ "$output" == *"unknown project"* ]]
  run "$WORKSPACE_ROOT/scripts/split-project.sh" netpulse
  [ "$status" -ne 0 ]
  [[ "$output" == *"does not exist"* ]]
  mkdir "$TEST_ROOT/taken"
  run "$WORKSPACE_ROOT/scripts/split-project.sh" --output "$TEST_ROOT/taken" certwatch
  [ "$status" -ne 0 ]
  [[ "$output" == *"output already exists"* ]]
  printf 'dirty\n' >>"$WORKSPACE_ROOT/certwatch/README.md"
  run "$WORKSPACE_ROOT/scripts/split-project.sh" --output "$TEST_ROOT/standalone" certwatch
  [ "$status" -ne 0 ]
  [[ "$output" == *"uncommitted changes"* ]]
}
