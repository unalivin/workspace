# Contributing

We welcome bug reports, documentation fixes, and pull requests for any of the 25 tools in this workspace.

## Commit messages

Use Conventional Commits:

- `feat(logforge-siem): add FTS5 query parser`
- `fix(netauto): reject duplicate device names`
- `docs(ipam-pro): document IPv6 boundary checks`
- `test(certwatch): add expired chain fixture`
- `refactor(vulntrack): extract risk score calculation`
- `perf(routescope): index prefix lookup table`
- `build(securebaseline): pin CMake minimum version`
- `ci(scripts): run verify-repo on pull requests`
- `chore(workspace): update registry status`
- `revert(portwatch): restore previous rate limit`

For breaking changes, add a `BREAKING CHANGE:` footer explaining what changed and how to migrate.

## Branch naming

Use `<type>/<short-description>`, for example `feat/logforge-fts5` or `fix/netauto-dup-device`.

## Before opening a pull request

1. Run `make check` at the repository root and in the project folder you changed.
2. Add tests for new behavior and regression tests for bug fixes.
3. Update the project's `docs/` and `CHANGELOG.md` if user-facing behavior changed.
4. Confirm there are no comments in source files and no email addresses anywhere in the diff.
