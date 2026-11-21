# Architecture

`vulntrack` is a Go module (`github.com/sonix-devs/vulntrack`) with:

- `cmd/vulntrack`: CLI entry point and launcher decision logic.
- `internal/db`: SQLite connection (WAL mode, foreign keys, busy timeout) and embedded SQL migrations.
- `internal/service`: Scan-result normalization, deduplication, CVE enrichment, explainable risk scoring, SLA tracking, and exception management.
- `internal/tui`: Terminal UI built with Bubble Tea.
- `internal/gui`: Desktop GUI built with Fyne (excluded under `-tags nogui`).

## Design

- **Direction:** VulnTrack is a vulnerability operations workbench for security teams. It uses neutral slate surfaces with a crimson accent (`#dc2626`) for SLA breaches and critical exploitability context.
- **Type scale:** System sans-serif for navigation and tables, monospace for CVE IDs, package names, and risk score breakdowns.
- **Spacing:** 4 px grid, 28 px compact / 36 px comfortable table row height.
- **Icons:** Embedded Lucide SVG icons (16 px, 1.5 px stroke).
