# VulnTrack

Local-first vulnerability management: imports scan findings, correlates assets and CVEs, prioritizes remediation with explainable risk scores and tracks fixes through tickets. Desktop GUI, terminal UI and CLI. Imports results, runs no scans.

## How it works

![VulnTrack architecture](docs/architecture.svg)

Scan results are imported into SQLite, deduplicated per asset and CVE, scored for risk using severity and exploitability context, and tracked against SLA due dates.

## Install

```bash
go build -o bin/vulntrack ./cmd/vulntrack
```

## Use

```bash
vulntrack demo
vulntrack --ui tui
vulntrack list
```

## License

MIT. See [LICENSE](LICENSE). Copyright (c) 2026 Sonix Devs.
