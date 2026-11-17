# 0003. SQLite compiled in, with FTS5 and WAL

Status: accepted

## Context

The data is local, small to medium in size and read by up to three processes at once: the dashboard, the terminal UI or a command, and the background notifier. Search over names and notes must be fast and tolerant of arbitrary input.

## Decision

- Library: the crate `rusqlite` with its `bundled` feature, which compiles SQLite from source into the application, with FTS5. The application never uses the SQLite of the operating system, so every platform runs the same version.
- Every connection sets `foreign_keys = ON` and `busy_timeout` (five seconds). A writing connection also sets `journal_mode = WAL` and `synchronous = NORMAL`; a read-only connection sets `query_only`. Connections are opened in one place (`store.rs`), so they cannot differ.
- Tables are `STRICT`. Timestamps are UTC text in one format.
- Migrations are numbered SQL files compiled into the binary, each applied in one transaction, with the schema version in `PRAGMA user_version`.
- Other processes' writes are detected by polling `PRAGMA data_version`.
- A busy database is waited for up to the busy timeout, then reported with what to do.
- The record identifiers use `AUTOINCREMENT` so an identifier is never reused after a removal.
- SQL lives only in the modules `store`, `settings`, `records` and `notify` of `secretsentry-core`, as constants with bound parameters.

## Consequences

- Building needs a C compiler, because SQLite is compiled from source. This is the only C code the application builds itself.
- Cross-compiling needs a C toolchain for the target; releases are built on native runners.
- A database written by a newer build is refused rather than opened with an unknown schema.
- The notifier can open the database read-only while another process writes.
