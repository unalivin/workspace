# 0003. SQLite through cgo with FTS5 and WAL

Status: accepted

## Context

The data is local, small to medium in size and read by up to three processes at once: the dashboard, the terminal UI or a command, and the background notifier. Search over names and notes must be fast and tolerant of arbitrary input.

## Decision

- Driver: `github.com/mattn/go-sqlite3`, built with the `sqlite_fts5` tag. cgo is already required by the window toolkit, so the driver adds no new build requirement.
- Every connection sets `foreign_keys = ON`, `journal_mode = WAL`, `busy_timeout` and `synchronous = NORMAL` through the connection string, so pooled connections cannot differ.
- Tables are `STRICT`. Timestamps are UTC text in one format.
- Migrations are numbered SQL files embedded in the binary, each applied in one transaction, with the schema version in `PRAGMA user_version`.
- Other processes' writes are detected by polling `PRAGMA data_version`.
- A busy database is retried a bounded number of times, then reported.
- The record identifiers use `AUTOINCREMENT` so an identifier is never reused after a removal.

## Consequences

- Cross-compiling needs a C toolchain for the target; releases are built on native runners.
- A database written by a newer build is refused rather than opened with an unknown schema.
- The notifier can open the database read-only while another process writes.
