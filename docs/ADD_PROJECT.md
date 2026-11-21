# Adding a Project

1. Add a row to `scripts/projects.tsv` with `folder`, `app`, `language`, `description`, `destination`, `copyright_holder`, and `status`.
2. Run `scripts/new-project.sh <folder>` to create the shared docs, pull request template, and language skeleton.
3. Run `scripts/sync-readme.sh` to update the root `README.md` tables.
4. Run `scripts/verify-repo.sh` to confirm all checks pass.
