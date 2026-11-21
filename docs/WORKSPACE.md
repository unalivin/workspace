# Workspace Layout

`scripts/projects.tsv` is the single source of truth for all 25 projects. It records each project's folder, app name, primary language, one-sentence description, destination (`sonix-devs` or `personal`), copyright holder, and status.

`scripts/gen-workflows.sh` reads the registry and writes `.github/workflows/<project>-ci.yml`, `.github/workflows/<project>-release.yml`, and `.github/PULL_REQUEST_TEMPLATE/<project>.md` from `templates/`.

`scripts/sync-readme.sh` updates the project and destination tables in `README.md` from `scripts/projects.tsv`.

`scripts/verify-repo.sh` checks that no email addresses appear anywhere, no `.workspace/` folder exists inside any project, all workflow and PR template files match the templates, and `README.md` matches the registry.
