# Changelog

All notable changes to Template App are recorded here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0-dev]

### Added

- Dashboard window, full-screen terminal UI and commands over one service layer and one SQLite database.
- Launcher that asks how to open the application, remembers the answer and never asks in scripts or on sessions without a display.
- Records with status, severity, note and an enabled switch; full-text search, filters, sorting, a column chooser and a detail panel.
- Import and export of records as CSV or JSON with a dry run before anything is written.
- Notifications with a rule per event type: enabled, minimum severity, quiet hours and de-duplication.
- Opt-in background notifier with a per-user start-at-sign-in entry.
- Settings stored in the database, validated before saving, with reset per setting and a visible source when a flag or environment variable overrides a value.
- Seven color themes in light and dark mode, a contrast-checked custom accent, density, text size, and a terminal palette that honors `NO_COLOR`.
- About with the offline usage guide, shortcuts, diagnostics, licenses, and links that open only after a confirmation that shows the full address.
- Labeled demo data that is loaded and removed with one action.
- Scripts for setup, checks, fuzzing, packaging and release verification.

[0.1.0-dev]: https://github.com/unalivin/workspace/releases
