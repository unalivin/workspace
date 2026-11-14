# Roadmap

This page states what exists in the workspace and what does not. The project table in the README shows the status of each project.

## Present

- Workspace tooling: the registry, the scripts in `scripts/`, the repository checks and the generated workflows, with tests. Checked by the workflow `Root checks`.
- The Go template `templates/go`: a complete application with a dashboard, a terminal UI and commands over one service layer, its scripts, documentation and tests. Built, tested and packaged by the workflow `CI template-go` on Linux, macOS and Windows runners.
- Workflow templates for Go projects: CI and tag-triggered release.

## Not present

- The Rust template `templates/rust` and its workflow templates.
- The C++ template `templates/cpp` and its workflow templates.
- All 25 projects. Every row of the project table is `Not started`.
- A published release of any project. The release workflow template has not run, because no project and no tag exists.

## Not verified

- The Go template's dashboard on a real desktop. It is built on the three CI runners and tested through the toolkit's headless driver. See `templates/go/docs/PLATFORM_SUPPORT.md`.
- Starting a packaged Go application on a clean Linux, macOS or Windows machine.

## Order of work

1. Rust template and C++ template with the same capabilities and checks as the Go template.
2. One pilot project per language: `certwatch` (Go), `secretsentry` (Rust), `netgraph` (C++).
3. The remaining projects in table order.

A project becomes `Complete` only when its CI is green on Linux, macOS and Windows and its documentation matches what is implemented.
