# Workspace: build 25 defensive security and networking tools in one repository

You are an autonomous senior engineering agent. Create one public GitHub repository named **`workspace`** that contains **25 independent projects, each in its own top-level folder**. Each folder is a complete, buildable, testable, releasable application that can later be split into its own repository. Work project by project, finish each one before starting the next, and keep the whole repository green.

Primary languages, in priority order: **Go (14 projects), Rust (8 projects), C++ (3 projects)**. **SQL** and **Bash** are used in every project. Do not use Python as a product language.

## Workspace repository

```text
workspace/
├── README.md
├── LICENSE
├── CONTRIBUTING.md
├── SECURITY.md
├── CODE_OF_CONDUCT.md
├── CHANGELOG.md
├── .editorconfig
├── .gitattributes
├── .gitignore
├── Makefile
├── scripts/
│   ├── list-projects.sh
│   ├── check-all.sh
│   ├── gen-workflows.sh
│   ├── new-project.sh
│   ├── split-project.sh
│   └── publish-github.sh
├── templates/
│   ├── go/
│   ├── rust/
│   ├── cpp/
│   └── workflows/
├── docs/
│   ├── WORKSPACE.md
│   ├── LANGUAGES.md
│   ├── ADD_PROJECT.md
│   └── RELEASING.md
├── .github/
│   ├── workflows/
│   ├── ISSUE_TEMPLATE/
│   └── PULL_REQUEST_TEMPLATE.md
├── certwatch/
├── dns-sentinel/
└── ... one folder per project in the table below
```

- The root `README.md` holds the project table (folder, app name, language, one-line description, status: Complete, In progress or Not started, kept truthful), build instructions per language and links to each project's docs.
- `templates/<language>/` holds the reusable skeleton for that language: launcher, settings table access, theme module, About and diagnostics, notifier, and test helpers. **Copy the template into each project; never import across project folders**, so any folder can be split into its own repository without changes. Fix a template bug in the template and in every project that already copied it.
- `scripts/new-project.sh <name> <go|rust|cpp>` creates a project folder from the template with its metadata filled in. `scripts/gen-workflows.sh` generates `.github/workflows/<project>-ci.yml` and `<project>-release.yml` from `templates/workflows/` with path filters (`<project>/**`) so a change to one project only runs that project's jobs. `scripts/list-projects.sh` prints the table. `scripts/check-all.sh [project...]` runs each project's `check` target. `scripts/publish-github.sh` creates the public `workspace` repository with `gh` (a `--dry-run` flag, a check that the logged-in user matches the owner, refusal of a non-noreply git email, push of `main`, enabling private vulnerability reporting, an optional tag push), and the workspace itself has the description "Monorepo of 25 local-first defensive security and network tools in Go, Rust and C++, each with a desktop GUI, terminal UI and CLI, SQLite storage and tag-triggered pre-releases." and exactly seven topics: `cybersecurity`, `defensive-security`, `network-security`, `monorepo`, `golang`, `rust`, `cpp`.
- All Bash scripts have bats tests with a fake `gh` and a local bare remote where they touch git or GitHub.
- A root CI job verifies that every project folder has every required file, that no email address appears in the repository, that no source file contains comments, and that the workflow files match what `gen-workflows.sh` generates.

## The 25 projects

| # | Folder | App | Primary language |
|---|---|---|---|
| 1 | `certwatch` | CertWatch | Go |
| 2 | `dns-sentinel` | DNS Sentinel | Go |
| 3 | `netpulse` | NetPulse | Go |
| 4 | `portwatch` | PortWatch | Go |
| 5 | `ipam-pro` | IPAM-Pro | Go |
| 6 | `netauto` | NetAuto | Go |
| 7 | `fleetops` | FleetOps | Go |
| 8 | `incidentflow` | IncidentFlow | Go |
| 9 | `cloudfence` | CloudFence | Go |
| 10 | `kubeguard` | KubeGuard | Go |
| 11 | `pipelineshield` | PipelineShield | Go |
| 12 | `vulntrack` | VulnTrack | Go |
| 13 | `threatatlas` | ThreatAtlas | Go |
| 14 | `secops-copilot` | SecOps-Copilot | Go |
| 15 | `secretsentry` | SecretSentry | Rust |
| 16 | `sbom-navigator` | SBOM-Navigator | Rust |
| 17 | `iac-sentinel` | IaC-Sentinel | Rust |
| 18 | `sigmalab` | SigmaLab | Rust |
| 19 | `logforge-siem` | LogForge-SIEM | Rust |
| 20 | `phishlens` | PhishLens | Rust |
| 21 | `forensicflow` | ForensicFlow | Rust |
| 22 | `netconfig-auditor` | NetConfig-Auditor | Rust |
| 23 | `routescope` | RouteScope | C++ |
| 24 | `netgraph` | NetGraph | C++ |
| 25 | `securebaseline` | SecureBaseline | C++ |

Go projects use Go `1.27.3`. Rust projects use one pinned stable toolchain. C++ projects use C++20.

## Language profiles

Every project has exactly one **primary language** (assigned in the project appendix). Language priority across the workspace is **1. Go, 2. Rust, 3. C++**, with **SQL** and **Bash** as first-class supporting languages in every project. Python is never part of a product; it may appear only as an optional developer helper with a clear reason.

| | Go (primary for 14 projects) | Rust (primary for 8 projects) | C++ (primary for 3 projects) |
|---|---|---|---|
| Toolchain | Go `1.27.3` in `go.mod` and every CI job | Stable Rust pinned in `rust-toolchain.toml` and every CI job (use the newest stable that exists when you start and record it) | C++20, CMake with Ninja, compilers GCC, Clang and MSVC; dependencies pinned through a vcpkg manifest with a baseline, or vendored sources with recorded hashes |
| Layout | `cmd/`, `internal/`, `tests/`, `testdata/` | Cargo workspace: `crates/<name>-core`, `-app`, `-tui`, `-gui` | `src/core`, `src/app`, `src/tui`, `src/gui`, `tests/`, `third_party/` |
| Desktop GUI | Fyne | egui with eframe (MIT or Apache-2.0) | Dear ImGui (MIT) with GLFW (zlib) and OpenGL 3, fonts bundled |
| Terminal UI | One permissively licensed library such as Bubble Tea | ratatui with crossterm | FTXUI (MIT) |
| SQLite | `modernc.org/sqlite` or a cgo driver, with FTS5 where search is needed | `rusqlite` with the `bundled` feature and FTS5 | SQLite amalgamation vendored, compiled with FTS5 |
| No-GUI build | `-tags nogui` | `--no-default-features` (the `gui` feature off) | `-DPROJECT_GUI=OFF` |
| Format and lint | `gofmt`, `go vet`, `staticcheck` | `cargo fmt --check`, `cargo clippy -- -D warnings` | `clang-format --dry-run --Werror`, `clang-tidy`, `cppcheck` |
| Tests | `go test`, `-race`, native fuzzing | `cargo test`, `proptest`, `cargo fuzz` for parsers | Catch2 or doctest, libFuzzer for parsers, ASan and UBSan builds in CI, TSan for concurrent code |
| Supply chain | `govulncheck`, license review | `cargo audit`, `cargo deny` | pinned dependencies, `osv-scanner` or equivalent free tool, license review |
| Packaging | `fyne package`, pinned to an exact version of `fyne.io/tools/cmd/fyne` | `cargo-packager` or `cargo-bundle` pinned to an exact version, with a `build.rs` resource step that embeds the Windows icon | CPack to `.tar.gz`, a Windows `.rc` resource that embeds the icon, a macOS `.app` bundle inside the archive |

### SQL (first-class in every project)

- All persistence uses SQLite in WAL mode with a busy timeout, `PRAGMA foreign_keys=ON`, and `PRAGMA user_version` tracking the schema version.
- The schema lives in plain numbered files `migrations/0001_*.sql`, `0002_*.sql`, and so on, applied in order inside transactions, embedded into the binary (`go:embed`, `include_str!`, or a generated resource), and never edited after release (add a new migration instead).
- Queries live in `.sql` files or clearly named constants and are always parameterized. Reports and dashboard aggregates are SQL views or prepared queries so the dashboard, the terminal UI and the CLI read identical numbers.
- Use indexes deliberately, `STRICT` tables where the SQLite version allows, check constraints for enumerations, FTS5 virtual tables for text search, and triggers only where they enforce an invariant such as the audit hash chain.
- SQL tests are mandatory: apply every migration to an empty database, compare a schema snapshot, run `PRAGMA integrity_check` and `PRAGMA foreign_key_check`, assert with `EXPLAIN QUERY PLAN` that the hot queries use their indexes, and test every view against a small fixture with known answers.
- Never build SQL by string concatenation with user input. A test must fail if it finds string-built SQL in the source.

### Bash (first-class in every project)

- Every project has a `scripts/` folder. Scripts start with `#!/usr/bin/env bash` and `set -euo pipefail`, support `--help`, and support `--dry-run` where they change something. They are linted with `shellcheck`, formatted with `shfmt`, and tested with `bats-core`. Line endings are LF (`.gitattributes`: `*.sh text eol=lf`).
- Required scripts per project: `scripts/publish-github.sh`, `scripts/dev-setup.sh` (installs or checks the prerequisites for the primary language), `scripts/check.sh` (runs the full local quality suite), `scripts/demo.sh` (loads demo data and starts the application), and `scripts/package.sh` (builds the release archive for the current OS).
- Windows developers run the scripts through Git Bash, and every script also has a `make` target. The product itself never requires Bash at runtime.
- Bash scripts are developer tooling. No product feature may depend on them.

### Optional native components (Rust inside Go, C++ inside Go or Rust)

Use a second native language **only when a benchmark proves it is worth it** (at least a 2x gain in throughput or a clear memory reduction on a reproducible workload) and the appendix allows it. Rules: a pure-primary-language implementation always exists and is the default and the reference; the native component sits behind a build tag or feature and a narrow C ABI; a differential test and a fuzz test compare both implementations; it is built on all three CI runners; and the release uses it only if all three builds pass. If the benchmark does not justify it, do not ship it and record the measured result in an ADR. The primary language owns at least 70 percent of production source lines. Primary-language projects whose appendix lists no optional component do not get one.

## Rules that apply to every project

### Delivery constraints

- Every project is a **local-first desktop application with two front ends over one service layer**: a native desktop dashboard (GUI) and a full-screen terminal UI, plus a complete CLI. It is **not** a website, web app, SaaS or hosted dashboard. No browser frontend. Any optional listener is disabled by default and binds to loopback only.
- The complete core workflow runs for **EUR 0** on a normal developer computer: free and open-source tools only, no paid API keys, subscriptions, cloud accounts, hosted databases, domains or paid feeds. External integrations are optional adapters with a fully usable offline demo mode and fixtures.
- Linux, macOS and Windows are supported. Do not claim a platform is supported until its build and test path ran. Report gaps honestly.
- No telemetry. Disclose every network request. Redact secrets in logs. Use OS-appropriate config directories with restrictive permissions. Provide dry-run for risky operations. Authorized-use only: scanners and probes act only on explicitly configured scope, and active checks need explicit confirmation.
- **No source-code comments** in any active or test source file (Go, Rust, C++, SQL, Bash). Express intent through names, small modules, tests and external docs.
- No fabricated features, metrics, benchmarks, test results or integrations. Every visible control performs a real action. Demo data is labeled.
- MIT license, subject to dependency review. Never claim third-party code is MIT.

### Engineering workflow: correctness first

1. Do not generate everything in one pass. Write requirements, acceptance criteria, threat model and a prioritized plan per project, then implement small complete vertical slices that compile and run.
2. After each meaningful change run the formatter, compiler, linter, unit and integration tests, the SQL tests, `shellcheck` and `bats`. Fix failures properly. Never suppress warnings or failures to get green.
3. Defensive engineering: validate untrusted input and paths, bounded concurrency and memory, timeouts, cancellation, no silent error swallowing, no hardcoded secrets.
4. Test valid, invalid, empty, boundary, malformed, oversized, interrupted, timed-out, offline, permission-denied and partial-failure cases with deterministic fixtures. Add fuzz or property tests for every parser.
5. Keep a requirements-to-tests checklist per project. Add a regression test for every bug found.
6. Profile before optimizing and benchmark only reproducible workloads. Never invent numbers.
7. Security review per project: threat model, dependency and license review, secret scan, path traversal, command injection, unsafe deserialization, data leakage, unintended network exposure.
8. A project is **Complete** only when its CI is green on all three operating systems, its documentation matches the implementation, and its quality report is honest. Never describe a platform as verified because a job was defined.

### Application shell (identical in every project)

- **Launcher.** Running with no arguments lets the user choose Dashboard or Terminal. From a desktop launch show a small native chooser titled "How do you want to open <App name>?" with **Dashboard (graphical window)**, **Terminal (text interface)**, a **Remember my choice** checkbox and Quit. From a terminal print a numbered prompt (1 Dashboard window, 2 Terminal UI) with the remembered or default choice on Enter. On a headless session skip the prompt and open the TUI with a one-line reason. Flags and environment: `--ui gui|tui`, `--ask`, `--no-prompt`, and `<APP>_UI`. Any explicit subcommand never shows the chooser. Non-interactive stdin and CI never prompt. Both front ends offer a menu action to relaunch in the other mode. Table-driven tests cover the launcher decision.
- **Navigation.** Left sidebar with an icon beside every label, collapsible to icons with tooltips, active item marked by shape and weight as well as color, About and Settings at the bottom. Top bar with global search, a data-mode chip when relevant, a notification bell and a help entry. Keyboard: Ctrl or Cmd plus 1 to 9 for categories, Ctrl or Cmd+F search, Ctrl or Cmd+comma Settings, F1 Usage guide. Persist window size, position, sidebar state, theme and table preferences.
- **Tables are the primary surface:** sortable, filterable, column chooser, row detail side panel, keyboard row navigation, copy and export, compact and comfortable density, virtualization for large data. Charts show real stored data only, with labeled axes, units and a text or table alternative. Every screen has meaningful loading, empty (with the next action), error (with the fix) and success states.
- **Avoid:** gradients, glow, glassmorphism, emoji as icons, stock illustrations, identical rounded card grids everywhere, decorative sparklines, random or hardcoded numbers, donut charts for two categories, placeholder text and dead controls.
- **Icons.** One consistent permissively licensed set embedded as SVG (Lucide under ISC or Tabler under MIT, recorded in `THIRD_PARTY_NOTICES.md`), never mixed, never emoji. The icon names in the appendix tables are Lucide-style suggestions.
- **Custom application icon (required).** An original icon per project from the concept in its appendix: editable SVG source, a simplified 16 px version, `.ico`, `.icns`, PNG sizes 16 to 1024, Linux SVG and PNG with a desktop entry. Used for the window, taskbar or dock, the launcher chooser and About. No third-party trademarks, no generic placeholder, no stock art.
- **About** (sidebar and Help menu) with tabs Overview, Usage guide (renders `docs/USAGE.md` offline, embedded at build time, with table of contents and search), Shortcuts, Diagnostics and Licenses and notices. Central metadata (one module per project) holds the repository URL and is the single source for every link. **Source Code** opens the configured repository, and **Found a bug? Please report it on GitHub.** opens the new-issue page after showing a preview of exactly what will be included (version, OS, architecture, build info, sanitized settings status). External links open only on an explicit click, only for URLs under the configured repository, after a confirmation that shows the full URL.
- **Accessibility:** full keyboard operation, visible focus, text scaling, reduced motion, labeled controls, no information by color alone, WCAG AA contrast (4.5:1 text, 3:1 components).
- **Notifications.** In the app: a bell with an unread count and a Notifications category (icon `bell`) with severity icon plus text, filters, mark-as-read and links to records; rules per event type with severity threshold, quiet hours and de-duplication. Outside the app (opt-in): `<app> notifier run|install|uninstall|status`, a per-user autostart entry without administrator rights (Windows Startup or Task Scheduler, macOS LaunchAgent, Linux XDG autostart), off by default, read-only database access on a bounded interval, no network calls of its own. Notification text is minimal by default and never contains secrets, credentials, evidence content or full indicators. Test rule evaluation, de-duplication, quiet hours and install and uninstall with an injected notifier and a temporary directory, and state that real OS display is unverified unless CI proves it.

### A. Everything is configurable inside the application

- A user can do every setup and configuration task from the dashboard without a terminal: first-time setup, creating the database, choosing its location, adding, editing, enabling, disabling and removing every object the product manages, imports and exports, notification rules, the notifier, the launcher preference, density, units, and every option that has a command-line flag.
- A terminal user can do the same from the TUI and from the CLI. Build a capability matrix in `docs/COMMANDS.md` with one row per capability and columns Dashboard, Terminal UI and CLI, and a test that fails if a capability is missing from a column that applies.
- No setting exists only in a hand-edited file or an environment variable. Flags and environment variables may override a setting, and the interface shows when a value is overridden and by what.
- Settings are validated before saving, show the error and the fix, can be reset one by one and are never silently dropped. Destructive actions ask for confirmation with a short statement of what changes and offer a dry run where sensible.

### B. Dashboard and terminal stay in sync

- Settings and data live in the local SQLite database (settings table). Only the database path and the saved front-end choice live in the OS user config directory with restrictive permissions.
- A change in one front end appears in the other and in the CLI. While both are open, the dashboard notices external changes within about two seconds (poll `PRAGMA data_version`) and refreshes the visible view without losing selection, scroll position or unsaved input, and shows an "updated by another window" notice. The TUI refreshes on its interval and on a key press.
- Two writers never corrupt data or crash. If the database is busy, show a notice and retry. Tests change a setting and an object through one front end's service calls and observe them through the other, including conflicting edits.

### C. Theming

- Light, Dark and Follow system, plus at least six named color themes with different accent and surface palettes: neutral slate, blue, green, red or crimson, amber, purple and a high-contrast theme. A theme is data (a named set of tokens: color, spacing, type scale, radius, elevation) in one theme module. The toolkit's default look must be genuinely replaced.
- A custom accent color picker that checks contrast and adjusts or warns to keep WCAG AA. Severity and status colors never depend on the accent, always pair an icon and a text label, and stay distinguishable for color-vision deficiency.
- Settings also cover density, text size, reduced motion and table columns, applied at once and stored in the settings table.
- The TUI offers the same themes mapped to ANSI colors, supports 16-color terminals, honors `NO_COLOR` and has a monochrome mode. `<app> config theme list|set|show` exists. Tests: every theme meets the contrast rules, switching changes the tokens in use, the setting is saved and loaded.

### D. Repository, commit and security-contact standards (per project folder)

- Each project folder contains: `README.md` (Download, Install, Usage, Documentation, Security, Contributing, License), `CHANGELOG.md` (Keep a Changelog, starting at `0.1.0-dev`), `CONTRIBUTING.md` plus `docs/CONTRIBUTING.md`, `CODE_OF_CONDUCT.md`, `SECURITY.md`, `LICENSE`, `Makefile` (`fmt`, `fmt-check`, `lint`, `test`, `race-or-sanitize`, `build`, `build-nogui`, `check`, `clean`), `.gitignore`, `.gitattributes`, `.env.example`, `THIRD_PARTY_NOTICES.md`, `migrations/`, `scripts/`, `assets/`, `testdata/`, and `docs/` with `README.md` (index), `INSTALLATION`, `USAGE`, `COMMANDS`, `CONFIGURATION`, `TROUBLESHOOTING`, `SECURITY`, `THREAT_MODEL`, `PLATFORM_SUPPORT`, `TESTING`, `ARCHITECTURE`, `ROADMAP`, `RELEASING`, `RELEASE_NOTES`, `LICENSES` and `adr/`. Docs describe what is implemented, not what is planned. No development-process notes (rounds, run IDs, sandbox logs) in any doc.
- `CONTRIBUTING.md` requires Conventional Commits and lists `feat`, `fix`, `docs`, `test`, `refactor`, `perf`, `build`, `ci`, `chore`, `revert` with one example each, the allowed scopes (the project's module names), the breaking-change footer, a branch naming rule and a review checklist. In the workspace, commit scopes start with the project folder name, for example `feat(certwatch): add expiry thresholds`.
- `SECURITY.md` structure: Supported versions, Reporting channel (the GitHub private vulnerability advisory form of the repository, with its link), What to include, Response expectations, Scope, Disclosure. **No email address appears anywhere in the repository.** A test enforces it. Only GitHub noreply addresses in git metadata are allowed. Never invent a contact address.
- Every project is at version `0.1.0-dev` from the first commit, prepared as if the first release were being published now. Commits use the GitHub noreply identity and never a personal email. The workspace's first commit is `chore: initial public release`.
- Every project description is one sentence of about 200 to 280 characters (what it does, main inputs and outputs, interfaces), without marketing words, and has exactly seven lower-case hyphenated topics, both given in the appendix.

### E. Release: tag-triggered pre-release called "dev beta"

- The workspace monorepo has one release workflow per project, `.github/workflows/<project>-release.yml`, named `Release <project>`, triggered by tags matching `<project>/v*` (for example `certwatch/v0.1.0-dev`). Nothing else publishes a release. No draft or manually triggered release workflow.
- Each builds the GUI on native runners (Linux x64 on `ubuntu-latest`, macOS arm64 on `macos-latest`, Windows x64 on `windows-latest`) with the packaging tool of the project's language profile, runs the tests on each runner first, and publishes exactly **three files** with a release description taken from `<project>/docs/RELEASE_NOTES.md`: `<project>-linux-x64.tar.gz`, `<project>-macos-arm64.tar.gz` and `<project>-windows-x64.exe` (a plain `.exe`, not a zip). No `SHA256SUMS` and no other file. `docs/RELEASING.md` explains how to compare the digest GitHub shows (`sha256sum`, `shasum -a 256`, `Get-FileHash`). The archives contain the app, the license files of the application and its dependencies, a short README and the usage guide.
- A separate publish job with `contents: write` checks that the tag points at the workflow commit, that no release with this tag exists and that exactly the three files are present, then runs `gh release create "$GITHUB_REF_NAME" <three files> --prerelease --title "<App name> v0.1.0-dev (dev beta)" --notes-file <project>/docs/RELEASE_NOTES.md`. It never overwrites a release.
- `docs/RELEASE_NOTES.md` says what the application does, that this is an early dev beta (`0.1.0-dev`), that not every planned feature exists, lists the three downloads, the digest check and how to open unsigned binaries (macOS: right-click, Open; Windows: More info, Run anyway; Linux: `chmod +x`). Do not claim signing or notarization.
- `docs/RELEASING.md` documents the maintainer steps from a phone (Termux) or desktop: confirm CI is green, update changelog and notes, `git tag <project>/v0.1.0-dev`, `git push origin <project>/v0.1.0-dev` (create the tag with git, not the GitHub release form), watch the run, verify the pre-release, recover with `gh release delete <tag> --cleanup-tag --yes`. Also build and test the no-GUI variant in CI without attaching it.
- To publish a project as its own repository later, `scripts/split-project.sh <project>` uses `git subtree split`, restores that project's own `.github/workflows` from `templates/`, and leaves the monorepo untouched. In a split repository the tag is plain `v*`.

### F. Requirement audit and honest reporting

- Per project, finish with a **requirement audit table**: one row per numbered requirement, status (implemented, partial, not implemented), the file or test that proves it, and the exact command run. Anything not run is reported as not run.
- Domain and service layers contain real logic with unit tests and a fuzz or property test where input is parsed. No padding to reach a size.
- Finish with exact commands and outcomes, what could not be verified in your environment (GUI on a real desktop, the macOS runner, OS notification display), and ranked next steps.

### Building the GUI when the sandbox is limited

If the sandbox cannot download dependencies, install graphics libraries or open a display, do not skip the GUI and do not fake verification. Use GitHub Actions: keep the no-GUI build fully tested; build the GUI on all three runners; install Linux system packages from the toolkit's official documentation at the time and record them; render every main screen headlessly with demo data (Fyne's software renderer, egui or Dear ImGui offscreen rendering where available) and upload the PNG files as workflow artifacts, treated as design evidence only; read CI failures through check-run annotations by making failing steps print GitHub error annotations. If every job fails at startup with a billing or spending-limit message, stop at once, say so in one line and do not keep pushing. If only macOS jobs cannot start, continue and mark macOS unverified everywhere. **Do not finish a project while any job on its final commit is red.** After the last push check every job on every operating system, fix the real cause of each red job, push and check again. If a job still fails, say so at the top of the final report with the job name and exact error and do not call the project finished.

## Project appendix

Every project also follows all language profiles and rules above. The topics below replace the old language topic with the one that matches the primary language.

### CertWatch — TLS Certificate Monitoring

- **Folder and repository name:** `certwatch`
- **App name:** CertWatch
- **Primary language:** Go (owns: TLS handshakes, certificate parsing, scheduling, alert delivery)
- **Optional native component:** none.
- **Persistence:** SQLite, with the schema in SQL migrations
- **GitHub description:** Local-first certificate inventory and TLS posture monitor: tracks expiry, validates hostnames, inspects chains and keeps an audit history, with expiry alerts. Desktop GUI, terminal UI and CLI. Checks only hosts you are authorized to test.
- **GitHub topics (exactly 7):** `certificate-management`, `tls`, `certificate-monitoring`, `x509`, `cybersecurity`, `defensive-security`, `golang`
- **Product mission:** Build TLS certificate monitoring for configured hostnames. Record expiry, issuer, SANs, chain validation, hostname match, supported protocol/configuration observations, history, alerts, and reports. Include local certificate fixtures and never disable production TLS verification.
- **Icon concept:** a certificate sheet with a ribbon seal whose ring doubles as a clock face
- **Notification events:** a certificate crosses an expiry threshold; a chain or hostname validation fails; a certificate changes between observations

Interface for CertWatch:

| Category | Icon (Lucide-style name) | What it shows |
|---|---|---|
| Overview | `layout-dashboard` | Certificates expiring in 7, 30, and 90 days, failed validations, hostname mismatches, soonest-expiry table |
| Certificates | `badge-check` | Certificate inventory with issuer, SANs, validity, and history |
| Endpoints | `globe` | Monitored hostnames and their last observation |
| Expiry Timeline | `calendar-clock` | Calendar-style timeline of upcoming expirations |
| Chains | `link-2` | Chain validation results and issuer relationships |
| Configuration | `sliders-horizontal` | Observed protocol and configuration findings |
| Alerts | `bell` | Alert history and thresholds |
| Reports | `file-text` | Exported certificate reports |
| About | `info` | App identity, offline Usage guide, Source Code, bug reporting |
| Settings | `settings-2` | Appearance, density, launch preference, data location, safe defaults |

**Overview screen (first screen after launch).** Answer "what needs attention now" with dense, information-first content computed from the real data store:

- Expiry buckets (7/30/90 days) as clickable counts
- Failed chain or hostname validations
- Soonest-expiring certificates table
- Recent alert history

**Signature views.**

- Expiry timeline with zoom
- Certificate detail: SANs, issuer, chain, and a change history across observations

### DNS Sentinel — DNS Change Monitoring

- **Folder and repository name:** `dns-sentinel`
- **App name:** DNS Sentinel
- **Primary language:** Go (owns: DNS queries, snapshot comparison, scheduled checks, notifications)
- **Optional native component:** none.
- **Persistence:** SQLite, with the schema in SQL migrations
- **GitHub description:** Local-first DNS change monitor for domains you own: tracks records, nameservers, DNSSEC status and expiry metadata, and alerts on unexpected changes with a full history. Desktop GUI, terminal UI and CLI. Read-only queries only.
- **GitHub topics (exactly 7):** `dns`, `dns-monitoring`, `dnssec`, `domain-monitoring`, `cybersecurity`, `defensive-security`, `golang`
- **Product mission:** Build DNS inventory and monitoring for configured domains. Track A/AAAA/CNAME/MX/TXT/NS/SOA records, snapshots, unexpected changes, DNSSEC observations when available, expiry metadata, expected-change allowlists, notifications, history, imports, and exports. Never treat a DNS change alone as proof of compromise.
- **Icon concept:** a globe drawn as meridian lines with a small sentinel notch at the top
- **Notification events:** an unexpected DNS change is detected; a domain nears expiry

Interface for DNS Sentinel:

| Category | Icon (Lucide-style name) | What it shows |
|---|---|---|
| Overview | `layout-dashboard` | Expected versus unexpected changes (24 hours, 7 days), domain check age, DNSSEC observations, nearest expiry |
| Domains | `globe` | Configured domains and last check |
| Records | `list-tree` | A, AAAA, CNAME, MX, TXT, NS, SOA records per domain |
| Changes | `git-compare` | Detected changes with expected-change matching |
| Snapshots | `history` | Stored snapshots and comparison |
| Allowlist | `list-checks` | Expected-change rules |
| Alerts | `bell` | Notification history |
| Import / Export | `arrow-left-right` | Imports and exports |
| About | `info` | App identity, offline Usage guide, Source Code, bug reporting |
| Settings | `settings-2` | Appearance, density, launch preference, data location, safe defaults |

**Overview screen (first screen after launch).** Answer "what needs attention now" with dense, information-first content computed from the real data store:

- Expected versus unexpected changes over recent windows
- Domains ranked by age of last successful check
- DNSSEC observations where available
- Domains nearest to expiry

**Signature views.**

- Side-by-side snapshot diff per record type
- Change detail worded as a change observed, never as proof of compromise

### NetPulse — Network Observability

- **Folder and repository name:** `netpulse`
- **App name:** NetPulse
- **Primary language:** Go (owns: pollers, scheduling, metrics ingestion, alert evaluation)
- **Optional native component:** Rust, optional only if measured: metric batch decoding. Follow the optional native component rules.
- **Persistence:** SQLite + Prometheus, with the schema in SQL migrations
- **GitHub description:** Local-first network observability tool: measures latency, packet loss, uptime and bandwidth for targets you configure, evaluates alert rules and builds incident timelines. Desktop GUI, terminal UI and CLI. Checks only the targets you add.
- **GitHub topics (exactly 7):** `network-monitoring`, `observability`, `uptime-monitoring`, `latency`, `alerting`, `networking`, `golang`
- **Product mission:** Build a network observability system with configurable ICMP/TCP/HTTP checks, latency and packet-loss measurements, time-series metrics, SLOs, alert thresholds, deduplication, silencing, maintenance windows, incident timelines, status pages, and historical charts. Include simulated targets and prevent overlapping polling jobs.
- **Icon concept:** a pulse waveform inside a ring
- **Notification events:** an alert fires or resolves; SLO burn crosses a threshold

Interface for NetPulse:

| Category | Icon (Lucide-style name) | What it shows |
|---|---|---|
| Overview | `layout-dashboard` | Target availability, latency percentiles, packet loss, SLO burn, active alerts |
| Targets | `target` | Targets and check configuration |
| Checks | `activity` | ICMP, TCP, and HTTP check results |
| Metrics | `chart-line` | Time-series explorer |
| SLOs | `gauge` | SLO status and burn |
| Alerts | `bell` | Active and historical alerts |
| Silences | `bell-off` | Silences and maintenance windows |
| Incidents | `history` | Incident timelines |
| Status Page | `panel-top` | Local status page preview and static export |
| About | `info` | App identity, offline Usage guide, Source Code, bug reporting |
| Settings | `settings-2` | Appearance, density, launch preference, data location, safe defaults |

**Overview screen (first screen after launch).** Answer "what needs attention now" with dense, information-first content computed from the real data store:

- Availability per target from stored measurements
- Latency percentile chart
- Packet loss and SLO burn
- Active alerts (simulated targets labeled)

**Signature views.**

- Metric explorer with time-range zoom and overlay
- Status page preview rendered inside the app and exportable as a static file; no web server

### PortWatch — Authorized Service Exposure Monitor

- **Folder and repository name:** `portwatch`
- **App name:** PortWatch
- **Primary language:** Go (owns: tightly scoped checks, concurrency limits, timeouts, state comparison)
- **Optional native component:** none.
- **Persistence:** SQLite, with the schema in SQL migrations
- **GitHub description:** Local-first exposure change monitor: records open ports and service metadata for assets explicitly in scope, compares runs and alerts on new or changed exposure. Desktop GUI, terminal UI and CLI. Scans only authorized, scoped targets.
- **GitHub topics (exactly 7):** `port-scanning`, `attack-surface`, `exposure-monitoring`, `change-detection`, `cybersecurity`, `defensive-security`, `golang`
- **Product mission:** Build a controlled service-exposure monitor for explicitly authorized assets. Enforce scope, rate limits, timeouts, record service state changes and timestamps, support scheduled runs, and compare service metadata conservatively. Include simulation mode; no stealth, exploitation, or internet-wide scanning.
- **Icon concept:** a port socket silhouette with a radar arc
- **Notification events:** an unexpected service change is detected; a scheduled run fails

Interface for PortWatch:

| Category | Icon (Lucide-style name) | What it shows |
|---|---|---|
| Overview | `layout-dashboard` | Services by state, changes since baseline, scope coverage, last and next run, rate-limit status |
| Authorized Targets | `target` | Targets with recorded authorization and scope |
| Runs | `radar` | Run history |
| Services | `server` | Observed service state |
| Changes | `git-compare` | State changes with timestamps |
| Schedules | `calendar-clock` | Scheduled runs |
| Baselines | `list-checks` | Baselines |
| Audit | `scroll-text` | Audit trail |
| About | `info` | App identity, offline Usage guide, Source Code, bug reporting |
| Settings | `settings-2` | Appearance, density, launch preference, data location, safe defaults |

**Overview screen (first screen after launch).** Answer "what needs attention now" with dense, information-first content computed from the real data store:

- Services by state
- Changes since baseline
- Scope coverage
- Last and next run with rate-limit status (simulation badge when simulated)

**Signature views.**

- Scope manager that refuses any target without recorded authorization and says why
- Change timeline per service

### IPAM-Pro — IP Address Management

- **Folder and repository name:** `ipam-pro`
- **App name:** IPAM-Pro
- **Primary language:** Go (owns: IPv4/IPv6/CIDR domain logic and allocation rules)
- **Optional native component:** none.
- **Persistence:** SQLite, with the schema in SQL migrations
- **GitHub description:** Local-first IPv4 and IPv6 address manager: plans subnets and CIDR blocks, tracks VLANs, allocations and reservations, and detects conflicts and overlaps before they cause an outage. Desktop GUI, terminal UI and CLI. No network scanning.
- **GitHub topics (exactly 7):** `ipam`, `ip-address-management`, `cidr`, `subnet-calculator`, `networking`, `ipv6`, `golang`
- **Product mission:** Build an IPv4/IPv6 IPAM system with CIDR calculations, nested prefixes, address pools, reservations, leases/imports, VLANs, locations, ownership, audit trails, utilization charts, conflict detection, bulk import/export, and RBAC. Test IPv6 and boundary cases extensively.
- **Icon concept:** a grid of address cells with a subnet bracket
- **Notification events:** a pool nears exhaustion; an address conflict is detected

Interface for IPAM-Pro:

| Category | Icon (Lucide-style name) | What it shows |
|---|---|---|
| Overview | `layout-dashboard` | Utilization by prefix, conflicts, nearly exhausted pools, recent changes |
| Prefixes | `network` | Nested prefix tree with utilization |
| Addresses | `list-ordered` | Address records, reservations, and leases |
| Pools | `layers` | Address pools |
| VLANs | `cable` | VLANs and their prefixes |
| Locations | `map-pin` | Locations and ownership |
| Conflicts | `triangle-alert` | Detected overlaps and duplicates |
| Calculator | `calculator` | CIDR calculator with IPv4 and IPv6 boundary handling |
| Import / Export | `arrow-left-right` | Bulk import and export |
| Audit | `scroll-text` | Audit trail |
| About | `info` | App identity, offline Usage guide, Source Code, bug reporting |
| Settings | `settings-2` | Appearance, density, launch preference, data location, safe defaults |

**Overview screen (first screen after launch).** Answer "what needs attention now" with dense, information-first content computed from the real data store:

- Utilization by top-level prefix as a real bar chart
- Open conflicts
- Pools nearing exhaustion
- Recent allocation and audit activity

**Signature views.**

- Nested prefix tree with inline utilization bars, IPv6 aware
- CIDR calculator whose results match the tested boundary cases

### NetAuto — Network Automation Workbench

- **Folder and repository name:** `netauto`
- **App name:** NetAuto
- **Primary language:** Go (owns: workflow orchestration, device adapters, dry runs, approvals, change logs)
- **Optional native component:** none.
- **Persistence:** SQLite, with the schema in SQL migrations
- **GitHub description:** Local-first network automation tool: backs up device configurations, runs scheduled checks, applies approved change workflows and produces change reports with an audit trail. Desktop GUI, terminal UI and CLI. Nothing changes without approval.
- **GitHub topics (exactly 7):** `network-automation`, `network-configuration`, `config-backup`, `change-management`, `networking`, `devops`, `golang`
- **Product mission:** Build network automation workflows with inventory, reusable jobs, dry-run previews, approval gates, encrypted credential storage, execution logs, configuration backups, diff views, rollback guidance, and adapters for documented devices. Use mocks and lab targets by default.
- **Icon concept:** a network node with a looping arrow
- **Notification events:** an approval is pending; a run fails; a device backup becomes stale

Interface for NetAuto:

| Category | Icon (Lucide-style name) | What it shows |
|---|---|---|
| Overview | `layout-dashboard` | Pending approvals, recent run outcomes, backup freshness, dry-run versus executed counts, lab or mock mode |
| Inventory | `router` | Devices and adapters |
| Jobs | `workflow` | Reusable jobs |
| Runs | `play` | Run history and execution logs |
| Approvals | `user-check` | Approval gates |
| Backups | `archive` | Configuration backups |
| Diffs | `git-compare` | Diff views |
| Credentials | `key-round` | Encrypted credential store (values never displayed) |
| Lab | `flask-conical` | Mock and lab targets |
| Change Log | `history` | Change history |
| About | `info` | App identity, offline Usage guide, Source Code, bug reporting |
| Settings | `settings-2` | Appearance, density, launch preference, data location, safe defaults |

**Overview screen (first screen after launch).** Answer "what needs attention now" with dense, information-first content computed from the real data store:

- Approvals waiting
- Recent run outcomes
- Devices by backup freshness
- Dry-run versus executed counts, with a Mock or Lab badge

**Signature views.**

- Dry-run preview with diff and an approval gate before execution
- Rollback guidance panel attached to each run

### FleetOps — Linux Fleet Inventory and Health

- **Folder and repository name:** `fleetops`
- **App name:** FleetOps
- **Primary language:** Go (owns: host inventory, metrics, scheduled checks, approval-gated jobs)
- **Optional native component:** none.
- **Persistence:** SQLite, with the schema in SQL migrations
- **GitHub description:** Local-first Linux fleet manager: keeps a server inventory, runs health checks, tracks patch status and metrics, and schedules maintenance tasks with an audit trail. Desktop GUI, terminal UI and CLI. Changes only hosts you configure.
- **GitHub topics (exactly 7):** `fleet-management`, `server-management`, `patch-management`, `monitoring`, `devops`, `golang`, `fyne`
- **Product mission:** Build Linux fleet operations with inventory, health, disk/memory/CPU metrics, patch status, scheduled checks, maintenance windows, RBAC, audit trails, and safe job execution. Start read-only and require explicit approval for state-changing operations. Include simulated hosts.
- **Icon concept:** a row of rack units whose indicator dots form a compass point
- **Notification events:** a host becomes unhealthy; a job awaits approval; a job fails; a maintenance window starts

Interface for FleetOps:

| Category | Icon (Lucide-style name) | What it shows |
|---|---|---|
| Overview | `layout-dashboard` | Hosts by health state, CPU, memory, and disk distributions, patch status, pending approvals, upcoming maintenance |
| Hosts | `server` | Inventory with per-host detail |
| Health | `activity` | Health checks and metric history |
| Patches | `package-check` | Patch status per host |
| Jobs | `play-circle` | Job definitions and run history |
| Approvals | `user-check` | Pending approvals for state-changing jobs |
| Maintenance | `calendar-range` | Maintenance windows |
| Audit | `scroll-text` | Audit trail |
| About | `info` | App identity, offline Usage guide, Source Code, bug reporting |
| Settings | `settings-2` | Appearance, density, launch preference, data location, safe defaults |

**Overview screen (first screen after launch).** Answer "what needs attention now" with dense, information-first content computed from the real data store:

- Hosts by health state
- CPU, memory, and disk distributions computed from stored metrics
- Patch status summary
- Jobs waiting for approval and upcoming maintenance windows (simulated hosts labeled)

**Signature views.**

- Host detail with metric time series
- Approval dialog listing the exact commands and target hosts before a state-changing job can run

### IncidentFlow — Security Incident Management

- **Folder and repository name:** `incidentflow`
- **App name:** IncidentFlow
- **Primary language:** Go (owns: workflow state transitions, permissions, evidence metadata, audit events)
- **Optional native component:** none.
- **Persistence:** SQLite, with the schema in SQL migrations
- **GitHub description:** Local-first incident management: tracks security alerts, cases, evidence, timelines, assignments and response checklists, and produces post-incident reports from stored records. Desktop GUI, terminal UI and CLI. Everything stays in one local file.
- **GitHub topics (exactly 7):** `incident-response`, `incident-management`, `case-management`, `dfir`, `cybersecurity`, `golang`, `fyne`
- **Product mission:** Build incident response workflows with cases, severity/status transitions, ownership, evidence metadata and hashes, timelines, task checklists, SLA tracking, audit history, role permissions, notifications, and reports. Preserve evidence integrity and keep evidence private.
- **Icon concept:** a beacon with concentric light rings rising from a rounded square
- **Notification events:** a new incident opens; an SLA is at risk or breached; a task is assigned or overdue

Interface for IncidentFlow:

| Category | Icon (Lucide-style name) | What it shows |
|---|---|---|
| Overview | `layout-dashboard` | Open incidents by severity and status, SLA at risk or breached, my tasks, recent timeline entries |
| Incidents | `siren` | Case list and case workspace |
| Timeline | `history` | Case timeline |
| Tasks | `list-checks` | Checklists and ownership |
| Evidence | `paperclip` | Evidence metadata and hashes, kept private |
| SLA | `timer` | SLA clocks and breaches |
| People & Roles | `users` | Responders and role permissions |
| Reports | `file-text` | Incident reports |
| Audit | `scroll-text` | Audit history |
| About | `info` | App identity, offline Usage guide, Source Code, bug reporting |
| Settings | `settings-2` | Appearance, density, launch preference, data location, safe defaults |

**Overview screen (first screen after launch).** Answer "what needs attention now" with dense, information-first content computed from the real data store:

- Open incidents by severity and status
- SLA at risk and breached
- Tasks assigned to the current user
- Recent timeline entries

**Signature views.**

- Incident detail with role-limited, validated status transitions
- Evidence list with hashes and integrity state

### CloudFence — Cloud Security Posture

- **Folder and repository name:** `cloudfence`
- **App name:** CloudFence
- **Primary language:** Go (owns: provider adapters, read-only inventory, policy evaluation, evidence capture)
- **Optional native component:** none.
- **Persistence:** SQLite, with the schema in SQL migrations
- **GitHub description:** Local-first cloud security posture tool: reads exported or read-only cloud configuration, flags risky settings with explainable severity, and tracks remediation over time. Desktop GUI, terminal UI and CLI. Never changes cloud resources.
- **GitHub topics (exactly 7):** `cloud-security`, `cspm`, `security-posture`, `misconfiguration`, `cybersecurity`, `defensive-security`, `golang`
- **Product mission:** Build a cloud posture platform with provider abstraction, read-only inventory for one provider first, policy rules, asset relationships, findings history, risk prioritization, remediation guidance, and evidence exports. Include mocked fixtures and least-privilege setup; never mutate cloud resources.
- **Icon concept:** a cloud outline closed by a row of fence posts
- **Notification events:** a new high or critical finding appears after a run; a run fails

Interface for CloudFence:

| Category | Icon (Lucide-style name) | What it shows |
|---|---|---|
| Overview | `layout-dashboard` | Findings by severity and resource type, posture trend across runs, top risky resources, run mode |
| Resources | `cloud` | Read-only inventory of provider resources |
| Policies | `scale` | Rule catalog with remediation guidance |
| Findings | `shield-alert` | Findings with history and prioritization |
| Relationships | `network` | Resource relationship view |
| Evidence | `folder-open` | Evidence captured per finding and export |
| Providers | `plug` | Least-privilege setup guidance and fixture mode |
| Reports | `file-text` | Evidence exports |
| About | `info` | App identity, offline Usage guide, Source Code, bug reporting |
| Settings | `settings-2` | Appearance, density, launch preference, data location, safe defaults |

**Overview screen (first screen after launch).** Answer "what needs attention now" with dense, information-first content computed from the real data store:

- Findings by severity and by resource type
- Posture trend across stored runs
- Top risky resources
- Last inventory run with a persistent Fixture or Live read-only badge

**Signature views.**

- Resource detail: relationships, findings history, evidence
- Policy detail with remediation guidance

### KubeGuard — Kubernetes Security Posture

- **Folder and repository name:** `kubeguard`
- **App name:** KubeGuard
- **Primary language:** Go (owns: YAML parsing, Kubernetes object analysis, policy evaluation, optional read-only API access)
- **Optional native component:** none.
- **Persistence:** SQLite optional; local scan should work without a database, with the schema in SQL migrations
- **GitHub description:** Local-first Kubernetes security posture analyzer: reads manifests, RBAC, pod security settings and network policies, ranks risky configuration with explainable rules and exports reports. Desktop GUI, terminal UI and CLI. Never touches a live cluster.
- **GitHub topics (exactly 7):** `kubernetes`, `kubernetes-security`, `rbac`, `security-posture`, `cybersecurity`, `defensive-security`, `golang`
- **Product mission:** Build a Kubernetes security analyzer for offline manifests and optional read-only cluster inspection. Check privileged containers, capabilities, host namespaces, secret references, RBAC scope, resource limits, image pinning, and network-policy coverage. Never modify clusters automatically.
- **Icon concept:** an outlined container cube closed by a guard bracket (do not use the Kubernetes logo or helm)
- **Notification events:** a scan finds a new critical issue; a scan fails

Interface for KubeGuard:

| Category | Icon (Lucide-style name) | What it shows |
|---|---|---|
| Overview | `layout-dashboard` | Findings by check type, namespaces by risk, image-pinning and network-policy coverage, scan mode |
| Manifests | `file-code-2` | Loaded manifests with highlighted findings |
| Workloads | `boxes` | Workloads and their security context |
| Findings | `shield-alert` | Findings with remediation |
| Policies | `scale` | Check catalog |
| RBAC | `key-round` | Role and binding scope analysis |
| Network Policies | `waypoints` | Network-policy coverage by namespace |
| Cluster | `plug` | Optional read-only cluster inspection |
| Reports | `file-text` | Exported reports |
| About | `info` | App identity, offline Usage guide, Source Code, bug reporting |
| Settings | `settings-2` | Appearance, density, launch preference, data location, safe defaults |

**Overview screen (first screen after launch).** Answer "what needs attention now" with dense, information-first content computed from the real data store:

- Findings by check type
- Namespaces ranked by risk
- Image-pinning coverage and network-policy coverage computed from the scan
- Offline or read-only-cluster mode badge

**Signature views.**

- Manifest viewer with findings highlighted at the offending lines
- RBAC scope view showing what each subject can reach

### PipelineShield — CI/CD Security Findings Hub

- **Folder and repository name:** `pipelineshield`
- **App name:** PipelineShield
- **Primary language:** Go (owns: result ingestion, normalization, deduplication, policy gates)
- **Optional native component:** none.
- **Persistence:** SQLite, with the schema in SQL migrations
- **GitHub description:** Local-first dashboard that merges SAST, dependency, secret and container scan results from CI/CD (SARIF and JSON), deduplicates findings, tracks status and exports reports. Desktop GUI, terminal UI and CLI. Imports results, runs no scans.
- **GitHub topics (exactly 7):** `devsecops`, `cicd`, `sast`, `sarif`, `vulnerability-management`, `cybersecurity`, `golang`
- **Product mission:** Build a CI/CD security hub that ingests SAST, dependency, secret, and container findings, normalizes/deduplicates issues, maps them to repositories/builds, tracks remediation, supports configurable quality gates, and includes example GitHub Actions workflows with least-privilege permissions.
- **Icon concept:** a pipeline segment passing through a gate bar
- **Notification events:** a quality gate fails; a new critical finding appears

Interface for PipelineShield:

| Category | Icon (Lucide-style name) | What it shows |
|---|---|---|
| Overview | `layout-dashboard` | Findings by source type, gate pass and fail by repository, trend, finding age |
| Repositories | `git-branch` | Repositories and their builds |
| Builds | `workflow` | Pipeline runs and mapped findings |
| Findings | `shield-alert` | Normalized and deduplicated findings |
| Gates | `shield-check` | Configurable quality gates |
| Remediation | `wrench` | Remediation tracking |
| Trends | `chart-line` | Trends over time |
| Sources | `upload` | SAST, dependency, secret, and container importers |
| About | `info` | App identity, offline Usage guide, Source Code, bug reporting |
| Settings | `settings-2` | Appearance, density, launch preference, data location, safe defaults |

**Overview screen (first screen after launch).** Answer "what needs attention now" with dense, information-first content computed from the real data store:

- Findings by source type (SAST, dependency, secret, container)
- Gate pass and fail by repository
- Trend over stored builds
- Finding age distribution

**Signature views.**

- Gate editor with live preview against stored data
- Finding detail showing deduplication provenance from each source

### VulnTrack — Vulnerability Management

- **Folder and repository name:** `vulntrack`
- **App name:** VulnTrack
- **Primary language:** Go (owns: import pipelines, normalization, enrichment, deduplication, risk scoring)
- **Optional native component:** Rust, optional only if measured: bulk scanner-report parsing. Follow the optional native component rules.
- **Persistence:** SQLite, with the schema in SQL migrations
- **GitHub description:** Local-first vulnerability management: imports scan findings, correlates assets and CVEs, prioritizes remediation with explainable risk scores and tracks fixes through tickets. Desktop GUI, terminal UI and CLI. Imports results, runs no scans.
- **GitHub topics (exactly 7):** `vulnerability-management`, `cve`, `risk-scoring`, `remediation`, `cybersecurity`, `defensive-security`, `golang`
- **Product mission:** Build vulnerability operations with scan-result importers, CVE enrichment from maintained sources, normalization, deduplication, asset mapping, severity/exploitability context, SLA policies, remediation ownership, exceptions, retest status, and audit reports. Do not claim unverified findings are confirmed.
- **Icon concept:** a hex bolt with a tick mark
- **Notification events:** a new critical vulnerability is imported; an SLA is at risk or breached; a retest is due

Interface for VulnTrack:

| Category | Icon (Lucide-style name) | What it shows |
|---|---|---|
| Overview | `layout-dashboard` | Open vulnerabilities by severity and exploitability context, SLA breached and at risk, aging, owner workload |
| Vulnerabilities | `bug` | Normalized, deduplicated findings |
| Assets | `server` | Asset mapping |
| Imports | `upload` | Scanner result importers |
| Enrichment | `database-zap` | CVE enrichment from maintained sources |
| SLA | `timer` | SLA policies and clocks |
| Remediation | `wrench` | Ownership and progress |
| Exceptions | `file-warning` | Exceptions with expiry |
| Retests | `rotate-ccw` | Retest status |
| Reports | `file-text` | Audit reports |
| About | `info` | App identity, offline Usage guide, Source Code, bug reporting |
| Settings | `settings-2` | Appearance, density, launch preference, data location, safe defaults |

**Overview screen (first screen after launch).** Answer "what needs attention now" with dense, information-first content computed from the real data store:

- Open vulnerabilities by severity with exploitability context
- SLA breached and at risk
- Aging distribution
- Workload by owner

**Signature views.**

- Finding detail with a status vocabulary that separates reported from confirmed
- SLA clock with the policy that produced it

### ThreatAtlas — Threat Intelligence Workspace

- **Folder and repository name:** `threatatlas`
- **App name:** ThreatAtlas
- **Primary language:** Go (owns: feed ingestion, parsing, deduplication, provenance, exports)
- **Optional native component:** none.
- **Persistence:** SQLite, with the schema in SQL migrations
- **GitHub description:** Local-first threat intelligence workbench: imports indicators (IPs, domains, URLs, hashes), deduplicates and scores them with sources and confidence, and links them to cases. Desktop GUI, terminal UI and CLI. Makes no outbound lookups by default.
- **GitHub topics (exactly 7):** `threat-intelligence`, `ioc`, `indicators-of-compromise`, `stix`, `cybersecurity`, `defensive-security`, `golang`
- **Product mission:** Build a threat-intelligence workspace that imports configured feeds and files, tracks provenance and timestamps, deduplicates indicators, scores confidence transparently, stores sightings, supports expiry/decay, and exports interoperable formats where feasible. Never automatically visit suspicious URLs or execute submitted files.
- **Icon concept:** an atlas globe with node points on its meridians
- **Notification events:** a feed goes stale; indicators are about to expire; a tracked indicator is sighted

Interface for ThreatAtlas:

| Category | Icon (Lucide-style name) | What it shows |
|---|---|---|
| Overview | `layout-dashboard` | Indicators by type and confidence band, feed freshness, expiring and decayed indicators, recent sightings |
| Indicators | `fingerprint` | Deduplicated indicators with confidence |
| Feeds | `rss` | Configured feeds and files, freshness |
| Sightings | `eye` | Recorded sightings |
| Provenance | `git-branch` | Source chain per indicator |
| Confidence | `gauge` | Transparent scoring and decay |
| Imports | `upload` | Feed and file imports |
| Exports | `file-output` | Interoperable export formats where feasible |
| About | `info` | App identity, offline Usage guide, Source Code, bug reporting |
| Settings | `settings-2` | Appearance, density, launch preference, data location, safe defaults |

**Overview screen (first screen after launch).** Answer "what needs attention now" with dense, information-first content computed from the real data store:

- Indicators by type and confidence band
- Feed freshness
- Indicators nearing expiry or decayed
- Recent sightings

**Signature views.**

- Indicator detail with provenance chain and the confidence score breakdown
- Decay preview showing when an indicator will expire

### SecOps-Copilot — Evidence-Grounded Security Assistant

- **Folder and repository name:** `secops-copilot`
- **App name:** SecOps-Copilot
- **Primary language:** Go (owns: policy enforcement, evidence retrieval, audit logging, provider interface with a deterministic mock mode)
- **Optional native component:** none.
- **Persistence:** SQLite, with the schema in SQL migrations
- **GitHub description:** Local-first SecOps assistant workspace: summarizes alerts, explains findings and drafts remediation steps and reports, each grounded in cited source records with an audit of every model call. Desktop GUI, terminal UI and CLI. You control the model.
- **GitHub topics (exactly 7):** `secops`, `security-operations`, `ai-assistant`, `incident-response`, `llm`, `cybersecurity`, `golang`
- **Product mission:** Build an AI-assisted SecOps workspace that summarizes alerts with citations to source records, drafts remediation, accepts analyst feedback, and audits model actions. Treat logs as untrusted data, resist prompt injection, redact secrets, require human approval for actions, and provide deterministic mock mode.
- **Icon concept:** a lens ring with a speech notch
- **Notification events:** an alert awaits triage; an approval is pending

Interface for SecOps-Copilot:

| Category | Icon (Lucide-style name) | What it shows |
|---|---|---|
| Overview | `layout-dashboard` | Alerts awaiting triage, pending approvals, citation coverage, mode indicator |
| Assistant | `messages-square` | Conversation panel where every claim carries citations |
| Alerts | `bell` | Alerts and their source records |
| Evidence | `folder-search` | Source records the assistant can cite |
| Approvals | `user-check` | Human approval of any proposed action |
| Policies | `shield` | Redaction and prompt-injection defenses |
| Providers | `plug` | Mock mode and provider configuration |
| Audit | `scroll-text` | Audit of every model action |
| About | `info` | App identity, offline Usage guide, Source Code, bug reporting |
| Settings | `settings-2` | Appearance, density, launch preference, data location, safe defaults |

**Overview screen (first screen after launch).** Answer "what needs attention now" with dense, information-first content computed from the real data store:

- Alerts awaiting triage
- Pending approvals
- Share of summary claims that carry a citation
- Persistent Mock or Provider mode badge

**Signature views.**

- Assistant panel with citation chips that open the exact source record, and an untrusted-data marker on log content
- Approval dialog for any proposed action; nothing executes without it

### SecretSentry — Secret Detection and Triage

- **Folder and repository name:** `secretsentry`
- **App name:** SecretSentry
- **Primary language:** Rust (owns: the scanning engine: detectors, entropy and pattern matching, Git history walking, redaction)
- **Optional native component:** C++, optional only if measured: multi-pattern matching. Follow the optional native component rules.
- **Persistence:** SQLite, with the schema in SQL migrations
- **GitHub description:** Local-first secret scanner for source trees and Git history: finds likely credentials with tuned rules, redacts values in every output and tracks triage decisions. Desktop GUI, terminal UI and CLI. Never transmits or stores a full secret.
- **GitHub topics (exactly 7):** `secret-scanning`, `secrets-detection`, `git-security`, `devsecops`, `cybersecurity`, `defensive-security`, `rust`
- **Product mission:** Build a high-signal secret scanner for source trees and Git history with detector rules, entropy checks where justified, test-fixture allowlists, contextual confidence, redaction, JSON/SARIF output, and CI integration. Never print full secrets by default or transmit repository contents externally.
- **Icon concept:** a key silhouette crossed by a redaction bar
- **Notification events:** a scan finds a new secret; a scan fails

Interface for SecretSentry:

| Category | Icon (Lucide-style name) | What it shows |
|---|---|---|
| Overview | `layout-dashboard` | Findings by detector and confidence, new versus baseline, top files, last scan |
| Scans | `scan-search` | Scan runs on trees and Git history |
| Findings | `key-round` | Findings with redacted values |
| Git History | `git-commit` | History findings by commit |
| Rules | `book-check` | Detector rules |
| Allowlists | `list-checks` | Test-fixture allowlists |
| Exports | `file-output` | JSON and SARIF |
| About | `info` | App identity, offline Usage guide, Source Code, bug reporting |
| Settings | `settings-2` | Appearance, density, launch preference, data location, safe defaults |

**Overview screen (first screen after launch).** Answer "what needs attention now" with dense, information-first content computed from the real data store:

- Findings by detector type and confidence
- New versus baseline findings
- Files with the most findings
- Last scan duration and scope

**Signature views.**

- Findings table that always shows secret values redacted
- Finding detail with context, confidence reasoning, and commit when applicable

### SBOM-Navigator — Software Supply Chain Inventory

- **Folder and repository name:** `sbom-navigator`
- **App name:** SBOM-Navigator
- **Primary language:** Rust (owns: CycloneDX and SPDX parsing, validation, component normalization, dependency graph generation)
- **Optional native component:** C++, optional only if measured: large dependency-graph traversal. Follow the optional native component rules.
- **Persistence:** SQLite, with the schema in SQL migrations
- **GitHub description:** Local-first SBOM explorer: imports CycloneDX and SPDX documents, shows the dependency graph, license metadata and vulnerability matches with explainable severity. Desktop GUI, terminal UI and CLI. Works offline on files you import.
- **GitHub topics (exactly 7):** `sbom`, `software-supply-chain`, `cyclonedx`, `spdx`, `dependency-graph`, `cybersecurity`, `rust`
- **Product mission:** Build an SBOM platform that imports and validates CycloneDX/SPDX, normalizes components and package URLs, builds dependency graphs, tracks licenses, correlates known vulnerabilities when available, and exports reports. Distinguish direct/transitive dependencies and flag incomplete metadata.
- **Icon concept:** a stack of package blocks joined by graph edges
- **Notification events:** an SBOM fails validation; a license policy is violated; new vulnerability data matches a component

Interface for SBOM-Navigator:

| Category | Icon (Lucide-style name) | What it shows |
|---|---|---|
| Overview | `layout-dashboard` | Direct versus transitive components, license distribution, incomplete metadata, validation status |
| SBOMs | `package` | Imported CycloneDX and SPDX documents |
| Components | `boxes` | Normalized components and package URLs |
| Dependency Graph | `git-fork` | Direct and transitive dependency graph |
| Licenses | `scale` | License inventory |
| Vulnerabilities | `bug` | Known vulnerabilities correlated when data is available |
| Validation | `circle-check` | Schema validation results |
| Diff | `git-compare` | Comparison between SBOMs |
| Exports | `file-output` | Reports and exports |
| About | `info` | App identity, offline Usage guide, Source Code, bug reporting |
| Settings | `settings-2` | Appearance, density, launch preference, data location, safe defaults |

**Overview screen (first screen after launch).** Answer "what needs attention now" with dense, information-first content computed from the real data store:

- Components split into direct and transitive
- License distribution
- Components with incomplete metadata
- Validation status per document

**Signature views.**

- Dependency graph that visually distinguishes direct from transitive edges, with a keyboard-accessible tree alternative
- Validation report listing schema errors with locations

### IaC-Sentinel — Infrastructure-as-Code Security

- **Folder and repository name:** `iac-sentinel`
- **App name:** IaC-Sentinel
- **Primary language:** Rust (owns: static analysis of infrastructure-as-code, rule evaluation, SARIF/JSON output, CI exit codes)
- **Optional native component:** none.
- **Persistence:** SQLite, with the schema in SQL migrations
- **GitHub description:** Static analyzer for Terraform, Dockerfiles and Kubernetes YAML: finds insecure infrastructure configuration before deployment, explains each rule and exports SARIF and JSON reports. Desktop GUI, terminal UI and CLI. Never deploys anything.
- **GitHub topics (exactly 7):** `iac-security`, `terraform`, `kubernetes`, `dockerfile`, `static-analysis`, `cybersecurity`, `rust`
- **Product mission:** Build an IaC scanner for Terraform, Dockerfiles, and Kubernetes YAML with parsers, versioned rules, file/line evidence, severity, expiring suppressions, JSON/SARIF output, CI exit codes, and actionable remediation. State supported syntax honestly and never deploy infrastructure.
- **Icon concept:** angle brackets framing a plumb line
- **Notification events:** a scan finds a new critical issue; a suppression is about to expire

Interface for IaC-Sentinel:

| Category | Icon (Lucide-style name) | What it shows |
|---|---|---|
| Overview | `layout-dashboard` | Findings by severity and file type, expiring suppressions, scan history with last CI exit code |
| Projects | `folder` | Scanned folders and files |
| Scans | `scan-search` | Scan runs and results |
| Findings | `shield-alert` | Findings with file and line evidence |
| Rules | `book-check` | Versioned rule catalog |
| Suppressions | `eye-off` | Suppressions with required expiry |
| Exports | `file-output` | JSON and SARIF output |
| Supported Syntax | `list-checks` | Honest statement of supported Terraform, Dockerfile, and Kubernetes YAML subsets |
| About | `info` | App identity, offline Usage guide, Source Code, bug reporting |
| Settings | `settings-2` | Appearance, density, launch preference, data location, safe defaults |

**Overview screen (first screen after launch).** Answer "what needs attention now" with dense, information-first content computed from the real data store:

- Findings by severity and by file type (Terraform, Dockerfile, Kubernetes)
- Suppressions nearing expiry
- Scan history with the exit code each run would return in CI

**Signature views.**

- Finding detail with source excerpt, highlighted line, and remediation
- Supported Syntax page that states limits plainly

### SigmaLab — Detection Rule Engineering

- **Folder and repository name:** `sigmalab`
- **App name:** SigmaLab
- **Primary language:** Rust (owns: rule validation, fixture replay, matching engine, CI integration)
- **Optional native component:** C++, optional only if measured: multi-rule matching. Follow the optional native component rules.
- **Persistence:** SQLite, with the schema in SQL migrations
- **GitHub description:** Local-first detection engineering workbench: author, validate, test and version Sigma-style rules against sample logs and map them to ATT&CK techniques with coverage reports. Desktop GUI, terminal UI and CLI. Tests run on files you provide.
- **GitHub topics (exactly 7):** `sigma`, `detection-engineering`, `mitre-attack`, `threat-detection`, `blue-team`, `cybersecurity`, `rust`
- **Product mission:** Build a detection engineering workbench with syntax validation, rule metadata, version history, fixture replay, false-positive notes, ATT&CK mapping, export, and CI integration. Clearly state supported syntax and explain why each rule matched test logs.
- **Icon concept:** an original angular summation-like glyph inside a test-tube outline (do not reuse any existing project logo)
- **Notification events:** a replay regression appears (a previously passing fixture now fails)

Interface for SigmaLab:

| Category | Icon (Lucide-style name) | What it shows |
|---|---|---|
| Overview | `layout-dashboard` | Rules by status, coverage, last replay results, rules without fixtures |
| Rules | `file-code` | Rule library with metadata |
| Editor | `square-pen` | Editor with inline syntax validation |
| Fixtures | `flask-conical` | Test log fixtures |
| Replay | `play` | Fixture replay with match explanations |
| ATT&CK Map | `grid-3x3` | Coverage matrix |
| History | `history` | Rule version history |
| False Positives | `notebook-pen` | False-positive notes |
| Exports | `file-output` | Export formats |
| About | `info` | App identity, offline Usage guide, Source Code, bug reporting |
| Settings | `settings-2` | Appearance, density, launch preference, data location, safe defaults |

**Overview screen (first screen after launch).** Answer "what needs attention now" with dense, information-first content computed from the real data store:

- Rules by status
- ATT&CK coverage computed from rule metadata
- Last replay pass and fail counts
- Rules that lack fixtures

**Signature views.**

- Editor with inline validation errors and a statement of supported syntax
- Replay view that highlights exactly which fields and values caused each match

### LogForge-SIEM — Log Collection and Security Analytics

- **Folder and repository name:** `logforge-siem`
- **App name:** LogForge-SIEM
- **Primary language:** Rust (owns: ingestion, parsing, normalization, deduplication, backpressure, correlation and alert processing)
- **Optional native component:** C++, optional hot path only if measured: batch hashing for deduplication. Follow the optional native component rules.
- **Persistence:** SQLite with FTS5 full-text search, with the schema in SQL migrations
- **GitHub description:** Local-first SIEM-style log platform: ingests files and events, normalizes and deduplicates them, offers full-text search, correlation rules, alerts and investigation timelines. Desktop GUI, terminal UI and CLI. Data stays in one SQLite file.
- **GitHub topics (exactly 7):** `siem`, `log-analysis`, `log-management`, `detection-engineering`, `cybersecurity`, `rust`, `sqlite`
- **Product mission:** Build a modular SIEM-style platform with ingestion APIs, file ingestion, schema normalization, timestamp handling, deduplication, retention controls, search, saved queries, detection rules, alerts, case links, dashboards, and audit logs. Include synthetic logs and test backpressure, malformed events, timezones, and redaction.
- **Icon concept:** stacked log lines converging into one bright segment
- **Notification events:** an alert fires; ingestion stalls or drops events

Interface for LogForge-SIEM:

| Category | Icon (Lucide-style name) | What it shows |
|---|---|---|
| Overview | `layout-dashboard` | Events over time, top sources, alerts by rule and severity, ingestion health |
| Search | `search` | Query bar, time range, field sidebar, event detail |
| Saved Queries | `bookmark` | Saved and recent queries |
| Sources | `plug` | File and API ingestion sources |
| Schema | `braces` | Normalization mappings and parsers |
| Detections | `radar` | Detection rules |
| Alerts | `bell-ring` | Alert queue with case links |
| Retention | `database` | Retention controls and storage use |
| Audit | `scroll-text` | Audit log |
| About | `info` | App identity, offline Usage guide, Source Code, bug reporting |
| Settings | `settings-2` | Appearance, density, launch preference, data location, safe defaults |

**Overview screen (first screen after launch).** Answer "what needs attention now" with dense, information-first content computed from the real data store:

- Events ingested over time from the real store
- Top sources
- Alerts by rule and severity
- Backpressure, drop, and malformed-event counters

**Signature views.**

- Search view with an explicit timezone indicator and redacted fields marked as redacted
- Alert detail linking to the matching events and the case

### PhishLens — Email Threat Triage

- **Folder and repository name:** `phishlens`
- **App name:** PhishLens
- **Primary language:** Rust (owns: safe .eml and MIME parsing, header analysis, indicator defanging, redaction)
- **Optional native component:** none.
- **Persistence:** SQLite, with the schema in SQL migrations
- **GitHub description:** Safe, offline email analysis tool: parses .eml files, message headers, authentication results and URLs, scores suspicious signals with an explanation and keeps analyst notes. Desktop GUI, terminal UI and CLI. Never opens links or fetches content.
- **GitHub topics (exactly 7):** `phishing`, `email-analysis`, `email-security`, `incident-response`, `cybersecurity`, `defensive-security`, `rust`
- **Product mission:** Build safe .eml email triage: parse headers, extract and defang URLs, inspect SPF/DKIM/DMARC evidence, identify display-name/domain mismatches, redact sensitive data, and generate structured reports. Never render active email content, auto-open URLs, or execute attachments.
- **Icon concept:** an envelope with a magnifier cutout
- **Notification events:** a batch analysis completes; a sample contains high-severity indicators

Interface for PhishLens:

| Category | Icon (Lucide-style name) | What it shows |
|---|---|---|
| Overview | `layout-dashboard` | Recent analyses, indicator types, authentication evidence summary |
| Samples | `mail` | Imported .eml samples |
| Analysis | `microscope` | Safe plain-text view and findings |
| Headers | `list-tree` | Header analysis and display-name mismatches |
| Authentication | `shield-check` | SPF, DKIM, and DMARC evidence |
| Indicators | `flag` | Defanged URLs and addresses |
| Reports | `file-text` | Structured reports |
| About | `info` | App identity, offline Usage guide, Source Code, bug reporting |
| Settings | `settings-2` | Appearance, density, launch preference, data location, safe defaults |

**Overview screen (first screen after launch).** Answer "what needs attention now" with dense, information-first content computed from the real data store:

- Recent analyses
- Indicator types found
- SPF, DKIM, and DMARC evidence counts, worded as evidence rather than verdicts

**Signature views.**

- Message view showing safe plain text only
- Indicator table with defanged values and copy actions

### ForensicFlow — Digital Evidence Casework

- **Folder and repository name:** `forensicflow`
- **App name:** ForensicFlow
- **Primary language:** Rust (owns: evidence metadata, SHA-256 hashing, custody events, immutable audit records, read-only evidence handling)
- **Optional native component:** C++, optional hot path only if measured: streaming hashing of large evidence files. Follow the optional native component rules.
- **Persistence:** SQLite, with the schema in SQL migrations
- **GitHub description:** Local-first digital forensics case manager: records evidence metadata, cryptographic hashes and chain of custody, builds timelines and keeps investigation notes in one tamper-evident file. Desktop GUI, terminal UI and CLI. Never alters evidence.
- **GitHub topics (exactly 7):** `digital-forensics`, `dfir`, `chain-of-custody`, `evidence-management`, `cybersecurity`, `rust`, `egui`
- **Product mission:** Build forensic casework with evidence intake, SHA-256 hashes, custody events, timestamps, handler identity, access controls, timeline correlation, reports, and integrity verification. Never modify source evidence; distinguish recorded metadata from forensic conclusions.
- **Icon concept:** a magnifying lens over a grid of hash blocks
- **Notification events:** an integrity check shows a hash mismatch; evidence awaits verification; a custody gap is detected

Interface for ForensicFlow:

| Category | Icon (Lucide-style name) | What it shows |
|---|---|---|
| Overview | `layout-dashboard` | Open cases, evidence awaiting verification, integrity results, recent custody events |
| Cases | `briefcase` | Case list and case workspace |
| Evidence | `archive` | Evidence items with recorded metadata and SHA-256 |
| Chain of Custody | `link` | Custody events with handler identity and timestamps |
| Integrity | `fingerprint` | Hash verification runs, expected versus computed |
| Timeline | `history` | Correlated event timeline |
| Reports | `file-text` | Generated reports |
| Access & Audit | `lock-keyhole` | Roles, access log, immutable audit records |
| About | `info` | App identity, offline Usage guide, Source Code, bug reporting |
| Settings | `settings-2` | Appearance, density, launch preference, data location, safe defaults |

**Overview screen (first screen after launch).** Answer "what needs attention now" with dense, information-first content computed from the real data store:

- Open cases and evidence items awaiting verification
- Integrity results: verified versus mismatch
- Recent custody events
- Recent access log entries

**Signature views.**

- Custody timeline per evidence item
- Integrity run showing expected and computed SHA-256 side by side; recorded metadata is visually separated from analyst conclusions

### NetConfig-Auditor — Network Configuration Audit

- **Folder and repository name:** `netconfig-auditor`
- **App name:** NetConfig-Auditor
- **Primary language:** Rust (owns: typed configuration parsers, rule evaluation, redaction, diffing)
- **Optional native component:** none.
- **Persistence:** SQLite, with the schema in SQL migrations
- **GitHub description:** Local-first configuration auditor for network devices and infrastructure files: checks backups against documented rules, detects drift between versions and reports findings with evidence. Desktop GUI, terminal UI and CLI. Reads files only.
- **GitHub topics (exactly 7):** `network-security`, `config-audit`, `configuration-drift`, `compliance`, `cybersecurity`, `defensive-security`, `rust`
- **Product mission:** Build a configuration auditing platform for supplied network configuration files and backups. Support documented vendor-neutral formats plus a clearly stated initial vendor subset, rule packs, severities, redacted evidence, baseline comparison, drift detection, exceptions, reports, and CI exit codes. Do not modify device configurations.
- **Icon concept:** a configuration sheet crossed by a single rule line with a tick
- **Notification events:** a new critical finding appears; drift against the baseline is detected; an exception is about to expire

Interface for NetConfig-Auditor:

| Category | Icon (Lucide-style name) | What it shows |
|---|---|---|
| Overview | `layout-dashboard` | Findings by severity and rule pack, drift versus baseline, expiring exceptions, last audit exit code |
| Configurations | `file-text` | Supplied configuration files and backups |
| Rule Packs | `book-check` | Versioned rule packs |
| Findings | `shield-alert` | Findings with redacted evidence |
| Drift | `diff` | Baseline comparison |
| Exceptions | `file-warning` | Exceptions with expiry |
| Reports | `file-text` | Exported reports |
| About | `info` | App identity, offline Usage guide, Source Code, bug reporting |
| Settings | `settings-2` | Appearance, density, launch preference, data location, safe defaults |

**Overview screen (first screen after launch).** Answer "what needs attention now" with dense, information-first content computed from the real data store:

- Findings by severity and rule pack
- Drift versus baseline
- Exceptions nearing expiry
- Last audit run and its CI exit code

**Signature views.**

- Configuration viewer with redacted evidence at the matching lines
- Baseline diff view

### RouteScope — Routing and BGP Visibility

- **Folder and repository name:** `routescope`
- **App name:** RouteScope
- **Primary language:** C++ (owns: route and MRT-style data parsing, prefix analysis, AS-path and origin comparison)
- **Optional native component:** Rust, optional only if measured: MRT/BGP dump decoding. Follow the optional native component rules.
- **Persistence:** SQLite, with the schema in SQL migrations
- **GitHub description:** Local-first routing analysis tool: loads route tables and BGP announcements, keeps prefix history, detects route changes and visualizes paths. Desktop GUI, terminal UI and CLI. Works on files you import and sends no routing traffic.
- **GitHub topics (exactly 7):** `bgp`, `routing`, `network-analysis`, `route-monitoring`, `network-visualization`, `networking`, `cpp`
- **Product mission:** Build routing visibility using imported route snapshots and reputable public data where available. Visualize prefixes, AS paths, origin AS, RPKI validation when available, route changes, and timelines. Show data freshness and source limitations. Never alter routes or send routing announcements.
- **Icon concept:** a path graph passing through three autonomous-system nodes
- **Notification events:** a prefix origin changes; an RPKI invalid state appears; a data source goes stale

Interface for RouteScope:

| Category | Icon (Lucide-style name) | What it shows |
|---|---|---|
| Overview | `layout-dashboard` | Tracked prefixes, origin changes, RPKI states where available, data freshness per source |
| Prefixes | `network` | Prefix list and detail |
| Routes | `route` | Imported route snapshots |
| AS Paths | `waypoints` | AS-path graph |
| RPKI | `badge-check` | Validation state when available |
| Timeline | `history` | Route-change timeline |
| Data Sources | `database` | Freshness and documented limitations of each source |
| Import | `upload` | Snapshot import |
| About | `info` | App identity, offline Usage guide, Source Code, bug reporting |
| Settings | `settings-2` | Appearance, density, launch preference, data location, safe defaults |

**Overview screen (first screen after launch).** Answer "what needs attention now" with dense, information-first content computed from the real data store:

- Prefixes tracked
- Origin changes
- RPKI valid, invalid, and unknown counts when available
- Data freshness for each source

**Signature views.**

- AS-path graph for a selected prefix
- Route-change timeline with a persistent data-freshness banner

### NetGraph — Network Topology and Inventory

- **Folder and repository name:** `netgraph`
- **App name:** NetGraph
- **Primary language:** C++ (owns: topology model, graph algorithms, layout computation, SNMP-fixture adapters)
- **Optional native component:** Rust, optional only if measured: topology file importers. Follow the optional native component rules.
- **Persistence:** SQLite, with the schema in SQL migrations
- **GitHub description:** Local-first network topology and inventory manager: models routers, switches, servers, interfaces, VLANs and links, shows dependencies on an interactive map and checks the model for inconsistencies. Desktop GUI, terminal UI and CLI.
- **GitHub topics (exactly 7):** `network-topology`, `network-inventory`, `network-visualization`, `dependency-graph`, `networking`, `cpp`, `imgui`
- **Product mission:** Build a network topology platform with device inventory, interfaces, links, VLAN/subnet associations, graph visualization, mocked discovery and optional authorized SNMP polling, topology history, search/filtering, CSV import/export, and stale-data indicators. Include realistic simulated topology.
- **Icon concept:** three nodes joined as a triangle with one emphasized link
- **Notification events:** the topology changes; device data becomes stale

Interface for NetGraph:

| Category | Icon (Lucide-style name) | What it shows |
|---|---|---|
| Overview | `layout-dashboard` | Devices by role and staleness, link counts, recent topology changes |
| Topology | `network` | Interactive topology canvas with filters |
| Devices | `router` | Device inventory |
| Interfaces & Links | `cable` | Interfaces and links |
| VLANs & Subnets | `layers` | VLAN and subnet associations |
| Discovery | `radar` | Mocked discovery and optional authorized SNMP |
| History | `history` | Topology history |
| Import / Export | `arrow-left-right` | CSV import and export |
| About | `info` | App identity, offline Usage guide, Source Code, bug reporting |
| Settings | `settings-2` | Appearance, density, launch preference, data location, safe defaults |

**Overview screen (first screen after launch).** Answer "what needs attention now" with dense, information-first content computed from the real data store:

- Devices by role
- Stale-data indicators
- Link counts
- Recent topology changes

**Signature views.**

- Topology canvas with stale-data styling and a keyboard-accessible device list as an alternative view
- Device detail with interfaces, links, VLANs, and history

### SecureBaseline — Configuration Compliance

- **Folder and repository name:** `securebaseline`
- **App name:** SecureBaseline
- **Primary language:** C++ (owns: policy evaluation, evidence collection adapters, versioned rules, reporting)
- **Optional native component:** Rust, optional only if measured: bulk configuration file parsing. Follow the optional native component rules.
- **Persistence:** SQLite, with the schema in SQL migrations
- **GitHub description:** Local-first security baseline assessor: checks host and application configuration against documented policies, reports pass, fail and evidence per control and tracks fixes. Desktop GUI, terminal UI and CLI. Read-only by default.
- **GitHub topics (exactly 7):** `security-baseline`, `compliance`, `hardening`, `configuration-audit`, `cybersecurity`, `defensive-security`, `cpp`
- **Product mission:** Build a policy-driven baseline checker with versioned rules, platform checks, supplied-file/read-only evidence collection, severity, exceptions, remediation guidance, run comparisons, and HTML/JSON reports. Never silently change system settings.
- **Icon concept:** a level bar over a check tick
- **Notification events:** a control newly fails; an exception is about to expire

Interface for SecureBaseline:

| Category | Icon (Lucide-style name) | What it shows |
|---|---|---|
| Overview | `layout-dashboard` | Compliance per baseline version, failed controls by severity, expiring exceptions, run comparison delta |
| Baselines | `list-checks` | Versioned baselines and rules |
| Controls | `shield-check` | Control catalog |
| Results | `clipboard-check` | Run results |
| Evidence | `folder-open` | Read-only evidence collected or supplied |
| Exceptions | `file-warning` | Exceptions with expiry |
| Comparisons | `git-compare` | Run-to-run comparison |
| Reports | `file-text` | HTML and JSON reports |
| About | `info` | App identity, offline Usage guide, Source Code, bug reporting |
| Settings | `settings-2` | Appearance, density, launch preference, data location, safe defaults |

**Overview screen (first screen after launch).** Answer "what needs attention now" with dense, information-first content computed from the real data store:

- Compliance percentage per baseline version computed from stored results
- Failed controls by severity
- Exceptions nearing expiry
- Change since the previous run

**Signature views.**

- Control detail with evidence and remediation guidance that is never applied automatically
- Run comparison showing newly failing and newly passing controls

## Work plan

1. **Foundation.** Create the workspace root, scripts, workflow generator and the three language templates, with their own tests and CI. Do not start a project until `scripts/check-all.sh` passes on the templates.
2. **Pilot one project per language** to prove the toolchain, GUI packaging, headless screenshots and release workflow on all three runners: **`certwatch` (Go), `secretsentry` (Rust), `netgraph` (C++)**. Do not continue until each pilot is Complete or its blockers are documented honestly. Fix template problems in the templates.
3. **Remaining Go projects** in the order of the table, then **remaining Rust projects**, then **remaining C++ projects**.
4. **Per project, the order is:** requirements, acceptance criteria and threat model; schema migrations and SQL tests; domain and service layers with tests; CLI; terminal UI; dashboard; notifications; icon and identity; docs; CI and release workflow; audit table and quality report.
5. **Never leave a project half-finished.** If time or budget is running out, bring the current project to a truthful state, mark it In progress or Complete in the root `README.md`, and stop. Do not start a new project you cannot finish. Report the state of every project at the end.
6. Keep the repository green after every push. Run the quality suite of the touched project and the root checks. Commit with Conventional Commits and the project scope.

## Final rule

Do not settle for vibe-coded output. Do not generate filler to inflate lines of code, do not pretend incomplete features are finished, do not guarantee zero bugs, and do not report checks that were not actually run. Keep all active and test source code free of comments. Deliver a correct, tested foundation and a small number of fully finished projects before widening scope, then continue through the table in order.
