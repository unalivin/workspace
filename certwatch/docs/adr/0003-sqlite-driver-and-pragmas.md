# 0003. SQLite through cgo with WAL

Status: accepted

## Context

The data is local, small to medium in size and read by up to three processes at once: the dashboard, the terminal UI or a command, and the background notifier. The inventory holds thousands of rows at most. Search over host names, notes and certificate names must be tolerant of arbitrary input.

## Decision

- Driver: `github.com/mattn/go-sqlite3`. cgo is already required by the window toolkit, so the driver adds no new build requirement. The build uses the `sqlite_fts5` tag like every Go project of this workspace, but CertWatch has no full-text index: search is a case-insensitive substring match with the text as a bound parameter.
- Every connection sets `foreign_keys = ON`, `journal_mode = WAL`, `busy_timeout` and `synchronous = NORMAL` through the connection string, so pooled connections cannot differ.
- Tables are `STRICT`. Timestamps are UTC text in one format.
- Migrations are numbered SQL files embedded in the binary, each applied in one transaction, with the schema version in `PRAGMA user_version`.
- Other processes' writes are detected by polling `PRAGMA data_version`.
- A busy database is retried a bounded number of times, then reported.
- The identifiers of endpoints, certificates and alerts use `AUTOINCREMENT` so an identifier is never reused after a removal.

## Consequences

- Cross-compiling needs a C toolchain for the target; releases are built on native runners.
- A database written by a newer build is refused rather than opened with an unknown schema.
- The notifier can open the database read-only while another process writes.
