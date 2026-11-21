# Contributing to NetAuto

We welcome bug reports, documentation fixes, and pull requests.

## Commit messages

Use Conventional Commits with scope `netauto`:

- `feat(netauto): add new capability`
- `fix(netauto): handle empty input`
- `docs(netauto): update usage guide`
- `test(netauto): add boundary fixture`
- `refactor(netauto): simplify parser`
- `perf(netauto): avoid extra allocation`
- `build(netauto): update dependency`
- `ci(netauto): run check target`
- `chore(netauto): update changelog`
- `revert(netauto): restore previous behavior`

Add a `BREAKING CHANGE:` footer when a change breaks compatibility.

## Branch naming

Use `<type>/<short-description>`, for example `feat/netauto-filter` or `fix/netauto-empty-db`.

## Review checklist

1. Run `make check` and confirm all checks pass.
2. Add tests for new behavior and regression tests for bug fixes.
3. Update `docs/` and `CHANGELOG.md` when user-facing behavior changes.
4. Confirm there are no comments in source files and no email addresses anywhere.
