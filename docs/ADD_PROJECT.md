# Adding a project

## From the registry

Every planned project already has a row in `scripts/projects.tsv`. To start one:

```bash
scripts/new-project.sh --dry-run certwatch go
scripts/new-project.sh certwatch go
make -C certwatch check
```

`new-project.sh`:

1. checks that the name is in the registry with the same primary language and that the folder does not exist
2. copies every tracked and non-ignored file of `templates/<language>/` into the new folder
3. replaces the template tokens in file contents and in file and directory names
4. sets the registry status to `In progress`
5. refreshes the README table and regenerates the workflows

| Token in the template | Replaced with |
|---|---|
| `templateapp` | Folder name, for example `dns-sentinel` |
| `TEMPLATEAPP` | Environment prefix, for example `DNS_SENTINEL` |
| `Template App` | App name, for example `DNS Sentinel` |
| `TEMPLATE_TAGLINE` | Tagline from the registry |
| `TEMPLATE_DESCRIPTION` | GitHub description from the registry |
| `TEMPLATE_TOPICS` | Seven topics, comma separated |
| `TEMPLATE_ICON_CONCEPT` | Icon concept from the registry |
| `templates/<language>` | Folder name |

For a Rust project, run `make fmt docs notices` in the new folder before `make check`. The folder name replaces `templateapp` inside source lines, so `rustfmt` may wrap them differently, and the generated documents name the crates. No Rust project has been created from the template yet, so this step is expected but not exercised.

## A project that is not in the registry

Add a row to `scripts/projects.tsv` first, with status `Not started`, seven topics and a description of about 200 to 280 characters. The registry check expects exactly 25 rows and the language split 14 Go, 8 Rust and 3 C++, so a new project also needs those expectations in `scripts/verify-repo.sh` updated in the same change.

## Order of work inside a project

1. Requirements, acceptance criteria and threat model
2. Schema migrations and SQL tests
3. Domain and service layers with tests
4. CLI
5. Terminal UI
6. Dashboard
7. Notifications
8. Icon and identity
9. Documentation
10. CI and release workflow
11. Requirement audit table and quality report

Set the status to `Complete` in the registry only when the project's CI is green on Linux, macOS and Windows on its final commit, the documentation matches the implementation and the audit table is honest. Then run `scripts/sync-readme.sh`.

## Template fixes

A bug found in template code is fixed in `templates/<language>/` and in every project folder that already copied the affected file, in the same change or in directly following commits.
