## Pick the project template

Every project has its own pull request template with its commit scope, test commands and rules. If this pull request changes a project, open it with that project's template: add the query shown below to the address of the page where you write this pull request, then reload.

- `certwatch` (CertWatch): `?template=certwatch.md`

Use the form below only for the workspace tooling in `scripts/`, the templates in `templates/`, the generated workflows and the root documents.

## What changed

## Area

Workspace tooling, a language template (`template-go`, `template-rust`, `template-cpp`) or the root documents.

## How it was verified

List the exact commands you ran and their outcome. Say what was not run.

## Checklist

- [ ] Limited to the workspace tooling or one template; project changes use the project's own template
- [ ] `make check` passes at the root and in the touched template
- [ ] Tests cover the change, including failure cases
- [ ] No comments in source files and no email addresses
- [ ] Documentation matches the implementation
- [ ] `CHANGELOG.md` has an entry under Unreleased
- [ ] The title follows Conventional Commits with the scope of the touched area
