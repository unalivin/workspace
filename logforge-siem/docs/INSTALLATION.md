# Installation

## Pre-built binaries (`0.1.0-dev`)

Download the archive for your platform from the GitHub Releases page:

- Linux x64: `logforge-siem-linux-x64.tar.gz`
- macOS arm64: `logforge-siem-macos-arm64.tar.gz`
- Windows x64: `logforge-siem-windows-x64.exe`

## Build from source

Requires Rust `1.83.0`:

```bash
cargo build --release -p logforge-app
```

For headless environments without GUI libraries:

```bash
cargo build --release -p logforge-app --no-default-features
```
