# Decisions Log

- 2026-11-21: Pin Rust toolchain to `1.83.0` (or latest stable in CI) in `rust-toolchain.toml` and CI workflows. Reason: sandbox has no local `rustc`, so CI will verify the pinned stable toolchain.
- 2026-11-21: Use `github.com/unalivin/workspace` as the monorepo root URL and `github.com/sonix-devs/<project>` / `github.com/unalivin/<project>` for per-project links. Reason: matches `git remote get-url origin` and the destination registry.
- 2026-11-21: The referenced `build-prompt-final.md` and its Appendices are not present in this checkout or Git history. Continue using the shipped project README, docs, CLI, existing screen registry, and QA.md as the available product specification; record this limitation in the audit instead of inventing Appendix criteria.
