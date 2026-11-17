#!/usr/bin/env bash
set -uo pipefail

cd secretsentry || exit 1
export CARGO_TERM_COLOR=never
packages=(-p secretsentry-core -p secretsentry-tui -p secretsentry-app)

step() {
  local name="$1"
  shift
  "$@" >"$EVIDENCE_DIR/$name.log" 2>&1
  printf '%s exit status: %s\n' "$name" "$?"
}

step toolchain rustup toolchain install 1.100.0 --profile minimal --component rustfmt --component clippy
step lock cargo update --workspace
step fuzz-lock cargo update --workspace --manifest-path fuzz/Cargo.toml
step fmt cargo fmt --all
step fuzz-fmt cargo fmt --all --manifest-path fuzz/Cargo.toml
step clippy cargo clippy "${packages[@]}" --no-default-features --all-targets -- -D warnings
step snapshot env UPDATE_SNAPSHOT=1 cargo test -p secretsentry-core schema_matches
step test cargo test "${packages[@]}" --no-default-features
step clippy-gui cargo clippy -p secretsentry-gui --all-targets -- -D warnings
step test-gui cargo test -p secretsentry-gui
step fuzz-check cargo check --manifest-path fuzz/Cargo.toml
step notices make notices LOCKED=
step docs make docs LOCKED=
grep -hE "^test .* FAILED|^error|panicked|^test result|warning: unused" "$EVIDENCE_DIR/test.log" "$EVIDENCE_DIR/clippy.log" "$EVIDENCE_DIR/clippy-gui.log" "$EVIDENCE_DIR/test-gui.log" >"$EVIDENCE_DIR/summary.txt" 2>&1
git status --short >"$EVIDENCE_DIR/status.txt"
git diff --binary >"$EVIDENCE_DIR/changes.patch"
