# Changelog

All notable changes to the workspace tooling are recorded here. Each project keeps its own `CHANGELOG.md`.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added

- Project registry (`scripts/projects.tsv`) with the 25 planned projects.
- Workspace scripts: `list-projects.sh`, `check-all.sh`, `gen-workflows.sh`, `new-project.sh`, `split-project.sh`, `publish-github.sh`, `sync-readme.sh`, `verify-repo.sh` and `install-shell-tools.sh`, with bats tests.
- Repository checks for required files, email addresses, source comments, generated workflows and the README project table.
- Go application template (`templates/go`) with its CI and release workflow templates.
- Rust application template (`templates/rust`) with its CI and release workflow templates.
- Registry columns `destination` and `copyright_holder`: 17 projects go to the Sonix-Devs organization with the holder Sonix Devs, 8 stay personal with the holder Unalivin. The README, `docs/WORKSPACE.md` and `docs/ROADMAP.md` carry the generated destination table.
- Repository checks `destinations`, `holders` and `pull-requests`.
- A pull request template per project, generated copies in `.github/PULL_REQUEST_TEMPLATE/` and a root default that lists them.
- A publisher in both templates and in `certwatch`: About screen, notices header, README, archive `README.txt`, the Windows version resource and the macOS bundle.

### Changed

- `scripts/split-project.sh` takes the owner from the destination, accepts `--owner`, rewrites module path and application identifier, sets and verifies the copyright holder and requires the project's pull request template.
- `scripts/new-project.sh` writes the owner and copyright holder of the destination into the new project.
- `scripts/gen-workflows.sh` also keeps the pull request templates current; `--only` limits it to workflows or pull request templates.
- `scripts/list-projects.sh --tsv` prints the destination and the copyright holder as two more columns.
- The Go packaging script builds the Windows executable with `go build` and a version resource instead of the toolkit's packager, which cannot set a company name.
