# Roadmap

This page states what exists in the workspace and what does not. The project table in the README shows the status of each project.

## Present

- Workspace tooling: the registry, the scripts in `scripts/`, the repository checks and the generated workflows, with tests. Checked by the workflow `Root checks`.
- The Go template `templates/go`: a complete application with a dashboard, a terminal UI and commands over one service layer, its scripts, documentation and tests. Built, tested and packaged by the workflow `CI template-go` on Linux, macOS and Windows runners.
- The Rust template `templates/rust`: the same application as a Cargo workspace, with the same scripts, documentation and tests. Built, tested and packaged by the workflow `CI template-rust` on Linux, macOS and Windows runners.
- Workflow templates for Go and Rust projects: CI and tag-triggered release.
- `certwatch`, the Go pilot, `In progress`. Its state is in `certwatch/docs/ROADMAP.md`. The pre-release `certwatch/v0.1.0-dev` was published by its release workflow.

## Not present

- The C++ template `templates/cpp` and its workflow templates.
- The other 24 projects. Their rows of the project table are `Not started`.
- A release of a Rust project. The Rust release workflow template has never run, because no Rust project and no tag for one exists.

## Not verified

- The dashboard of either template on a real desktop. Both are built on the three CI runners. The Go dashboard is tested through its toolkit's headless driver; the Rust dashboard is tested by feeding events to the interface code and painting it into a picture without a window. See `PLATFORM_SUPPORT.md` in each template's `docs/`.
- The terminal UI of the Rust template in a real terminal. It is tested by feeding keys and reading the drawn screen.
- Starting a packaged application on a clean Linux, macOS or Windows machine.

## Order of work

1. C++ template with the same capabilities and checks as the Go and Rust templates.
2. One pilot project per language: `certwatch` (Go, in progress), `secretsentry` (Rust), `netgraph` (C++).
3. The remaining projects in table order.

A project becomes `Complete` only when its CI is green on Linux, macOS and Windows and its documentation matches what is implemented.
