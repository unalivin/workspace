# NetAuto

Local-first network automation tool: backs up device configurations, runs scheduled checks, applies approved change workflows and produces change reports with an audit trail. Desktop GUI, terminal UI and CLI. Nothing changes without approval.

## How it works

![NetAuto architecture](docs/architecture.svg)

The service layer owns device inventory, dry-run diffing, approval gates, and configuration backups in SQLite. Mock and lab adapters run by default so no production device is touched without explicit configuration and approval.

## Install

```bash
go build -o bin/netauto ./cmd/netauto
```

## Use

```bash
netauto demo
netauto --ui tui
netauto overview
```

## License

MIT. See [LICENSE](LICENSE). Copyright (c) 2026 Sonix Devs.
