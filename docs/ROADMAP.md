# Roadmap

This page states what exists in the workspace and what does not. The project table in the README shows the status of each project.

## Present

- Workspace tooling: the registry, the scripts in `scripts/`, the repository checks, the generated workflows and the generated pull request templates, with tests. Checked by the workflow `Root checks`.
- The Go template `templates/go`: a complete application with a dashboard, a terminal UI and commands over one service layer, its scripts, documentation and tests. Built, tested and packaged by the workflow `CI template-go` on Linux, macOS and Windows runners.
- The Rust template `templates/rust`: the same application as a Cargo workspace, with the same scripts, documentation and tests. Built, tested and packaged by the workflow `CI template-rust` on Linux, macOS and Windows runners.
- The C++ template `templates/cpp`: the same application in C++20 with CMake, with the same scripts, documentation and tests. Built, tested and packaged by the workflow `CI template-cpp` on Linux, macOS and Windows runners.
- Workflow templates for Go, Rust and C++ projects: CI and tag-triggered release.
- `certwatch`, the Go pilot, `In progress`. Its state is in `certwatch/docs/ROADMAP.md`. The pre-release `certwatch/v0.1.0-dev` was published by its release workflow.

## Not present

- The other 24 projects. Their rows of the project table are `Not started`.
- A release of a Rust or a C++ project. The Rust and C++ release workflow templates have never run, because no such project and no tag for one exists.
- Any standalone repository. No project has been split or published to its destination yet.

## Not verified

- The dashboard of either template on a real desktop. Both are built on the three CI runners. The Go dashboard is tested through its toolkit's headless driver; the Rust dashboard is tested by feeding events to the interface code and painting it into a picture without a window. See `PLATFORM_SUPPORT.md` in each template's `docs/`.
- The terminal UI of the Rust template in a real terminal. It is tested by feeding keys and reading the drawn screen.
- Starting a packaged application on a clean Linux, macOS or Windows machine.
- `scripts/split-project.sh` against a real project of the company. It is tested with a generated project in a temporary workspace.

## Destinations

<!-- destinations:start -->
17 projects will be published in the GitHub organization [Sonix-Devs](https://github.com/Sonix-Devs), which belongs to the company. 8 projects stay on the personal profile [unalivin](https://github.com/unalivin). The copyright holder follows the destination: Sonix Devs for the company projects, Unalivin for the personal ones.

| Project | Language | Destination |
|---|---|---|
| `netpulse` | Go | Sonix-Devs organization (company) |
| `ipam-pro` | Go | Sonix-Devs organization (company) |
| `netauto` | Go | Sonix-Devs organization (company) |
| `fleetops` | Go | Sonix-Devs organization (company) |
| `incidentflow` | Go | Sonix-Devs organization (company) |
| `cloudfence` | Go | Sonix-Devs organization (company) |
| `kubeguard` | Go | Sonix-Devs organization (company) |
| `pipelineshield` | Go | Sonix-Devs organization (company) |
| `vulntrack` | Go | Sonix-Devs organization (company) |
| `secops-copilot` | Go | Sonix-Devs organization (company) |
| `sbom-navigator` | Rust | Sonix-Devs organization (company) |
| `iac-sentinel` | Rust | Sonix-Devs organization (company) |
| `sigmalab` | Rust | Sonix-Devs organization (company) |
| `logforge-siem` | Rust | Sonix-Devs organization (company) |
| `forensicflow` | Rust | Sonix-Devs organization (company) |
| `netgraph` | C++ | Sonix-Devs organization (company) |
| `securebaseline` | C++ | Sonix-Devs organization (company) |
| `certwatch` | Go | Personal profile (unalivin) |
| `dns-sentinel` | Go | Personal profile (unalivin) |
| `portwatch` | Go | Personal profile (unalivin) |
| `threatatlas` | Go | Personal profile (unalivin) |
| `secretsentry` | Rust | Personal profile (unalivin) |
| `phishlens` | Rust | Personal profile (unalivin) |
| `netconfig-auditor` | Rust | Personal profile (unalivin) |
| `routescope` | C++ | Personal profile (unalivin) |
<!-- destinations:end -->

## Order of work

One project at a time. A project is finished, or brought to a truthful `In progress` state, before the next one starts.

1. The remaining pilots: `secretsentry` (Rust) and `netgraph` (C++). `certwatch` (Go) is the first pilot and is in progress.
2. The projects that go to the company, in this order: `logforge-siem`, `forensicflow`, `sigmalab`, `iac-sentinel`, `sbom-navigator`, `securebaseline`, `netauto`, `vulntrack`, `ipam-pro`, `netpulse`, `fleetops`, `incidentflow`, `cloudfence`, `secops-copilot`, `kubeguard`, `pipelineshield`.
3. The personal projects, in this order: `threatatlas`, `certwatch` (finish it), `dns-sentinel`, `portwatch`, `phishlens`, `netconfig-auditor`, `routescope`.

A project becomes `Complete` only when its CI is green on Linux, macOS and Windows and its documentation matches what is implemented.
