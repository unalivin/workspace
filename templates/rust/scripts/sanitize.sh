#!/usr/bin/env bash
set -euo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

usage() {
  cat <<USAGE
Usage: scripts/sanitize.sh [--dry-run]

Runs the tests of the no-GUI build with runtime checks that the normal test
run does not have.

  Linux, macOS  AddressSanitizer on the Rust code, including the unsafe code
                of the dependencies. The SQLite C code that is compiled in is
                not instrumented, but its allocations go through the checked
                allocator. The pinned stable compiler ($RUST_VERSION) is used
                with RUSTC_BOOTSTRAP=1, which unlocks the sanitizer flag
                without a nightly toolchain. Leak detection is on where the
                platform supports it (Linux).
  Windows       The sanitizer runtime is not available for this compiler, so
                the tests run as an optimized build with overflow checks and
                debug assertions turned on.

The sanitized build has its own directory under target/, so it does not
disturb the normal build. When llvm-symbolizer is installed, reports name the
functions and lines.

  --dry-run  Print the commands instead of running them
  --help     Show this help
USAGE
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --dry-run) DRY_RUN=1 ;;
    --help | -h)
      usage
      exit 0
      ;;
    *) die "unknown argument: $1 (see --help)" ;;
  esac
  shift
done

cd "$PROJECT_ROOT"
packages=()
for package in $NOGUI_PACKAGES; do
  packages+=(-p "$package")
done

case "$(host_os)" in
  linux | macos)
    triple="$(host_triple)"
    if [ -z "$triple" ]; then
      [ "$DRY_RUN" = "1" ] || die "rustc is not installed: run scripts/dev-setup.sh"
      triple="<host-triple>"
    fi
    settings=(RUSTC_BOOTSTRAP=1 RUSTFLAGS="-Zsanitizer=address")
    symbolizer="$(command -v llvm-symbolizer || true)"
    if [ -z "$symbolizer" ]; then
      symbolizer="$(find /usr/bin -maxdepth 1 -name 'llvm-symbolizer-*' 2>/dev/null | sort -V | tail -n 1)"
    fi
    if [ -n "$symbolizer" ]; then
      settings+=(ASAN_SYMBOLIZER_PATH="$symbolizer")
    fi
    run env "${settings[@]}" \
      cargo test --locked "${packages[@]}" --no-default-features --lib --bins --tests --no-fail-fast --target "$triple"
    ;;
  windows)
    run env RUSTFLAGS="-C overflow-checks=on -C debug-assertions=on" CARGO_TARGET_DIR="target/checked" \
      cargo test --locked --release "${packages[@]}" --no-default-features --no-fail-fast
    ;;
  *) die "unsupported operating system: $(uname -s)" ;;
esac
