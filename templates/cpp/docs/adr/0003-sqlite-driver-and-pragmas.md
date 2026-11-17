# 0003. SQLite compiled in, with FTS5 and WAL

Status: accepted

## Context

The data is local, small to medium in size and read by up to three processes at once: the dashboard, the terminal UI or a command, and the background notifier. Search over names and notes must be fast and tolerant of arbitrary input.

## Decision

- Library: the SQLite amalgamation in `third_party/sqlite`, compiled into the application with FTS5 and called through its C interface. The application never uses the SQLite of the operating system, so every platform runs the same version. The classes `Store`, `Statement` and `Transaction` in `store.cpp` own the handles and are the only code that calls the C interface.
- Every connection sets `foreign_keys = ON` and `busy_timeout` (five seconds). A writing connection also sets `journal_mode = WAL` and `synchronous = NORMAL`; a read-only connection sets `query_only`. Connections are opened in one place (`store.cpp`), so they cannot differ.
- Tables are `STRICT`. Timestamps are UTC text in one format.
- Migrations are numbered SQL files compiled into the binary, each applied in one transaction, with the schema version in `PRAGMA user_version`.
- Other processes' writes are detected by polling `PRAGMA data_version`.
- A busy database is waited for up to the busy timeout, then reported with what to do.
- The record identifiers use `AUTOINCREMENT` so an identifier is never reused after a removal.
- SQL lives only in the modules `store`, `settings`, `records` and `notify` of `src/core`, as constants with bound parameters.

## Consequences

- SQLite is the only C code in the build. It is compiled without the project warnings, because it is not changed, and with the sanitizers in the sanitizer run.
- Updating SQLite means replacing the two files of the amalgamation and the version in `third_party/components.tsv`; `make vuln` fails when they differ.
- A database written by a newer build is refused rather than opened with an unknown schema.
- The notifier can open the database read-only while another process writes.
