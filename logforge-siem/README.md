# LogForge-SIEM

Local-first SIEM-style log platform: ingests files and events, normalizes and deduplicates them, offers full-text search, correlation rules, alerts and investigation timelines. Desktop GUI, terminal UI and CLI. Data stays in one SQLite file.

## How it works

![LogForge-SIEM architecture](docs/architecture.svg)

The core owns all data access; the desktop GUI, terminal UI, and CLI call the same service layer. Log lines enter through file or synthetic sources, get normalized to UTC, have secrets redacted, and are hashed with SHA-256 for deduplication. Events land in SQLite with an FTS5 index, and matching detection rules create alerts in the same transaction.

## Install

Download the `0.1.0-dev` pre-release for your platform from GitHub Releases, or build from source with Rust `1.83.0`:

```bash
cargo build --release -p logforge-app
```

## Use

```bash
logforge-app demo
logforge-app --ui tui
logforge-app search "Failed"
```

## Documentation

See [docs/README.md](docs/README.md).

## Security

See [SECURITY.md](SECURITY.md).

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

MIT. See [LICENSE](LICENSE). Copyright (c) 2026 Sonix Devs.
