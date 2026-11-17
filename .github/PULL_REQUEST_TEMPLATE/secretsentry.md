## What changed

## Why

## How it was verified

Run these in the SecretSentry folder and tick what passed. Say plainly what you did not run.

- [ ] `make check` (`cargo fmt --check`, `cargo clippy` with warnings as errors, tests of both builds, sanitizer, generated files, scripts)
- [ ] `make test` (whole workspace, dashboard included) and `make test-nogui` (build without the dashboard)
- [ ] `make race-or-sanitize` (AddressSanitizer)
- [ ] `make fuzz` when a parser or an importer changed
- [ ] `make screenshots` when the dashboard changed, and the pictures in `dist/screens` were looked at

## Platforms

SecretSentry supports Linux x86-64, macOS on Apple silicon and Windows x86-64. CI builds and tests all three. Tick the systems you ran it on yourself:

- [ ] Linux x86-64
- [ ] macOS on Apple silicon
- [ ] Windows x86-64
- [ ] None, I rely on CI

## Checklist

- [ ] The title is a Conventional Commit with the scope `secretsentry`, for example `fix(secretsentry): keep the selection after a refresh`
- [ ] Tests were added or changed with this change, including a failure case
- [ ] The documents in `docs/` and the usage guide match the new behavior
- [ ] `CHANGELOG.md` has an entry under Unreleased
- [ ] No comments in source files (Rust, SQL, Bash)
- [ ] No email address anywhere
- [ ] The capability is reachable from the dashboard, the terminal UI and the commands, and `docs/COMMANDS.md` lists it
- [ ] Every SQL statement uses bound parameters, and schema changes are a new file in `migrations/`
- [ ] Interface changes follow `docs/DESIGN.md`
- [ ] Nothing writes, deletes or sends without a confirmation or a dry run
