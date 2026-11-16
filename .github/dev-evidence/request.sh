#!/usr/bin/env bash
set -uo pipefail

cd templates/rust
rustup toolchain install 1.100.0 --profile minimal --component rustfmt --component clippy
rustc --version
cargo --version
cargo generate-lockfile
cargo fmt --all
cargo check --workspace --all-targets --message-format short >"$EVIDENCE_DIR/check.short.log" 2>&1
printf 'check exit status: %s\n' "$?"
cargo check --workspace --all-targets >"$EVIDENCE_DIR/check.full.log" 2>&1
cargo clippy --workspace --all-targets --message-format short -- -D warnings >"$EVIDENCE_DIR/clippy.short.log" 2>&1
printf 'clippy exit status: %s\n' "$?"
cargo clippy --workspace --all-targets -- -D warnings >"$EVIDENCE_DIR/clippy.full.log" 2>&1
: >"$EVIDENCE_DIR/test.log"
for package in templateapp-core templateapp-tui templateapp-gui templateapp-app; do
  cargo test -p "$package" --no-fail-fast >>"$EVIDENCE_DIR/test.log" 2>&1
  printf 'test %s exit status: %s\n' "$package" "$?"
done
cargo test -p templateapp-app --no-default-features >>"$EVIDENCE_DIR/test.log" 2>&1
printf 'test without the dashboard exit status: %s\n' "$?"
cargo run -q -p templateapp-gui --example screenshots -- -out "$EVIDENCE_DIR/screens" >"$EVIDENCE_DIR/screens.log" 2>&1
printf 'screenshots exit status: %s\n' "$?"
mkdir -p "$EVIDENCE_DIR/tree"
cargo run -q -p templateapp-core --example genassets -- -out "$EVIDENCE_DIR/tree/assets"
printf 'genassets exit status: %s\n' "$?"
cp Cargo.lock "$EVIDENCE_DIR/tree/" || true
find crates -name '*.rs' -print0 | while IFS= read -r -d '' file; do
  mkdir -p "$EVIDENCE_DIR/tree/$(dirname "$file")"
  cp "$file" "$EVIDENCE_DIR/tree/$file"
done
