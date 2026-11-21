# Contributing to {{APP}}

We welcome bug reports, documentation fixes, and pull requests.

## Commit messages

Use Conventional Commits with scope `{{PROJECT}}`:

- `feat({{PROJECT}}): add new capability`
- `fix({{PROJECT}}): handle empty input`
- `docs({{PROJECT}}): update usage guide`
- `test({{PROJECT}}): add boundary fixture`
- `refactor({{PROJECT}}): simplify parser`
- `perf({{PROJECT}}): avoid extra allocation`
- `build({{PROJECT}}): update dependency`
- `ci({{PROJECT}}): run check target`
- `chore({{PROJECT}}): update changelog`
- `revert({{PROJECT}}): restore previous behavior`

Add a `BREAKING CHANGE:` footer when a change breaks compatibility.

## Branch naming

Use `<type>/<short-description>`, for example `feat/{{PROJECT}}-filter` or `fix/{{PROJECT}}-empty-db`.

## Review checklist

1. Run `{{TEST_CMD}}` and confirm all checks pass.
2. Add tests for new behavior and regression tests for bug fixes.
3. Update `docs/` and `CHANGELOG.md` when user-facing behavior changes.
4. Confirm there are no comments in source files and no email addresses anywhere.
