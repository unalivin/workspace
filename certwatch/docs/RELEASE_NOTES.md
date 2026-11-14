# CertWatch v0.1.0-dev (dev beta)

Local-first certificate inventory and TLS posture monitor: tracks expiry, validates hostnames, inspects chains and keeps an audit history, with expiry alerts. Desktop GUI, terminal UI and CLI. Checks only hosts you are authorized to test.

CertWatch keeps its data in one SQLite file on your computer. It has no account, no telemetry and no update check. It connects to the network only for TLS handshakes with the hosts you added and confirmed, and it lists every connection before it makes one. It has a dashboard window, a full-screen terminal UI and commands for scripts, all over the same data.

## This is an early dev beta

Version `0.1.0-dev` is a pre-release for testing. Not every planned feature exists; [docs/ROADMAP.md](https://github.com/NexzaDev/workspace/blob/HEAD/certwatch/docs/ROADMAP.md) lists what is missing and what has not been verified on real desktops. The database format may change before `0.1.0`. Export your endpoint list before you update (`certwatch endpoints export --file endpoints.csv`).

Checks have been tested against local servers with generated certificates, not yet against servers on the internet. Services that need STARTTLS and revocation checks are not supported.

## Downloads

| File | Platform |
|---|---|
| `certwatch-linux-x64.tar.gz` | Linux x86-64 |
| `certwatch-macos-arm64.tar.gz` | macOS on Apple silicon |
| `certwatch-windows-x64.exe` | Windows x86-64 |

## Check the digest

GitHub shows the SHA-256 digest of each file next to it. Compute the digest of your download and compare:

```bash
sha256sum certwatch-linux-x64.tar.gz
shasum -a 256 certwatch-macos-arm64.tar.gz
```

```powershell
Get-FileHash certwatch-windows-x64.exe -Algorithm SHA256
```

## Open an unsigned build

The files are not signed and not notarized.

- **macOS**: right-click `CertWatch.app`, choose Open, then Open again.
- **Windows**: in the SmartScreen dialog choose More info, then Run anyway.
- **Linux**: unpack the archive and run `make user-install`, or `chmod +x usr/local/bin/certwatch` and start it from the folder.

## Getting started

Start the application and choose Dashboard or Terminal. Add an endpoint you are authorized to test and run a check, or load the demo data, whose hosts are never contacted. The usage guide is built in: About, Usage guide, or `certwatch guide`. Report problems from About, "Found a bug? Please report it on GitHub."
