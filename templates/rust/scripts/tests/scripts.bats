#!/usr/bin/env bats

load helpers

setup() {
  make_sandbox
}

teardown() {
  remove_sandbox
}

@test "every script prints usage with --help" {
  for script in "$REAL_PROJECT"/scripts/*.sh; do
    [ "$(basename "$script")" != "lib.sh" ] || continue
    run "$script" --help
    [ "$status" -eq 0 ]
    [[ "$output" == Usage:* ]]
  done
}

@test "every script starts with the bash shebang and strict mode" {
  for script in "$REAL_PROJECT"/scripts/*.sh; do
    [ "$(sed -n 1p "$script")" = "#!/usr/bin/env bash" ]
    [ "$(sed -n 2p "$script")" = "set -euo pipefail" ]
    [ -x "$script" ]
  done
}

@test "every script rejects an unknown option" {
  for name in check demo dev-setup fuzz package publish-github release-verify; do
    run "$REAL_PROJECT/scripts/$name.sh" --no-such-option
    [ "$status" -ne 0 ]
    [[ "$output" == *"unknown argument"* ]]
  done
}

@test "every required make target has a script or a recipe" {
  for target in fmt fmt-check lint test race-or-sanitize build build-nogui check clean package demo; do
    grep -Eq "^${target}:" "$REAL_PROJECT/Makefile"
  done
}

@test "package --dry-run prints the pinned packaging command and creates nothing" {
  copy_project_scripts "$SANDBOX/project"
  run "$SANDBOX/project/scripts/package.sh" --dry-run
  [ "$status" -eq 0 ]
  [[ "$output" == *"fyne package --target"* ]]
  [[ "$output" == *"--app-version 0.1.0"* ]]
  [[ "$output" == *"no_animations"* ]]
  [ ! -e "$SANDBOX/project/dist" ]
}

@test "demo --dry-run uses a scratch database and config directory" {
  copy_project_scripts "$SANDBOX/project"
  run "$SANDBOX/project/scripts/demo.sh" --dry-run --ui none
  [ "$status" -eq 0 ]
  [[ "$output" == *"demo load"* ]]
  [[ "$output" == *"db init"* ]]
  [[ "$output" != *"--ui"* ]]
  [ ! -e "$SANDBOX/project/dist" ]
}

@test "demo rejects an unknown front end" {
  run "$REAL_PROJECT/scripts/demo.sh" --dry-run --ui web
  [ "$status" -ne 0 ]
  [[ "$output" == *"gui, tui or none"* ]]
}

@test "check --dry-run lists the suite without running it" {
  copy_project_scripts "$SANDBOX/project"
  run "$SANDBOX/project/scripts/check.sh" --dry-run
  [ "$status" -eq 0 ]
  [[ "$output" == *"make --no-print-directory fmt-check"* ]]
  [[ "$output" == *"make --no-print-directory test-nogui"* ]]
  [[ "$output" == *"make --no-print-directory race-or-sanitize"* ]]
  [[ "$output" == *"make --no-print-directory notices-check"* ]]
  [[ "$output" == *"failed:  0"* ]]
}

@test "fuzz --list names every fuzz target with its package" {
  run "$REAL_PROJECT/scripts/fuzz.sh" --list
  [ "$status" -eq 0 ]
  [[ "$output" == *"./internal/settings FuzzNormalize"* ]]
  count="$(grep -rE --include='*_test.go' -c '^func Fuzz' "$REAL_PROJECT/internal" "$REAL_PROJECT/tests" | awk -F: '{ total += $2 } END { print total }')"
  [ "${#lines[@]}" -eq "$count" ]
}

@test "fuzz --dry-run prints one go test command per target" {
  run "$REAL_PROJECT/scripts/fuzz.sh" --dry-run --time 1s
  [ "$status" -eq 0 ]
  [[ "$output" == *"-fuzztime 1s"* ]]
  [[ "$output" == *"FuzzNormalize"* ]]
}

@test "ci-step runs a command and passes its output through" {
  run "$REAL_PROJECT/scripts/ci-step.sh" --command "say hello" printf 'hello\n'
  [ "$status" -eq 0 ]
  [[ "$output" == *"==> say hello"* ]]
  [[ "$output" == *"hello"* ]]
}

@test "ci-step reports a failing command with its exit status and an annotation" {
  GITHUB_ACTIONS=true run "$REAL_PROJECT/scripts/ci-step.sh" --command "break things" bash -c 'printf "first line\nsecond 100%% line\n"; exit 7'
  [ "$status" -eq 7 ]
  [[ "$output" == *"step failed: break things (exit 7)"* ]]
  [[ "$output" == *"::error title="*"break things failed"*"::first line%0Asecond 100%25 line"* ]]
}

@test "ci-step puts the failing test before the last lines of a long log" {
  {
    printf 'ok  \texample/first\t0.1s\n'
    printf -- '--- FAIL: TestWidget (0.00s)\n'
    printf '    widget_test.go:12: got 3, want 4\n'
    for number in $(seq 1 40); do
      printf 'ok  \texample/package%s\t0.1s\n' "$number"
    done
  } >"$SANDBOX/long.log"
  GITHUB_ACTIONS=true run "$REAL_PROJECT/scripts/ci-step.sh" --command "long test" bash -c "cat '$SANDBOX/long.log'; exit 1"
  [ "$status" -eq 1 ]
  annotation="${output##*::error}"
  [[ "$annotation" == *"--- FAIL: TestWidget"*"widget_test.go:12: got 3, want 4"*"...%0A"*"example/package40"* ]]
  [[ "$annotation" != *"example/package20"* ]]
}

@test "ci-step prints no annotation outside GitHub Actions" {
  GITHUB_ACTIONS="" run "$REAL_PROJECT/scripts/ci-step.sh" --command "break things" false
  [ "$status" -eq 1 ]
  [[ "$output" != *"::error"* ]]
}

@test "ci-step without arguments is a usage error" {
  run "$REAL_PROJECT/scripts/ci-step.sh"
  [ "$status" -eq 2 ]
}

@test "dev-setup --install --dry-run prints the pinned tool versions" {
  command -v go >/dev/null 2>&1 || skip "go is not installed"
  GOBIN="$SANDBOX/gobin" PATH="$(dirname "$(command -v go)"):/usr/bin:/bin" run "$REAL_PROJECT/scripts/dev-setup.sh" --install --dry-run
  [ "$status" -eq 0 ]
  [[ "$output" == *"staticcheck@v"* ]]
  [[ "$output" == *"govulncheck@v"* ]]
  [[ "$output" == *"fyne.io/tools/cmd/fyne@v"* ]]
}

@test "the script constants match the application metadata" {
  meta="$REAL_PROJECT/internal/meta/meta.go"
  [ "$(project_value APP_SLUG)" = "$(sed -n 's/^[[:space:]]*Slug[[:space:]]*= "\(.*\)"$/\1/p' "$meta")" ]
  [ "$(project_value APP_NAME)" = "$(sed -n 's/^[[:space:]]*AppName[[:space:]]*= "\(.*\)"$/\1/p' "$meta")" ]
  [ "$(project_value APP_ID)" = "$(sed -n 's/^[[:space:]]*AppID[[:space:]]*= "\(.*\)"$/\1/p' "$meta")" ]
  [ "$(project_value APP_VERSION)" = "$(sed -n 's/^[[:space:]]*Version[[:space:]]*= "\(.*\)"$/\1/p' "$meta")" ]
  [ "$(project_value APP_REPOSITORY_URL)" = "$(sed -n 's/^[[:space:]]*RepositoryURL[[:space:]]*= "\(.*\)"$/\1/p' "$meta")" ]
  [ "$(project_value APP_NUMERIC_VERSION)" = "$(project_value APP_VERSION | sed 's/-.*//')" ]
  [ "$(project_value GO_VERSION)" = "$(sed -n 's/^go //p' "$REAL_PROJECT/go.mod")" ]
}

@test "the environment example lists only variables the documentation explains" {
  while IFS='=' read -r name _; do
    [ -n "$name" ] || continue
    grep -q "$name" "$REAL_PROJECT/docs/COMMANDS.md"
  done <"$REAL_PROJECT/.env.example"
}

@test "package builds the Linux archive from an existing binary" {
  [ "$(uname -s)" = "Linux" ] || skip "the archive layout under test is the Linux one"
  command -v fyne >/dev/null 2>&1 || skip "fyne is not installed"
  slug="$(project_value APP_SLUG)"
  mkdir -p "$SANDBOX/bin"
  printf '#!/bin/sh\nexit 0\n' >"$SANDBOX/bin/$slug"
  chmod +x "$SANDBOX/bin/$slug"
  run "$REAL_PROJECT/scripts/package.sh" --executable "$SANDBOX/bin/$slug" --output "$SANDBOX/out"
  [ "$status" -eq 0 ]
  [ "$(find "$SANDBOX/out" -mindepth 1 | wc -l)" -eq 1 ]
  archive="$(find "$SANDBOX/out" -name "$slug-linux-*.tar.gz")"
  [ -s "$archive" ]
  listing="$(tar -tzf "$archive")"
  for entry in LICENSE THIRD_PARTY_NOTICES.md USAGE.md README.txt Makefile "usr/local/bin/$slug"; do
    [[ "$listing" == *"/$entry"* ]]
  done
  [[ "$listing" == *".desktop"* ]]
  [ -z "$(find "$REAL_PROJECT" -maxdepth 2 -name '*.tar.xz')" ]
}

@test "package refuses a binary with another name" {
  printf 'x\n' >"$SANDBOX/other"
  run "$REAL_PROJECT/scripts/package.sh" --executable "$SANDBOX/other" --dry-run
  [ "$status" -ne 0 ]
  [[ "$output" == *"must be named"* ]]
}
