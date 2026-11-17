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

@test "gen-workflows creates a missing project pull request template and copies it to the root" {
  make_fake_template go
  "$WORKSPACE_ROOT/scripts/new-project.sh" certwatch go >/dev/null
  rm "$WORKSPACE_ROOT/certwatch/.github/PULL_REQUEST_TEMPLATE.md" "$WORKSPACE_ROOT/.github/PULL_REQUEST_TEMPLATE/certwatch.md"
  run "$WORKSPACE_ROOT/scripts/gen-workflows.sh" --check
  [ "$status" -eq 1 ]
  [[ "$output" == *"missing: certwatch/.github/PULL_REQUEST_TEMPLATE.md"* ]]
  [ ! -e "$WORKSPACE_ROOT/certwatch/.github/PULL_REQUEST_TEMPLATE.md" ]
  run "$WORKSPACE_ROOT/scripts/gen-workflows.sh"
  [ "$status" -eq 0 ]
  grep -q "The title uses the scope \`certwatch\`" "$WORKSPACE_ROOT/certwatch/.github/PULL_REQUEST_TEMPLATE.md"
  grep -q "\`make check\` passes for CertWatch" "$WORKSPACE_ROOT/certwatch/.github/PULL_REQUEST_TEMPLATE.md"
  cmp "$WORKSPACE_ROOT/certwatch/.github/PULL_REQUEST_TEMPLATE.md" "$WORKSPACE_ROOT/.github/PULL_REQUEST_TEMPLATE/certwatch.md"
  run "$WORKSPACE_ROOT/scripts/gen-workflows.sh" --check
  [ "$status" -eq 0 ]
}

@test "gen-workflows keeps a project's own edits and carries them to the root copy" {
  make_fake_template go
  "$WORKSPACE_ROOT/scripts/new-project.sh" certwatch go >/dev/null
  printf -- '- [ ] Only the TLS package opens connections\n' >>"$WORKSPACE_ROOT/certwatch/.github/PULL_REQUEST_TEMPLATE.md"
  run "$WORKSPACE_ROOT/scripts/gen-workflows.sh" --check --only pull-requests
  [ "$status" -eq 1 ]
  [[ "$output" == *"out of date: .github/PULL_REQUEST_TEMPLATE/certwatch.md"* ]]
  run "$WORKSPACE_ROOT/scripts/gen-workflows.sh" --check --only workflows
  [ "$status" -eq 0 ]
  "$WORKSPACE_ROOT/scripts/gen-workflows.sh" >/dev/null
  grep -q "Only the TLS package opens connections" "$WORKSPACE_ROOT/.github/PULL_REQUEST_TEMPLATE/certwatch.md"
  grep -q "Only the TLS package opens connections" "$WORKSPACE_ROOT/certwatch/.github/PULL_REQUEST_TEMPLATE.md"
}

@test "gen-workflows removes root copies without a project and lists templates in the root default" {
  make_fake_template go
  run grep -c "No project folder exists yet" "$WORKSPACE_ROOT/.github/PULL_REQUEST_TEMPLATE.md"
  [ "$output" = "1" ]
  "$WORKSPACE_ROOT/scripts/new-project.sh" certwatch go >/dev/null
  "$WORKSPACE_ROOT/scripts/new-project.sh" netauto go >/dev/null
  grep -q "^- \`certwatch\` (CertWatch): \`?template=certwatch.md\`$" "$WORKSPACE_ROOT/.github/PULL_REQUEST_TEMPLATE.md"
  grep -q "^- \`netauto\` (NetAuto): \`?template=netauto.md\`$" "$WORKSPACE_ROOT/.github/PULL_REQUEST_TEMPLATE.md"
  refute grep -q "__PROJECT_TEMPLATES__\|No project folder exists yet" "$WORKSPACE_ROOT/.github/PULL_REQUEST_TEMPLATE.md"
  printf 'stale\n' >"$WORKSPACE_ROOT/.github/PULL_REQUEST_TEMPLATE/gone.md"
  run "$WORKSPACE_ROOT/scripts/gen-workflows.sh" --check
  [ "$status" -eq 1 ]
  [[ "$output" == *"not generated: .github/PULL_REQUEST_TEMPLATE/gone.md"* ]]
  "$WORKSPACE_ROOT/scripts/gen-workflows.sh" >/dev/null
  [ ! -e "$WORKSPACE_ROOT/.github/PULL_REQUEST_TEMPLATE/gone.md" ]
  printf 'edited by hand\n' >>"$WORKSPACE_ROOT/.github/PULL_REQUEST_TEMPLATE.md"
  run "$WORKSPACE_ROOT/scripts/gen-workflows.sh" --check
  [ "$status" -eq 1 ]
  [[ "$output" == *"out of date: .github/PULL_REQUEST_TEMPLATE.md"* ]]
}

@test "gen-workflows rejects an unknown --only value" {
  run "$WORKSPACE_ROOT/scripts/gen-workflows.sh" --only everything
  [ "$status" -ne 0 ]
  [[ "$output" == *"unknown --only value"* ]]
}
