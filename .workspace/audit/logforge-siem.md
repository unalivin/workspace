# LogForge-SIEM Requirement Audit

| # | Requirement | Status | Proof | Command |
|---|---|---|---|---|
| 1 | Threat model and architecture | Implemented | `docs/THREAT_MODEL.md`, `docs/ARCHITECTURE.md` | `verify-repo.sh` |
| 2 | SQLite schema migrations and SQL tests | Implemented | `migrations/0001_initial.sql`, `crates/logforge-core/src/db.rs` | `cargo test --workspace` (pending CI) |
| 3 | Ingestion, normalization, deduplication, redaction | Not started | - | - |
| 4 | FTS5 search, saved queries, detection rules, alerts | Not started | - | - |
| 5 | CLI, launcher, and capability matrix | Not started | - | - |
| 6 | Terminal UI (`ratatui`) | Not started | - | - |
| 7 | Desktop GUI (`eframe`/`egui`) | Not started | - | - |
| 8 | Notifications and per-user autostart | Not started | - | - |
| 9 | Application icon and identity | Not started | - | - |
| 10 | Docs, diagrams, and release workflow | Not started | - | - |
