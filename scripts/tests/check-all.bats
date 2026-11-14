#!/usr/bin/env bats

load helpers

setup() {
  make_workspace
}

teardown() {
  remove_workspace
}

@test "check-all prints help" {
  run "$WORKSPACE_ROOT/scripts/check-all.sh" --help
  [ "$status" -eq 0 ]
  [[ "$output" == *"Usage: scripts/check-all.sh"* ]]
}

@test "check-all with nothing to check succeeds and says so" {
  run "$WORKSPACE_ROOT/scripts/check-all.sh"
  [ "$status" -eq 0 ]
  [[ "$output" == *"nothing to check"* ]]
}

@test "check-all runs make check in every template and project" {
  make_fake_template go
  "$WORKSPACE_ROOT/scripts/new-project.sh" certwatch go
  run "$WORKSPACE_ROOT/scripts/check-all.sh"
  [ "$status" -eq 0 ]
  [[ "$output" == *"==> templates/go"* ]]
  [[ "$output" == *"==> certwatch"* ]]
  [[ "$output" == *"checked certwatch"* ]]
  [[ "$output" == *"passed: 2"* ]]
  [[ "$output" == *"failed: 0"* ]]
}

@test "check-all lists targets without running them" {
  make_fake_template go
  run "$WORKSPACE_ROOT/scripts/check-all.sh" --list
  [ "$status" -eq 0 ]
  [ "$output" = "templates/go" ]
}

@test "check-all reports a failing target and continues" {
  make_fake_template go
  make_fake_template rust
  touch "$WORKSPACE_ROOT/templates/go/fail-marker"
  run "$WORKSPACE_ROOT/scripts/check-all.sh"
  [ "$status" -eq 1 ]
  [[ "$output" == *"passed: 1"* ]]
  [[ "$output" == *"failed: 1"* ]]
  [[ "$output" == *"failed target: templates/go"* ]]
}

@test "check-all stops at the first failure with --fail-fast" {
  make_fake_template go
  make_fake_template rust
  touch "$WORKSPACE_ROOT/templates/go/fail-marker"
  run "$WORKSPACE_ROOT/scripts/check-all.sh" --fail-fast
  [ "$status" -eq 1 ]
  [[ "$output" != *"==> templates/rust"* ]]
}

@test "check-all prints error annotations in GitHub Actions" {
  make_fake_template go
  touch "$WORKSPACE_ROOT/templates/go/fail-marker"
  GITHUB_ACTIONS=true run "$WORKSPACE_ROOT/scripts/check-all.sh"
  [ "$status" -eq 1 ]
  [[ "$output" == *"::error title=check-all::make check failed in templates/go"* ]]
}

@test "check-all dry run does not execute make" {
  make_fake_template go
  touch "$WORKSPACE_ROOT/templates/go/fail-marker"
  run "$WORKSPACE_ROOT/scripts/check-all.sh" --dry-run
  [ "$status" -eq 0 ]
  [[ "$output" == *"dry-run: make -C"* ]]
}

@test "check-all rejects unknown targets and paths outside the workspace" {
  run "$WORKSPACE_ROOT/scripts/check-all.sh" missing-project
  [ "$status" -ne 0 ]
  [[ "$output" == *"no Makefile"* ]]
  run "$WORKSPACE_ROOT/scripts/check-all.sh" ../elsewhere
  [ "$status" -ne 0 ]
  [[ "$output" == *"inside the workspace"* ]]
}
