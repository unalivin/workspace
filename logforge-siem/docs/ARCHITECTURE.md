# Architecture

`logforge-siem` is structured as a Cargo workspace with four crates:

- `crates/logforge-core`: SQLite migrations, domain types, log parsers (JSON lines, syslog), normalization, deduplication, redaction, FTS5 search, detection rules, and alert evaluation.
- `crates/logforge-app`: CLI entry point, launcher decision logic, and headless subcommands.
- `crates/logforge-tui`: Full-screen terminal UI built with `ratatui` and `crossterm`.
- `crates/logforge-gui`: Desktop GUI built with `eframe` and `egui`.

The core owns all data access. The CLI, terminal UI, and desktop GUI call the same service layer and read identical numbers from SQLite views and prepared queries.

## Design

- **Direction:** LogForge-SIEM is a local-first log analysis workbench for security analysts and engineers. It feels like a quiet, high-density terminal instrument: dark slate neutrals, crisp tabular numbers, and a single amber accent for active alerts and matching log segments.
- **Palette:** Slate neutral ramp (`#0f172a` to `#f8fafc` in dark/light modes) with amber (`#d97706` / `#f59e0b`) as the primary accent. Severity colors pair an icon and a text label.
- **Type scale:** System sans-serif for navigation and labels (13 px body, 16 px section headers, 20 px view titles) paired with monospace for log lines, timestamps, and query bars.
- **Spacing:** 4 px grid, 28 px compact / 36 px comfortable table row height, 8 px gap between grouped controls.
- **Icons:** Embedded Lucide SVG icon set (16 px, 1.5 px stroke).
