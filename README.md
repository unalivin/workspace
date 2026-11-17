# workspace

Monorepo of 25 local-first defensive security and network tools in Go, Rust and C++, each with a desktop GUI, terminal UI and CLI, SQLite storage and tag-triggered pre-releases.

Every project lives in its own top-level folder, builds and tests on its own, never imports code from another folder, and can be split into its own repository with `scripts/split-project.sh`.

## Status

The status column is generated from `scripts/projects.tsv` and checked in CI. A project is **Complete** only when its CI is green on Linux, macOS and Windows, its documentation matches the implementation and its quality report is honest. A folder that does not exist yet is **Not started**.

What exists and what does not is stated in [docs/ROADMAP.md](docs/ROADMAP.md).

<!-- projects:start -->
| # | Folder | App | Language | Description | Status | Docs |
|---|---|---|---|---|---|---|
| 1 | [`certwatch`](certwatch/) | CertWatch | Go | TLS Certificate Monitoring | In progress | [docs](certwatch/docs/README.md) |
| 2 | `dns-sentinel` | DNS Sentinel | Go | DNS Change Monitoring | Not started | - |
| 3 | `netpulse` | NetPulse | Go | Network Observability | Not started | - |
| 4 | `portwatch` | PortWatch | Go | Authorized Service Exposure Monitor | Not started | - |
| 5 | `ipam-pro` | IPAM-Pro | Go | IP Address Management | Not started | - |
| 6 | `netauto` | NetAuto | Go | Network Automation Workbench | Not started | - |
| 7 | `fleetops` | FleetOps | Go | Linux Fleet Inventory and Health | Not started | - |
| 8 | `incidentflow` | IncidentFlow | Go | Security Incident Management | Not started | - |
| 9 | `cloudfence` | CloudFence | Go | Cloud Security Posture | Not started | - |
| 10 | `kubeguard` | KubeGuard | Go | Kubernetes Security Posture | Not started | - |
| 11 | `pipelineshield` | PipelineShield | Go | CI/CD Security Findings Hub | Not started | - |
| 12 | `vulntrack` | VulnTrack | Go | Vulnerability Management | Not started | - |
| 13 | `threatatlas` | ThreatAtlas | Go | Threat Intelligence Workspace | Not started | - |
| 14 | `secops-copilot` | SecOps-Copilot | Go | Evidence-Grounded Security Assistant | Not started | - |
| 15 | `secretsentry` | SecretSentry | Rust | Secret Detection and Triage | Not started | - |
| 16 | `sbom-navigator` | SBOM-Navigator | Rust | Software Supply Chain Inventory | Not started | - |
| 17 | `iac-sentinel` | IaC-Sentinel | Rust | Infrastructure-as-Code Security | Not started | - |
| 18 | `sigmalab` | SigmaLab | Rust | Detection Rule Engineering | Not started | - |
| 19 | `logforge-siem` | LogForge-SIEM | Rust | Log Collection and Security Analytics | Not started | - |
| 20 | `phishlens` | PhishLens | Rust | Email Threat Triage | Not started | - |
| 21 | `forensicflow` | ForensicFlow | Rust | Digital Evidence Casework | Not started | - |
| 22 | `netconfig-auditor` | NetConfig-Auditor | Rust | Network Configuration Audit | Not started | - |
| 23 | `routescope` | RouteScope | C++ | Routing and BGP Visibility | Not started | - |
| 24 | `netgraph` | NetGraph | C++ | Network Topology and Inventory | Not started | - |
| 25 | `securebaseline` | SecureBaseline | C++ | Configuration Compliance | Not started | - |
<!-- projects:end -->

## Where each project will be published

The projects are developed here and published later as separate repositories. Some go to the company, the others stay personal. The table is generated from `scripts/projects.tsv` and checked in CI.

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

`scripts/split-project.sh <project>` reads the destination and prepares the standalone repository for the right owner, see [docs/RELEASING.md](docs/RELEASING.md).

## Layout

| Path | Purpose |
|---|---|
| `scripts/` | Workspace tooling: project registry, checks, workflow generator, project creation, split and publish |
| `templates/go` | Reusable Go application skeleton, copied into each Go project |
| `templates/rust` | Reusable Rust application skeleton, copied into each Rust project |
| `templates/cpp` | Reusable C++ application skeleton, copied into each C++ project |
| `templates/workflows/` | Workflow templates that `scripts/gen-workflows.sh` renders into `.github/workflows/` |
| `templates/pull-request/` | Source of the root pull request template; each language template carries the project pull request template in its `.github/` |
| `docs/` | Workspace documentation |
| `<project>/` | One independent application per folder |

## Build instructions per language

Each project and each template has a `Makefile` with the same targets: `fmt`, `fmt-check`, `lint`, `test`, `race-or-sanitize`, `build`, `build-nogui`, `check` and `clean`.

| Language | Toolchain | Build with GUI | Build without GUI | Full check |
|---|---|---|---|---|
| Go | Go 1.27.4, a C compiler for cgo | `make build` | `make build-nogui` | `make check` |
| Rust | Pinned stable toolchain from `rust-toolchain.toml` | `make build` | `make build-nogui` | `make check` |
| C++ | C++20 compiler, CMake, Ninja; libraries pinned through vcpkg | `make build` | `make build-nogui` | `make check` |

Details, system packages and tool versions are in [docs/LANGUAGES.md](docs/LANGUAGES.md).

## Workspace commands

```bash
make check                      # root checks: shellcheck, shfmt, bats and scripts/verify-repo.sh
scripts/list-projects.sh        # print the project table
scripts/check-all.sh            # run "make check" in every template and project that exists
scripts/check-all.sh certwatch  # run it for selected targets
scripts/new-project.sh certwatch go
scripts/gen-workflows.sh        # regenerate .github/workflows and the pull request templates
scripts/sync-readme.sh          # regenerate the project table and the destination tables
scripts/split-project.sh certwatch
```

## Documentation

- [docs/WORKSPACE.md](docs/WORKSPACE.md): how the monorepo is organised and checked
- [docs/LANGUAGES.md](docs/LANGUAGES.md): language profiles, toolchains and tool versions
- [docs/ADD_PROJECT.md](docs/ADD_PROJECT.md): creating a project from a template
- [docs/RELEASING.md](docs/RELEASING.md): tag-triggered pre-releases and splitting a project
- [docs/ROADMAP.md](docs/ROADMAP.md): what exists, what does not, and the order of work

## Security

Report vulnerabilities through the private advisory form described in [SECURITY.md](SECURITY.md). Every tool in this workspace is defensive and acts only on scope the user configures.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) and the [Code of Conduct](CODE_OF_CONDUCT.md).

## License

MIT, see [LICENSE](LICENSE). Each project carries its own `LICENSE` with the copyright holder of its destination: Sonix Devs for the projects that go to the company, Unalivin for the personal ones. Third-party components keep their own licenses, recorded in each project's `THIRD_PARTY_NOTICES.md`.
