# Contributing to LogForge-SIEM

We welcome bug reports, documentation fixes, and pull requests.

## Commit messages

Use Conventional Commits with scope `logforge-siem`:

- `feat(logforge-siem): add new capability`
- `fix(logforge-siem): handle empty input`
- `docs(logforge-siem): update usage guide`
- `test(logforge-siem): add boundary fixture`
- `refactor(logforge-siem): simplify parser`
- `perf(logforge-siem): avoid extra allocation`
- `build(logforge-siem): update dependency`
- `ci(logforge-siem): run check target`
- `chore(logforge-siem): update changelog`
- `revert(logforge-siem): restore previous behavior`

Add a `BREAKING CHANGE:` footer when a change breaks compatibility.

## Branch naming

Use `<type>/<short-description>`, for example `feat/logforge-siem-filter` or `fix/logforge-siem-empty-db`.

## Review checklist

1. Run `make check` and confirm all checks pass.
2. Add tests for new behavior and regression tests for bug fixes.
3. Update `docs/` and `CHANGELOG.md` when user-facing behavior changes.
4. Confirm there are no comments in source files and no email addresses anywhere.
