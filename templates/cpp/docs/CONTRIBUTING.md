# Contributing guide

## Setup

```bash
scripts/dev-setup.sh            # report missing tools
scripts/dev-setup.sh --install  # install them (apt packages use sudo on Debian and Ubuntu)
make check                      # the full local suite
```

`make help` lists every target. `make check` skips a step with a stated reason when its tool is not installed (for example `clang-tidy`); CI installs every tool.

The project is C++20, built with CMake and Ninja through the presets in `CMakePresets.json`. `scripts/dev-setup.sh --install` puts vcpkg at the pinned commit into `.tools/vcpkg`, and the first configure step builds the libraries of `vcpkg.json` from source, which takes several minutes once. If you already have the libraries, pass their location in `CMAKE_PREFIX_PATH` and vcpkg is not needed. The versions of the analysis tools are pinned in `scripts/lib.sh`; `scripts/dev-setup.sh` reports a version that differs.

## Rules

- **No comments** in C++, CMake, SQL or Bash source files. Use names, small functions and tests; put reasons into `docs/` or an ADR. Preprocessor lines (`#include`, `#if`) and the script interpreter line are directives, not comments, and stay. The vendored SQLite in `third_party/` is not changed.
- **No suppressions.** No `NOLINT`, no `cppcheck-suppress`, no pragma that silences a warning. The code is changed until the compilers (`-Wall -Wextra -Wpedantic -Wshadow -Wconversion` and more, `/W4` on MSVC, all as errors), `clang-tidy` and `cppcheck` are satisfied. A check that does not fit the code base is turned off for everyone in `.clang-tidy`, with the reason in [TESTING.md](TESTING.md).
- **No owning raw pointers, no `new` and `delete`.** Values, containers and standard smart pointers own; handles of C libraries are wrapped in a class with a destructor.
- **No email address** anywhere. Commit with your GitHub noreply address.
- **One service layer.** The dashboard, the terminal UI and the commands call `src/core`. A feature is added to all three in the same change and gets a row in the capability matrix (`capabilities()` in `src/core/service.cpp`); `make docs` regenerates the tables in `docs/COMMANDS.md`.
- **SQL** lives in `store.cpp`, `settings.cpp`, `records.cpp` and `notify.cpp` of `src/core`, always as constants with bound parameters. Schema changes are a new numbered file in `migrations/`, which the build embeds, and an updated `testdata/schema.snapshot`.
- **Settings** are added in `src/core/settings.cpp` with validation and a default, and documented in `docs/CONFIGURATION.md`.
- **Dependencies** need a reason and an exact version: an override in `vcpkg.json` and a row in `third_party/components.tsv` with the license, the license text in `third_party/licenses` and the upstream commit. After a change run `make notices` and `make vuln`; both fail for a license outside the accepted list.
- **Documents** describe what is implemented. What is missing goes into `docs/ROADMAP.md`. A test is cited by its name in quotes inside a code span, for example `"the decision table holds"`, and a test fails when a cited name does not exist.
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
| `build` | build system or dependencies | `build(templateapp): update sqlite` |
| `ci` | workflows | `ci(templateapp): upload rendered screens` |
| `chore` | maintenance | `chore(templateapp): regenerate the notices` |
| `revert` | reverting a commit | `revert(templateapp): revert "add a column chooser"` |

### Scopes

In the workspace the scope starts with the project folder name, `templateapp`. A module name may follow after a slash. The allowed module names are the three interfaces, the modules of `src/core`, and a few areas outside `src`:

`cli`, `gui`, `tui`, `brand`, `clock`, `csv`, `files`, `guide`, `launcher`, `meta`, `notifier`, `notify`, `platform`, `png`, `records`, `service`, `settings`, `store`, `text`, `theme`, `transfer`, `userconfig`, plus `build`, `fuzz`, `migrations`, `scripts`, `docs` and `release`.

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
