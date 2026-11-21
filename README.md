# workspace

25 local-first defensive security and network tools, each in its own top-level folder. Every project ships a desktop GUI, a terminal UI, and a CLI backed by one SQLite store.

![Monorepo structure](docs/workspace.svg)

## Projects

<!-- PROJECTS_TABLE_START -->
| Folder | App | Language | Description | Status | Docs |
|---|---|---|---|---|---|
| `logforge-siem` | LogForge-SIEM | rust | Local-first SIEM-style log platform: ingests files and events, normalizes and deduplicates them, offers full-text search, correlation rules, alerts and investigation timelines. Desktop GUI, terminal UI and CLI. Data stays in one SQLite file. | Not started | [docs](logforge-siem/docs/README.md) |
| `netauto` | NetAuto | go | Local-first network automation tool: backs up device configurations, runs scheduled checks, applies approved change workflows and produces change reports with an audit trail. Desktop GUI, terminal UI and CLI. Nothing changes without approval. | Not started | [docs](netauto/docs/README.md) |
| `vulntrack` | VulnTrack | go | Local-first vulnerability management: imports scan findings, correlates assets and CVEs, prioritizes remediation with explainable risk scores and tracks fixes through tickets. Desktop GUI, terminal UI and CLI. Imports results, runs no scans. | Not started | - |
| `forensicflow` | ForensicFlow | rust | Local-first digital forensics case manager: records evidence metadata, cryptographic hashes and chain of custody, builds timelines and keeps investigation notes in one tamper-evident file. Desktop GUI, terminal UI and CLI. Never alters evidence. | Not started | - |
| `ipam-pro` | IPAM-Pro | go | Local-first IPv4 and IPv6 address manager: plans subnets and CIDR blocks, tracks VLANs, allocations and reservations, and detects conflicts and overlaps before they cause an outage. Desktop GUI, terminal UI and CLI. No network scanning. | Not started | - |
| `securebaseline` | SecureBaseline | cpp | Local-first security baseline assessor: checks host and application configuration against documented policies, reports pass, fail and evidence per control and tracks fixes. Desktop GUI, terminal UI and CLI. Read-only by default. | Not started | - |
| `netgraph` | NetGraph | cpp | Local-first network topology and inventory manager: models routers, switches, servers, interfaces, VLANs and links, shows dependencies on an interactive map and checks the model for inconsistencies. Desktop GUI, terminal UI and CLI. | Not started | - |
| `secops-copilot` | SecOps-Copilot | go | Local-first SecOps assistant workspace: summarizes alerts, explains findings and drafts remediation steps and reports, each grounded in cited source records with an audit of every model call. Desktop GUI, terminal UI and CLI. You control the model. | Not started | - |
| `cloudfence` | CloudFence | go | Local-first cloud security posture tool: reads exported or read-only cloud configuration, flags risky settings with explainable severity, and tracks remediation over time. Desktop GUI, terminal UI and CLI. Never changes cloud resources. | Not started | - |
| `incidentflow` | IncidentFlow | go | Local-first incident management: tracks security alerts, cases, evidence, timelines, assignments and response checklists, and produces post-incident reports from stored records. Desktop GUI, terminal UI and CLI. Everything stays in one local file. | Not started | - |
| `fleetops` | FleetOps | go | Local-first Linux fleet manager: keeps a server inventory, runs health checks, tracks patch status and metrics, and schedules maintenance tasks with an audit trail. Desktop GUI, terminal UI and CLI. Changes only hosts you configure. | Not started | - |
| `iac-sentinel` | IaC-Sentinel | rust | Static analyzer for Terraform, Dockerfiles and Kubernetes YAML: finds insecure infrastructure configuration before deployment, explains each rule and exports SARIF and JSON reports. Desktop GUI, terminal UI and CLI. Never deploys anything. | Not started | - |
| `kubeguard` | KubeGuard | go | Local-first Kubernetes security posture analyzer: reads manifests, RBAC, pod security settings and network policies, ranks risky configuration with explainable rules and exports reports. Desktop GUI, terminal UI and CLI. Never touches a live cluster. | Not started | - |
| `netpulse` | NetPulse | go | Local-first network observability tool: measures latency, packet loss, uptime and bandwidth for targets you configure, evaluates alert rules and builds incident timelines. Desktop GUI, terminal UI and CLI. Checks only the targets you add. | Not started | - |
| `sigmalab` | SigmaLab | rust | Local-first detection engineering workbench: author, validate, test and version Sigma-style rules against sample logs and map them to ATT&CK techniques with coverage reports. Desktop GUI, terminal UI and CLI. Tests run on files you provide. | Not started | - |
| `sbom-navigator` | SBOM-Navigator | rust | Local-first SBOM explorer: imports CycloneDX and SPDX documents, shows the dependency graph, license metadata and vulnerability matches with explainable severity. Desktop GUI, terminal UI and CLI. Works offline on files you import. | Not started | - |
| `pipelineshield` | PipelineShield | go | Local-first dashboard that merges SAST, dependency, secret and container scan results from CI/CD (SARIF and JSON), deduplicates findings, tracks status and exports reports. Desktop GUI, terminal UI and CLI. Imports results, runs no scans. | Not started | - |
| `routescope` | RouteScope | cpp | Local-first routing analysis tool: loads route tables and BGP announcements, keeps prefix history, detects route changes and visualizes paths. Desktop GUI, terminal UI and CLI. Works on files you import and sends no routing traffic. | Not started | - |
| `threatatlas` | ThreatAtlas | go | Local-first threat intelligence workbench: imports indicators (IPs, domains, URLs, hashes), deduplicates and scores them with sources and confidence, and links them to cases. Desktop GUI, terminal UI and CLI. Makes no outbound lookups by default. | Not started | - |
| `portwatch` | PortWatch | go | Local-first exposure change monitor: records open ports and service metadata for assets explicitly in scope, compares runs and alerts on new or changed exposure. Desktop GUI, terminal UI and CLI. Scans only authorized, scoped targets. | Not started | - |
| `netconfig-auditor` | NetConfig-Auditor | rust | Local-first configuration auditor for network devices and infrastructure files: checks backups against documented rules, detects drift between versions and reports findings with evidence. Desktop GUI, terminal UI and CLI. Reads files only. | Not started | - |
| `secretsentry` | SecretSentry | rust | Local-first secret scanner for source trees and Git history: finds likely credentials with tuned rules, redacts values in every output and tracks triage decisions. Desktop GUI, terminal UI and CLI. Never transmits or stores a full secret. | Not started | - |
| `phishlens` | PhishLens | rust | Safe, offline email analysis tool: parses .eml files, message headers, authentication results and URLs, scores suspicious signals with an explanation and keeps analyst notes. Desktop GUI, terminal UI and CLI. Never opens links or fetches content. | Not started | - |
| `certwatch` | CertWatch | go | Local-first certificate inventory and TLS posture monitor: tracks expiry, validates hostnames, inspects chains and keeps an audit history, with expiry alerts. Desktop GUI, terminal UI and CLI. Checks only hosts you are authorized to test. | Not started | - |
| `dns-sentinel` | DNS Sentinel | go | Local-first DNS change monitor for domains you own: tracks records, nameservers, DNSSEC status and expiry metadata, and alerts on unexpected changes with a full history. Desktop GUI, terminal UI and CLI. Read-only queries only. | Not started | - |
<!-- PROJECTS_TABLE_END -->

## Destinations

Seventeen projects will move to [Sonix-Devs](https://github.com/Sonix-Devs), and eight stay on [unalivin](https://github.com/unalivin).

<!-- DESTINATIONS_TABLE_START -->
| Folder | Language | Destination |
|---|---|---|
| `logforge-siem` | rust | Sonix-Devs |
| `netauto` | go | Sonix-Devs |
| `vulntrack` | go | Sonix-Devs |
| `forensicflow` | rust | Sonix-Devs |
| `ipam-pro` | go | Sonix-Devs |
| `securebaseline` | cpp | Sonix-Devs |
| `netgraph` | cpp | Sonix-Devs |
| `secops-copilot` | go | Sonix-Devs |
| `cloudfence` | go | Sonix-Devs |
| `incidentflow` | go | Sonix-Devs |
| `fleetops` | go | Sonix-Devs |
| `iac-sentinel` | rust | Sonix-Devs |
| `kubeguard` | go | Sonix-Devs |
| `netpulse` | go | Sonix-Devs |
| `sigmalab` | rust | Sonix-Devs |
| `sbom-navigator` | rust | Sonix-Devs |
| `pipelineshield` | go | Sonix-Devs |
| `routescope` | cpp | Personal (unalivin) |
| `threatatlas` | go | Personal (unalivin) |
| `portwatch` | go | Personal (unalivin) |
| `netconfig-auditor` | rust | Personal (unalivin) |
| `secretsentry` | rust | Personal (unalivin) |
| `phishlens` | rust | Personal (unalivin) |
| `certwatch` | go | Personal (unalivin) |
| `dns-sentinel` | go | Personal (unalivin) |
<!-- DESTINATIONS_TABLE_END -->

## Building

- **Go projects:** Go `1.27.4` (`make -C <project> check`)
- **Rust projects:** Stable Rust `1.83.0` (`make -C <project> check`)
- **C++ projects:** C++20, CMake, Ninja (`make -C <project> check`)

## Repository layout

- `scripts/` holds the project registry (`projects.tsv`), workflow generator, README sync, and verification scripts.
- `templates/` holds shared docs, pull request templates, workflow templates, and language shells.
- `docs/` holds [workspace layout](docs/WORKSPACE.md), [languages](docs/LANGUAGES.md), [adding a project](docs/ADD_PROJECT.md), [releasing](docs/RELEASING.md), and [roadmap](docs/ROADMAP.md).

## License

MIT. See [LICENSE](LICENSE).
