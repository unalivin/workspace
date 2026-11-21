.PHONY: fmt fmt-check lint test race-or-sanitize build build-nogui check clean

fmt:
	cargo fmt --all

fmt-check:
	cargo fmt --all -- --check

lint:
	cargo clippy --workspace --all-targets -- -D warnings

test:
	cargo test --workspace --no-default-features
	cargo test --workspace

race-or-sanitize:
	cargo test --workspace --no-default-features

build:
	cargo build --workspace --release

build-nogui:
	cargo build --workspace --no-default-features --release

check: fmt-check lint test

clean:
	cargo clean
