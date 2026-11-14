#!/usr/bin/env bats

load helpers

setup() {
  make_workspace
}

teardown() {
  remove_workspace
}

@test "gen-workflows prints help" {
  run "$WORKSPACE_ROOT/scripts/gen-workflows.sh" --help
  [ "$status" -eq 0 ]
  [[ "$output" == *"Usage: scripts/gen-workflows.sh"* ]]
}

@test "gen-workflows writes the root workflow and reports up to date afterwards" {
  [ -f "$WORKSPACE_ROOT/.github/workflows/root-ci.yml" ]
  run "$WORKSPACE_ROOT/scripts/gen-workflows.sh"
  [ "$status" -eq 0 ]
  [[ "$output" == *"up to date"* ]]
}

@test "gen-workflows writes one CI workflow per template with a path filter" {
  make_fake_template go
  run "$WORKSPACE_ROOT/scripts/gen-workflows.sh"
  [ "$status" -eq 0 ]
  local file="$WORKSPACE_ROOT/.github/workflows/template-go-ci.yml"
  [ -f "$file" ]
  grep -q "templates/go/\*\*" "$file"
  grep -q "name: CI template-go" "$file"
  [ ! -f "$WORKSPACE_ROOT/.github/workflows/template-go-release.yml" ]
}

@test "gen-workflows writes CI and release workflows per project" {
  make_fake_template go
  "$WORKSPACE_ROOT/scripts/new-project.sh" certwatch go
  local ci="$WORKSPACE_ROOT/.github/workflows/certwatch-ci.yml"
  local release="$WORKSPACE_ROOT/.github/workflows/certwatch-release.yml"
  [ -f "$ci" ]
  [ -f "$release" ]
  grep -q "'certwatch/\*\*'" "$ci"
  grep -q "working-directory: certwatch" "$ci"
  grep -q "name: Release certwatch" "$release"
  grep -q "'certwatch/v\*'" "$release"
  grep -qF "CertWatch \${version} (dev beta)" "$release"
  grep -q "certwatch-linux-x64.tar.gz" "$release"
  grep -q "certwatch-macos-arm64.tar.gz" "$release"
  grep -q "certwatch-windows-x64.exe" "$release"
  refute grep -q "workflow_dispatch" "$release"
  refute grep -q "__" "$ci"
}

@test "gen-workflows --check detects drift and stale files" {
  make_fake_template go
  "$WORKSPACE_ROOT/scripts/gen-workflows.sh"
  run "$WORKSPACE_ROOT/scripts/gen-workflows.sh" --check
  [ "$status" -eq 0 ]
  printf 'drift: true\n' >>"$WORKSPACE_ROOT/.github/workflows/template-go-ci.yml"
  run "$WORKSPACE_ROOT/scripts/gen-workflows.sh" --check
  [ "$status" -eq 1 ]
  [[ "$output" == *"out of date: .github/workflows/template-go-ci.yml"* ]]
  "$WORKSPACE_ROOT/scripts/gen-workflows.sh"
  printf 'name: stale\n' >"$WORKSPACE_ROOT/.github/workflows/stale.yml"
  run "$WORKSPACE_ROOT/scripts/gen-workflows.sh" --check
  [ "$status" -eq 1 ]
  [[ "$output" == *"not generated: .github/workflows/stale.yml"* ]]
  [ -f "$WORKSPACE_ROOT/.github/workflows/stale.yml" ]
  run "$WORKSPACE_ROOT/scripts/gen-workflows.sh"
  [ "$status" -eq 0 ]
  [ ! -f "$WORKSPACE_ROOT/.github/workflows/stale.yml" ]
}

@test "gen-workflows dry run writes nothing" {
  make_fake_template go
  run "$WORKSPACE_ROOT/scripts/gen-workflows.sh" --dry-run
  [ "$status" -eq 0 ]
  [[ "$output" == *"dry-run: would write .github/workflows/template-go-ci.yml"* ]]
  [ ! -f "$WORKSPACE_ROOT/.github/workflows/template-go-ci.yml" ]
}

@test "gen-workflows standalone output uses plain tags and the repository root" {
  make_fake_template go
  run "$WORKSPACE_ROOT/scripts/gen-workflows.sh" --standalone certwatch --output "$TEST_ROOT/standalone"
  [ "$status" -eq 0 ]
  grep -q "working-directory: \." "$TEST_ROOT/standalone/ci.yml"
  grep -q "'v\*'" "$TEST_ROOT/standalone/release.yml"
  refute grep -q "certwatch/v" "$TEST_ROOT/standalone/release.yml"
  grep -q "name: Release certwatch" "$TEST_ROOT/standalone/release.yml"
}

@test "gen-workflows rejects unknown projects and unresolved tokens" {
  run "$WORKSPACE_ROOT/scripts/gen-workflows.sh" --standalone nope --output "$TEST_ROOT/standalone"
  [ "$status" -ne 0 ]
  [[ "$output" == *"unknown project"* ]]
  make_fake_template go
  printf 'broken: __UNKNOWN_TOKEN__\n' >>"$WORKSPACE_ROOT/templates/workflows/go-ci.yml"
  run "$WORKSPACE_ROOT/scripts/gen-workflows.sh"
  [ "$status" -ne 0 ]
  [[ "$output" == *"unresolved token"* ]]
}
