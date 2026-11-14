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

## Layout

| Path | Purpose |
|---|---|
| `scripts/` | Workspace tooling: project registry, checks, workflow generator, project creation, split and publish |
| `templates/go` | Reusable Go application skeleton, copied into each Go project. The Rust and C++ templates are not written yet. |
| `templates/workflows/` | Workflow templates that `scripts/gen-workflows.sh` renders into `.github/workflows/` |
| `docs/` | Workspace documentation |
| `<project>/` | One independent application per folder |

## Build instructions per language

Each project and each template has a `Makefile` with the same targets: `fmt`, `fmt-check`, `lint`, `test`, `race-or-sanitize`, `build`, `build-nogui`, `check` and `clean`.

| Language | Toolchain | Build with GUI | Build without GUI | Full check |
|---|---|---|---|---|
| Go | Go 1.27.4, a C compiler for cgo | `make build` | `make build-nogui` | `make check` |
| Rust | Pinned stable toolchain from `rust-toolchain.toml` | `make build` | `make build-nogui` | `make check` |
| C++ | C++20 compiler, CMake, Ninja | `make build` | `make build-nogui` | `make check` |

Details, system packages and tool versions are in [docs/LANGUAGES.md](docs/LANGUAGES.md).

## Workspace commands

```bash
make check                      # root checks: shellcheck, shfmt, bats, structure, emails, comments, workflows
scripts/list-projects.sh        # print the project table
scripts/check-all.sh            # run "make check" in every template and project that exists
scripts/check-all.sh certwatch  # run it for selected targets
scripts/new-project.sh certwatch go
scripts/gen-workflows.sh        # regenerate .github/workflows
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

MIT, see [LICENSE](LICENSE). Third-party components keep their own licenses, recorded in each project's `THIRD_PARTY_NOTICES.md`.
