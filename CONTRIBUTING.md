# Contributing

Thank you for helping. This file covers the workspace as a whole. Each project folder has its own `CONTRIBUTING.md` with its module names and checks.

## Ground rules

- One project per top-level folder. Never import code from another project folder. Shared code is copied from `templates/<language>/`.
- A template bug is fixed in the template and in every project that already copied it.
- No comments in Go, Rust, C++, SQL or Bash source files. Express intent through names, small modules, tests and the documents in `docs/`.
- No email address anywhere in the repository. Use a GitHub noreply address for commits.
- No fabricated features, results or measurements. Report anything that was not run as not run.

## Before you push

```bash
make check                        # root checks
scripts/check-all.sh <project>    # the full suite of the project you touched
```

## Commit messages

Commits follow [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/). In this workspace the scope starts with the project folder name. Workspace tooling uses the scope `workspace`, and templates use `template-go`, `template-rust` or `template-cpp`.

| Type | Example |
|---|---|
| `feat` | `feat(certwatch): add expiry thresholds` |
| `fix` | `fix(netpulse/poller): stop overlapping check runs` |
| `docs` | `docs(workspace): describe the split procedure` |
| `test` | `test(template-go): cover launcher decisions on headless sessions` |
| `refactor` | `refactor(ipam-pro/cidr): split prefix arithmetic into its own package` |
| `perf` | `perf(logforge-siem): batch inserts during ingestion` |
| `build` | `build(netgraph): pin the dependency baseline` |
| `ci` | `ci(workspace): verify generated workflows` |
| `chore` | `chore(workspace): refresh the project table` |
| `revert` | `revert(certwatch): revert "add expiry thresholds"` |

A breaking change adds `!` after the scope and a footer:

```text
feat(certwatch)!: rename the endpoints table

BREAKING CHANGE: databases created before this change need the 0002 migration.
```

## Branches

Use `<type>/<project>-<short-topic>`, for example `feat/certwatch-expiry-thresholds` or `fix/workspace-workflow-generator`.

## Review checklist

- [ ] The change is limited to one project, one template or the workspace tooling
- [ ] `make check` passes at the root and in the touched folder
- [ ] New behaviour has tests, including failure cases
- [ ] No comments in source files and no email addresses
- [ ] Documentation describes what is implemented
- [ ] Generated files are current (`scripts/gen-workflows.sh`, `scripts/sync-readme.sh`)
- [ ] The commit message follows the convention above
