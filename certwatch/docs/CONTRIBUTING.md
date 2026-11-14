# Contributing guide

## Setup

```bash
scripts/dev-setup.sh            # report missing tools
scripts/dev-setup.sh --install  # install them (apt packages use sudo on Debian and Ubuntu)
make check                      # the full local suite
```

`make help` lists every target. `make check` skips the dashboard build with a stated reason when the windowing headers are not installed; CI always builds it.

## Rules

- **No comments** in Go, SQL or Bash source files. Use names, small functions and tests; put reasons into `docs/` or an ADR. Build constraints (`//go:build`) and the script interpreter line are the only exceptions.
- **No email address** anywhere. Commit with your GitHub noreply address.
- **One service layer.** The dashboard, the terminal UI and the commands call `internal/app`. A feature is added to all three in the same change and gets a row in the capability matrix (`internal/app/capabilities.go`); `make docs` regenerates the tables in `docs/COMMANDS.md`.
- **SQL** lives in `internal/store`, `internal/settings`, `internal/records` and `internal/notify`, always with bound parameters. Schema changes are a new numbered file in `migrations/` and an updated `testdata/schema.snapshot`.
- **Settings** are added in `internal/settings/settings.go` with validation and a default, and documented in `docs/CONFIGURATION.md`.
- **Dependencies** need a reason. After a change run `make notices`; it fails for a license outside the allowed list.
- **Documents** describe what is implemented. What is missing goes into `docs/ROADMAP.md`.
- **Generated files** (`assets/`, `THIRD_PARTY_NOTICES.md`, the tables in `docs/COMMANDS.md`) are never edited by hand: use `make assets`, `make notices`, `make docs`.

## Commit messages

Commits follow [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/): `type(scope): summary` in the imperative, lower case, without a full stop.

| Type | Use for | Example |
|---|---|---|
| `feat` | a new capability | `feat(certwatch/records): add a column chooser` |
| `fix` | a bug fix | `fix(certwatch/store): retry when the database is busy during open` |
| `docs` | documentation only | `docs(certwatch): describe the digest check` |
| `test` | tests only | `test(certwatch/launcher): cover piped sessions on windows` |
| `refactor` | no change in behavior | `refactor(certwatch/gui): split the records page` |
| `perf` | a measured speed-up | `perf(certwatch/records): use the status index for the overview` |
| `build` | build system or dependencies | `build(certwatch): update the sqlite driver` |
| `ci` | workflows | `ci(certwatch): upload rendered screens` |
| `chore` | maintenance | `chore(certwatch): regenerate the notices` |
| `revert` | reverting a commit | `revert(certwatch): revert "add a column chooser"` |

### Scopes

In the workspace the scope starts with the project folder name, `certwatch`. A module name may follow after a slash. The allowed module names are the package names:

`app`, `brand`, `cli`, `gui`, `guide`, `launcher`, `licenses`, `meta`, `notify`, `platform`, `records`, `settings`, `sqlscan`, `store`, `theme`, `tui`, `userconfig`, plus `migrations`, `scripts`, `docs` and `release`.

In a repository that contains only this project, the folder name is dropped: `feat(records): add a column chooser`.

### Breaking changes

Add `!` after the scope and a `BREAKING CHANGE:` footer that says what users must do:

```text
feat(certwatch/store)!: rename the records table

BREAKING CHANGE: databases created before this change are migrated by 0002 on first start and can no longer be opened by older builds.
```

## Branches

`<type>/<short-topic>` in lower case with hyphens, for example `feat/column-chooser` or `fix/busy-retry`. In the workspace, put the project name first: `feat/certwatch-column-chooser`.

## Review checklist

- [ ] `make check` passes
- [ ] New behavior has tests, including the failure cases
- [ ] The capability is reachable from the dashboard, the terminal UI and the commands, or the matrix says why not
- [ ] User-visible text is plain, says what happened and what to do
- [ ] No comments in source files, no email address
- [ ] `docs/` and `CHANGELOG.md` are updated and describe what is implemented
- [ ] Generated files are current
- [ ] The commit messages follow the convention

## Security

Do not report a vulnerability in an issue or a pull request. Use the private advisory form, see [../SECURITY.md](../SECURITY.md).
