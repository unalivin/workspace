# Testing

Run the full check suite:

```bash
make check
```

This runs `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace` (both with and without the `gui` feature).
