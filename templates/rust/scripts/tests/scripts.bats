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
  for name in check demo dev-setup fuzz package publish-github release-verify sanitize; do
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

@test "package --dry-run prints the release build and creates nothing" {
  copy_project_scripts "$SANDBOX/project"
  run "$SANDBOX/project/scripts/package.sh" --dry-run
  [ "$status" -eq 0 ]
  [[ "$output" == *"dry-run: cargo "*"--release"* ]]
  [[ "$output" == *"-app"* ]]
  [[ "$output" == *"packaged "* ]]
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

@test "fuzz --list names every fuzz target" {
  run "$REAL_PROJECT/scripts/fuzz.sh" --list
  [ "$status" -eq 0 ]
  [[ "$output" == *"settings_normalize"* ]]
  count="$(find "$REAL_PROJECT/fuzz/fuzz_targets" -name '*.rs' | wc -l)"
  [ "${#lines[@]}" -eq "$count" ]
  for name in "${lines[@]}"; do
    grep -q "name = \"$name\"" "$REAL_PROJECT/fuzz/Cargo.toml"
  done
}

@test "fuzz --dry-run prints one cargo fuzz command per target" {
  run "$REAL_PROJECT/scripts/fuzz.sh" --dry-run --time 2m
  [ "$status" -eq 0 ]
  [[ "$output" == *"cargo fuzz run settings_normalize -- -max_total_time=120"* ]]
  count="$(find "$REAL_PROJECT/fuzz/fuzz_targets" -name '*.rs' | wc -l)"
  [ "${#lines[@]}" -eq "$count" ]
}

@test "fuzz rejects a duration it cannot read" {
  run "$REAL_PROJECT/scripts/fuzz.sh" --dry-run --time soon
  [ "$status" -ne 0 ]
  [[ "$output" == *"5s or 2m"* ]]
}

@test "sanitize --dry-run prints the test command of the no-GUI build" {
  run "$REAL_PROJECT/scripts/sanitize.sh" --dry-run
  [ "$status" -eq 0 ]
  [[ "$output" == *"cargo test --locked"* ]]
  [[ "$output" == *"--no-default-features"* ]]
  [[ "$output" != *"-gui"* ]]
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
    printf 'test first::tests::works ... ok\n'
    printf 'test widget::tests::adds ... FAILED\n'
    printf "thread 'widget::tests::adds' panicked at crates/app/src/widget.rs:12:5:\n"
    printf 'assertion failed: got 3, want 4\n'
    for number in $(seq 1 40); do
      printf 'test example::case%s ... ok\n' "$number"
    done
  } >"$SANDBOX/long.log"
  GITHUB_ACTIONS=true run "$REAL_PROJECT/scripts/ci-step.sh" --command "long test" bash -c "cat '$SANDBOX/long.log'; exit 1"
  [ "$status" -eq 1 ]
  annotation="${output##*::error}"
  [[ "$annotation" == *"widget::tests::adds ... FAILED"*"widget.rs:12:5"*"got 3, want 4"*"...%0A"*"example::case40"* ]]
  [[ "$annotation" != *"example::case20"* ]]
}

@test "ci-step shows a compiler error with its location" {
  {
    printf '   Compiling example v0.1.0\n'
    printf 'error[E0308]: mismatched types\n'
    printf '  --> crates/app/src/main.rs:7:9\n'
    for number in $(seq 1 40); do
      printf 'note line %s\n' "$number"
    done
  } >"$SANDBOX/build.log"
  GITHUB_ACTIONS=true run "$REAL_PROJECT/scripts/ci-step.sh" --command "build" bash -c "cat '$SANDBOX/build.log'; exit 101"
  [ "$status" -eq 101 ]
  [[ "$output" == *"::error title="*"build failed"*"::error[E0308]: mismatched types%0A  --> crates/app/src/main.rs:7:9"* ]]
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
  SETUP_ALL_TOOLS=1 run "$REAL_PROJECT/scripts/dev-setup.sh" --install --dry-run
  [ "$status" -eq 0 ]
  [[ "$output" == *"rustup toolchain install $(project_value RUST_VERSION) "* ]]
  [[ "$output" == *"cargo install --locked cargo-audit --version $(project_value CARGO_AUDIT_VERSION)"* ]]
  [[ "$output" == *"cargo install --locked cargo-deny --version $(project_value CARGO_DENY_VERSION)"* ]]
  [[ "$output" == *"cargo install --locked cargo-fuzz --version $(project_value CARGO_FUZZ_VERSION)"* ]]
  [[ "$output" == *"cargo install --locked cargo-bundle --version $(project_value CARGO_BUNDLE_VERSION)"* ]]
}

@test "the script constants match the application metadata" {
  meta="$REAL_PROJECT/crates/$(project_value APP_SLUG)-core/src/meta.rs"
  constant() {
    sed -n "s/^pub const $1: &str = \"\\(.*\\)\";\$/\\1/p" "$meta"
  }
  [ "$(project_value APP_SLUG)" = "$(constant SLUG)" ]
  [ "$(project_value APP_NAME)" = "$(constant APP_NAME)" ]
  [ "$(project_value APP_ID)" = "$(constant APP_ID)" ]
  [ "$(project_value APP_REPOSITORY_URL)" = "$(constant REPOSITORY_URL)" ]
  [ "$(project_value APP_VERSION)" = "$(sed -n 's/^version = "\(.*\)"$/\1/p' "$REAL_PROJECT/Cargo.toml" | head -n 1)" ]
  [ "$(project_value APP_NUMERIC_VERSION)" = "$(project_value APP_VERSION | sed 's/-.*//')" ]
  [ "$(project_value RUST_VERSION)" = "$(sed -n 's/^channel = "\(.*\)"$/\1/p' "$REAL_PROJECT/rust-toolchain.toml")" ]
  grep -q "^name = \"$(project_value APP_NAME)\"$" "$REAL_PROJECT/crates/$(project_value APP_SLUG)-app/Cargo.toml"
  grep -q "^identifier = \"$(project_value APP_ID)\"$" "$REAL_PROJECT/crates/$(project_value APP_SLUG)-app/Cargo.toml"
}

@test "the environment example lists only variables the documentation explains" {
  while IFS='=' read -r name _; do
    [ -n "$name" ] || continue
    grep -q "$name" "$REAL_PROJECT/docs/COMMANDS.md"
  done <"$REAL_PROJECT/.env.example"
}

@test "package builds the Linux archive from an existing binary" {
  [ "$(uname -s)" = "Linux" ] || skip "the archive layout under test is the Linux one"
  slug="$(project_value APP_SLUG)"
  id="$(project_value APP_ID)"
  mkdir -p "$SANDBOX/bin"
  printf '#!/bin/sh\nexit 0\n' >"$SANDBOX/bin/$slug"
  chmod +x "$SANDBOX/bin/$slug"
  run "$REAL_PROJECT/scripts/package.sh" --executable "$SANDBOX/bin/$slug" --output "$SANDBOX/out"
  [ "$status" -eq 0 ]
  [ "$(find "$SANDBOX/out" -mindepth 1 | wc -l)" -eq 1 ]
  archive="$(find "$SANDBOX/out" -name "$slug-linux-*.tar.gz")"
  [ -s "$archive" ]
  listing="$(tar -tzf "$archive")"
  for entry in LICENSE THIRD_PARTY_NOTICES.md USAGE.md README.txt Makefile "usr/local/bin/$slug" \
    "usr/local/share/applications/$id.desktop" "usr/local/share/pixmaps/$id.png"; do
    [[ "$listing" == *"/$entry"* ]]
  done
}

@test "the Linux archive installs into a user prefix and removes itself again" {
  [ "$(uname -s)" = "Linux" ] || skip "the archive layout under test is the Linux one"
  slug="$(project_value APP_SLUG)"
  id="$(project_value APP_ID)"
  mkdir -p "$SANDBOX/bin" "$SANDBOX/unpacked"
  printf '#!/bin/sh\nexit 0\n' >"$SANDBOX/bin/$slug"
  chmod +x "$SANDBOX/bin/$slug"
  run "$REAL_PROJECT/scripts/package.sh" --executable "$SANDBOX/bin/$slug" --output "$SANDBOX/out"
  [ "$status" -eq 0 ]
  tar -xzf "$SANDBOX/out/$slug-linux-"*.tar.gz -C "$SANDBOX/unpacked" --strip-components=1
  run make -C "$SANDBOX/unpacked" user-install USER_PREFIX="$SANDBOX/prefix"
  [ "$status" -eq 0 ]
  [ -x "$SANDBOX/prefix/bin/$slug" ]
  grep -q "^Exec=$SANDBOX/prefix/bin/$slug$" "$SANDBOX/prefix/share/applications/$id.desktop"
  grep -q "^Icon=$SANDBOX/prefix/share/icons/$id.png$" "$SANDBOX/prefix/share/applications/$id.desktop"
  [ -s "$SANDBOX/prefix/share/icons/$id.png" ]
  run make -C "$SANDBOX/unpacked" user-uninstall USER_PREFIX="$SANDBOX/prefix"
  [ "$status" -eq 0 ]
  [ -z "$(find "$SANDBOX/prefix" -type f)" ]
}

@test "package refuses a binary with another name" {
  [ "$(uname -s)" = "Linux" ] || skip "--executable is supported on Linux only"
  printf 'x\n' >"$SANDBOX/other"
  run "$REAL_PROJECT/scripts/package.sh" --executable "$SANDBOX/other" --dry-run
  [ "$status" -ne 0 ]
  [[ "$output" == *"must be named"* ]]
}
