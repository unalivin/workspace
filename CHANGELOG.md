# Changelog

All notable changes to the workspace tooling are recorded here. Each project keeps its own `CHANGELOG.md`.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added

- Project registry (`scripts/projects.tsv`) with the 25 planned projects.
- Workspace scripts: `list-projects.sh`, `check-all.sh`, `gen-workflows.sh`, `new-project.sh`, `split-project.sh`, `publish-github.sh`, `sync-readme.sh`, `verify-repo.sh` and `install-shell-tools.sh`, with bats tests.
- Repository checks for required files, email addresses, source comments, generated workflows and the README project table.
- Go application template (`templates/go`) with its CI and release workflow templates.
