# Architecture

`netauto` is a Go module (`github.com/sonix-devs/netauto`) with:

- `cmd/netauto`: CLI entry point, launcher decision logic, and subcommands.
- `internal/db`: SQLite connection (WAL mode, foreign keys, busy timeout) and embedded SQL migrations.
- `internal/service`: Device inventory, job orchestration, dry-run diffing, approval gates, configuration backups, and encrypted credential store.
- `internal/tui`: Terminal UI built with Bubble Tea.
- `internal/gui`: Desktop GUI built with Fyne (excluded under `-tags nogui`).

## Design

- **Direction:** NetAuto is a network automation workbench for infrastructure engineers. It uses cool slate neutrals with a teal-blue accent (`#0284c7`) for approved states and dry-run diffs.
- **Type scale:** System sans-serif for navigation and forms, monospace for configuration backups, diffs, and execution logs.
- **Spacing:** 4 px grid, 28 px compact / 36 px comfortable table row height.
- **Icons:** Embedded Lucide SVG icons (16 px, 1.5 px stroke).
