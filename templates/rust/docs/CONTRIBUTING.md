# Contributing guide

## Setup

```bash
scripts/dev-setup.sh            # report missing tools
scripts/dev-setup.sh --install  # install them (apt packages use sudo on Debian and Ubuntu)
make check                      # the full local suite
```

`make help` lists every target. `make check` skips a step with a stated reason when its tool is not installed (for example `cargo audit`); CI installs every tool.

The Rust version is pinned in `rust-toolchain.toml`; `rustup` installs it on the first `cargo` call. Direct dependencies are pinned to exact versions in the workspace `Cargo.toml`, and every cargo call of the Makefile uses `--locked`.

## Rules

- **No comments** in Rust, SQL or Bash source files, doc comments included. Use names, small functions and tests; put reasons into `docs/` or an ADR. Attributes (`#[cfg(...)]`, `#![no_main]`) and the script interpreter line are directives, not comments, and stay.
- **No `unsafe`.** The workspace lints forbid it, and clippy runs with warnings as errors.
- **No email address** anywhere. Commit with your GitHub noreply address.
- **One service layer.** The dashboard, the terminal UI and the commands call `templateapp-core`. A feature is added to all three in the same change and gets a row in the capability matrix (`CAPABILITIES` in `crates/templateapp-core/src/service.rs`); `make docs` regenerates the tables in `docs/COMMANDS.md`.
- **SQL** lives in the modules `store`, `settings`, `records` and `notify` of `templateapp-core`, always with bound parameters. Schema changes are a new numbered file in `migrations/`, a new entry in the migration list of `store.rs`, and an updated `testdata/schema.snapshot`.
- **Settings** are added in `crates/templateapp-core/src/settings.rs` with validation and a default, and documented in `docs/CONFIGURATION.md`.
- **Dependencies** need a reason and an exact version. After a change run `make notices` and `make vuln`; `cargo deny` fails for a license outside the list in `deny.toml`.
- **Documents** describe what is implemented. What is missing goes into `docs/ROADMAP.md`.
- **Generated files** (`assets/`, `THIRD_PARTY_NOTICES.md`, the tables in `docs/COMMANDS.md`) are never edited by hand: use `make assets`, `make notices`, `make docs`.

## Commit messages

Commits follow [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/): `type(scope): summary` in the imperative, lower case, without a full stop.

| Type | Use for | Example |
|---|---|---|
| `feat` | a new capability | `feat(templateapp/records): add a column chooser` |
| `fix` | a bug fix | `fix(templateapp/store): report a busy database with the way out` |
| `docs` | documentation only | `docs(templateapp): describe the digest check` |
| `test` | tests only | `test(templateapp/launcher): cover piped sessions on windows` |
| `refactor` | no change in behavior | `refactor(templateapp/gui): split the records page` |
| `perf` | a measured speed-up | `perf(templateapp/records): use the status index for the overview` |
| `build` | build system or dependencies | `build(templateapp): update the sqlite driver` |
| `ci` | workflows | `ci(templateapp): upload rendered screens` |
| `chore` | maintenance | `chore(templateapp): regenerate the notices` |
| `revert` | reverting a commit | `revert(templateapp): revert "add a column chooser"` |

### Scopes

In the workspace the scope starts with the project folder name, `templateapp`. A module name may follow after a slash. The allowed module names are the three interfaces, the modules of `templateapp-core`, and a few areas outside the crates:

`cli`, `gui`, `tui`, `brand`, `clock`, `csv`, `files`, `guide`, `launcher`, `meta`, `notifier`, `notify`, `platform`, `png`, `records`, `service`, `settings`, `store`, `theme`, `transfer`, `userconfig`, plus `fuzz`, `migrations`, `scripts`, `docs` and `release`.

In a repository that contains only this project, the folder name is dropped: `feat(records): add a column chooser`.

### Breaking changes

Add `!` after the scope and a `BREAKING CHANGE:` footer that says what users must do:

```text
feat(templateapp/store)!: rename the records table

BREAKING CHANGE: databases created before this change are migrated by 0002 on first start and can no longer be opened by older builds.
```

## Branches

`<type>/<short-topic>` in lower case with hyphens, for example `feat/column-chooser` or `fix/busy-retry`. In the workspace, put the project name first: `feat/templateapp-column-chooser`.

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
