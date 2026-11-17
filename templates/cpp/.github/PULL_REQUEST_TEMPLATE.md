## What changed

## Why

## How it was verified

Run these in the Template App folder and tick what passed. Say plainly what you did not run.

- [ ] `make check` (`clang-format`, `cppcheck` and `clang-tidy` with warnings as errors, tests of both builds, sanitizers, generated files, scripts, supply chain)
- [ ] `make test` (everything, dashboard included) and `make test-nogui` (build without the dashboard)
- [ ] `make race-or-sanitize` (AddressSanitizer with UndefinedBehaviorSanitizer, then ThreadSanitizer)
- [ ] `make fuzz` when a parser or an importer changed
- [ ] `make screenshots` when the dashboard changed, and the pictures in `dist/screens` were looked at

## Platforms

Template App supports Linux x86-64, macOS on Apple silicon and Windows x86-64. CI builds and tests all three, with GCC and Clang on Linux, Apple Clang on macOS and MSVC on Windows. Tick the systems you ran it on yourself:

- [ ] Linux x86-64
- [ ] macOS on Apple silicon
- [ ] Windows x86-64
- [ ] None, I rely on CI

## Checklist

- [ ] The title is a Conventional Commit with the scope `templateapp`, for example `fix(templateapp): keep the selection after a refresh`
- [ ] Tests were added or changed with this change, including a failure case
- [ ] The documents in `docs/` and the usage guide match the new behavior
- [ ] `CHANGELOG.md` has an entry under Unreleased
- [ ] No comments in source files (C++, CMake, SQL, Bash)
- [ ] No email address anywhere
- [ ] The capability is reachable from the dashboard, the terminal UI and the commands, and `docs/COMMANDS.md` lists it
- [ ] Every SQL statement uses bound parameters, and schema changes are a new file in `migrations/`
- [ ] Interface changes follow the Theming section of `docs/ARCHITECTURE.md`
- [ ] Nothing writes, deletes or sends without a confirmation or a dry run
