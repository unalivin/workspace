# Contributing to VulnTrack

We welcome bug reports, documentation fixes, and pull requests.

## Commit messages

Use Conventional Commits with scope `vulntrack`:

- `feat(vulntrack): add new capability`
- `fix(vulntrack): handle empty input`
- `docs(vulntrack): update usage guide`
- `test(vulntrack): add boundary fixture`
- `refactor(vulntrack): simplify parser`
- `perf(vulntrack): avoid extra allocation`
- `build(vulntrack): update dependency`
- `ci(vulntrack): run check target`
- `chore(vulntrack): update changelog`
- `revert(vulntrack): restore previous behavior`

Add a `BREAKING CHANGE:` footer when a change breaks compatibility.

## Branch naming

Use `<type>/<short-description>`, for example `feat/vulntrack-filter` or `fix/vulntrack-empty-db`.

## Review checklist

1. Run `make check` and confirm all checks pass.
2. Add tests for new behavior and regression tests for bug fixes.
3. Update `docs/` and `CHANGELOG.md` when user-facing behavior changes.
4. Confirm there are no comments in source files and no email addresses anywhere.
