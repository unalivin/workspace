# Template App

TEMPLATE_DESCRIPTION

Template App is a local-first desktop application. Your data stays in one SQLite file on your computer. The application makes no network requests and collects nothing.

It has three interfaces over the same data:

- **Dashboard**: a native desktop window.
- **Terminal UI**: a full-screen text interface for terminals and SSH sessions.
- **Commands**: `templateapp <command>` for scripts.

A change made in one interface appears in the others.

Version `0.1.0-dev`. This is an early development build ("dev beta"). Not every planned feature exists; [docs/ROADMAP.md](docs/ROADMAP.md) lists what is missing.

## Download

Pre-releases are published on the [releases page](https://github.com/NexzaDev/workspace/releases). Each one has exactly three files:

| File | For |
|---|---|
| `templateapp-linux-x64.tar.gz` | Linux, x86-64 |
| `templateapp-macos-arm64.tar.gz` | macOS, Apple silicon |
| `templateapp-windows-x64.exe` | Windows, x86-64 |

The files are not signed and not notarized. [docs/INSTALLATION.md](docs/INSTALLATION.md) explains how to check the digest and how to open an unsigned build.

## Install

Linux:

```bash
tar -xzf templateapp-linux-x64.tar.gz
cd templateapp-linux-x64
make user-install
```

macOS: unpack the archive, move `Template App.app` to Applications, then right-click it and choose Open the first time.

Windows: run `templateapp-windows-x64.exe`. If SmartScreen appears, choose More info, then Run anyway.

To build from source, see [docs/INSTALLATION.md](docs/INSTALLATION.md).

## Usage

Start the application without arguments and choose Dashboard or Terminal. Or use the commands:

```bash
templateapp --ui tui
templateapp records add --name "First record" --severity high
templateapp records list
templateapp overview
templateapp --help
```

The usage guide is built into the application (About, Usage guide, or `templateapp guide`) and is also in [docs/USAGE.md](docs/USAGE.md).

## Documentation

[docs/README.md](docs/README.md) is the index. The most used pages:

- [Installation](docs/INSTALLATION.md)
- [Usage guide](docs/USAGE.md)
- [Commands and capability matrix](docs/COMMANDS.md)
- [Configuration](docs/CONFIGURATION.md)
- [Troubleshooting](docs/TROUBLESHOOTING.md)
- [Platform support](docs/PLATFORM_SUPPORT.md)

## Security

Report vulnerabilities privately through the [GitHub advisory form](https://github.com/NexzaDev/workspace/security/advisories/new). See [SECURITY.md](SECURITY.md) and the [threat model](docs/THREAT_MODEL.md).

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). Run `make check` before you push.

## License

MIT, see [LICENSE](LICENSE). Third-party components are listed in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
