# CertWatch

Local-first certificate inventory and TLS posture monitor: tracks expiry, validates hostnames, inspects chains and keeps an audit history, with expiry alerts. Desktop GUI, terminal UI and CLI. Checks only hosts you are authorized to test.

CertWatch is a local-first desktop application. Your data stays in one SQLite file on your computer. There is no account, no telemetry and no update check.

It connects to the network for one purpose: a TLS handshake with the hosts you added. You confirm for every host that you are authorized to test it, and the application lists each connection before it makes one. Certificate verification is never turned off; an untrusted chain is recorded as untrusted.

What it does:

- Keeps an inventory of endpoints and of the certificates they present, with issuer, subject alternative names, validity, key and fingerprint.
- Validates the chain against the trusted roots and the certificate against the host name.
- Records the negotiated TLS version and cipher suite, and optionally which TLS versions a server accepts.
- Keeps the history of observations and shows when a certificate changed.
- Raises alerts at expiry thresholds, when validation starts to fail and when a certificate changes.
- Shows an expiry timeline and writes reports as Markdown, CSV or JSON.

It has three interfaces over the same data:

- **Dashboard**: a native desktop window.
- **Terminal UI**: a full-screen text interface for terminals and SSH sessions.
- **Commands**: `certwatch <command>` for scripts.

A change made in one interface appears in the others.

Version `0.1.0-dev`. This is an early development build ("dev beta"). Not every planned feature exists; [docs/ROADMAP.md](docs/ROADMAP.md) lists what is missing.

## Download

Pre-releases are published on the [releases page](https://github.com/unalivin/workspace/releases); look for the entries named CertWatch. If there is none, build from source as [docs/INSTALLATION.md](docs/INSTALLATION.md) describes. Each pre-release has exactly three files:

| File | For |
|---|---|
| `certwatch-linux-x64.tar.gz` | Linux, x86-64 |
| `certwatch-macos-arm64.tar.gz` | macOS, Apple silicon |
| `certwatch-windows-x64.exe` | Windows, x86-64 |

The files are not signed and not notarized. [docs/INSTALLATION.md](docs/INSTALLATION.md) explains how to check the digest and how to open an unsigned build.

## Install

Linux:

```bash
tar -xzf certwatch-linux-x64.tar.gz
cd certwatch-linux-x64
make user-install
```

macOS: unpack the archive, move `CertWatch.app` to Applications, then right-click it and choose Open the first time.

Windows: run `certwatch-windows-x64.exe`. If SmartScreen appears, choose More info, then Run anyway.

To build from source, see [docs/INSTALLATION.md](docs/INSTALLATION.md).

## Usage

Start the application without arguments and choose Dashboard or Terminal. Or use the commands:

```bash
certwatch endpoints add --host www.example.org --authorized
certwatch endpoints check-all --dry-run
certwatch endpoints check-all --yes
certwatch overview
certwatch timeline list --zoom 30
certwatch reports generate --kind expiry --format csv
certwatch --ui tui
certwatch --help
```

`--dry-run` prints the connections a check would make without making them. To look around without touching the network, load the labeled demo data; demo hosts are never contacted:

```bash
certwatch demo load
certwatch demo remove
```

The usage guide is built into the application (About, Usage guide, or `certwatch guide`) and is also in [docs/USAGE.md](docs/USAGE.md).

## Documentation

[docs/README.md](docs/README.md) is the index. The most used pages:

- [Installation](docs/INSTALLATION.md)
- [Usage guide](docs/USAGE.md)
- [Commands and capability matrix](docs/COMMANDS.md)
- [Configuration](docs/CONFIGURATION.md)
- [Troubleshooting](docs/TROUBLESHOOTING.md)
- [Platform support](docs/PLATFORM_SUPPORT.md)
- [Requirements and their audit](docs/REQUIREMENTS.md)
- [Roadmap and known gaps](docs/ROADMAP.md)

## Security

Report vulnerabilities privately through the [GitHub advisory form](https://github.com/unalivin/workspace/security/advisories/new). See [SECURITY.md](SECURITY.md) and the [threat model](docs/THREAT_MODEL.md).

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). Run `make check` before you push.

## License

MIT, Copyright (c) 2026 Unalivin. See [LICENSE](LICENSE). Third-party components are listed in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
