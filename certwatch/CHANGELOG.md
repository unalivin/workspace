# Changelog

All notable changes to CertWatch are recorded here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Fixed

- Usage guide: a heading whose title ends like the suffix of a repeated title (for example "Step", "Step" and "Step i") no longer gets the same anchor as another section.

## [0.1.0-dev]

### Added

- Dashboard window, full-screen terminal UI and commands over one service layer and one SQLite database.
- Launcher that asks how to open the application, remembers the answer and never asks in scripts or on sessions without a display.
- Endpoints: hosts to monitor, each stored only after a confirmation that the user is authorized to test it; enable, disable, edit with conflict detection, remove, search and filters.
- TLS checks: one handshake per endpoint with verification always on, recording the chain, issuer, subject alternative names, validity, key, fingerprint, chain validation, host name validation, TLS version and cipher suite. Optional probing of the accepted TLS versions.
- Every network action lists its connections and asks first; `--dry-run` prints them. Automatic checks are off by default.
- Findings for expired, not yet valid, self-signed, untrusted, mismatching, weak-key, weak-signature and long-lived certificates, old TLS versions and weak cipher suites.
- Certificate inventory with detail, chain, the endpoints that present a certificate and its change history; import and export of PEM files.
- Expiry Timeline with zoom, Chains and Configuration views.
- Overview with clickable counts for expired and expiring certificates, chain failures, name mismatches, failed checks and unchecked endpoints.
- Alerts when a certificate crosses an expiry threshold, when validation starts to fail and when a certificate changes, with a rule per event type: enabled, minimum severity, quiet hours and de-duplication.
- Reports of the inventory, of expiring certificates and of findings as Markdown, CSV or JSON, stored with a digest.
- Import and export of the endpoint list as CSV or JSON with a preview before anything is written.
- `certwatch watch` for checks at an interval from a terminal.
- Additional trusted roots from a PEM file for private certificate authorities.
- Text from certificates and handshake errors is reduced to printable characters before it is stored or shown.
- Opt-in background notifier with a per-user start-at-sign-in entry.
- Settings stored in the database, validated before saving, with reset per setting and a visible source when a flag or environment variable overrides a value.
- Seven color themes in light and dark mode, a contrast-checked custom accent, density, text size, and a terminal palette that honors `NO_COLOR`.
- About with the offline usage guide, shortcuts, diagnostics, licenses, and links that open only after a confirmation that shows the full address.
- Labeled demo data with generated certificates that is loaded and removed with one action; demo hosts are never contacted.
- Scripts for setup, checks, fuzzing, packaging and release verification.

[0.1.0-dev]: https://github.com/unalivin/workspace/releases
