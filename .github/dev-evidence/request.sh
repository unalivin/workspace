#!/usr/bin/env bash
set -uo pipefail

cd templates/rust
rustup toolchain install 1.100.0 --profile minimal --component rustfmt --component clippy
rustc --version
cargo --version
cargo generate-lockfile
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
mkdir -p "$EVIDENCE_DIR/tree"
cp Cargo.lock "$EVIDENCE_DIR/tree/" || true
find crates -name '*.rs' -print0 | while IFS= read -r -d '' file; do
  mkdir -p "$EVIDENCE_DIR/tree/$(dirname "$file")"
  cp "$file" "$EVIDENCE_DIR/tree/$file"
done
