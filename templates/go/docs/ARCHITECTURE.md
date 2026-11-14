# Architecture

## Overview

```text
            cmd/templateapp            process entry, console handling, wiring
                   |
              internal/cli             argument parsing, commands, launcher call
              /    |     \
   internal/gui  internal/tui  (command handlers)
              \    |     /
              internal/app             one service layer for all three interfaces
        /      |       |       \
  records  settings  notify   theme    domain packages
        \      |       |
           internal/store              SQLite, migrations, data_version polling
```

The dashboard, the terminal UI and the commands call the same functions of `internal/app`. None of them contains business rules or SQL. That is what keeps them in step.

## Packages

| Package | Responsibility |
|---|---|
| `cmd/templateapp` | `main`, signal handling, Windows console attachment, build-tag switch between the dashboard build and the no-GUI build |
| `internal/meta` | Name, slug, version, application identifier, repository address; the single source of every link |
| `internal/userconfig` | The user configuration directory and `config.json` (database path, remembered interface) |
| `internal/store` | Opening SQLite with the required pragmas, migrations, read-only mode, busy retries, `PRAGMA data_version` |
| `internal/settings` | Setting definitions, validation, stored values, overrides and their source |
| `internal/theme` | Design tokens, the seven themes, contrast math, severity colors, the terminal palette |
| `internal/records` | Records, full-text search, overview numbers, demo data, import and export |
| `internal/notify` | Notification rules, the inbox, the background notifier, start at sign-in, delivery to the operating system |
| `internal/launcher` | The decision which interface to open, session detection, the terminal prompt |
| `internal/guide` | Parsing and searching the embedded usage guide |
| `internal/app` | Service layer: opening and moving the database, appearance, diagnostics, links, the capability matrix |
| `internal/cli` | Commands, flags, help, the generated command reference |
| `internal/tui` | Terminal UI on Bubble Tea |
| `internal/gui` | Dashboard on Fyne: the skin that replaces the toolkit look, widgets, pages, the window host |
| `internal/platform` | Opening the browser and a terminal window |
| `internal/brand` | The application icon, drawn in code, and its file formats |
| `internal/licenses` | License detection and the third-party notices |
| `internal/sqlscan` | Static check that no query is built from non-constant strings |
| `internal/tools/*` | Generators for `assets/` and `THIRD_PARTY_NOTICES.md` |
| `migrations` | Numbered SQL migrations, embedded into the binary |
| `tests` | Checks that span packages: SQL policy, generated documents, repository rules |

## Data

One SQLite database in WAL mode with foreign keys on and a busy timeout.

| Object | Content |
|---|---|
| `settings` | key, value, time of change |
| `records` | name (unique), status, severity, note, enabled, demo flag, revision, timestamps |
| `records_fts` | FTS5 index over name and note, kept current by triggers |
| `notification_rules` | one optional row per event type; absent means the default rule |
| `notifications` | event type, severity, safe title, link to a record, de-duplication key, read time |
| `overview_record_counts`, `overview_attention` | views that produce the Overview numbers for every interface |

`testdata/schema.snapshot` is the expected schema; a test fails when a migration changes it without updating the snapshot. Migrations are applied in one transaction each and are never edited after release; a change is a new numbered file.

## Staying in sync

- Every write goes through the service layer into the database. There is no in-memory state shared between interfaces.
- The dashboard polls `PRAGMA data_version` about every two seconds. When another connection has written, it reloads the visible page, keeps the selected row and any open form, and shows "Updated by another window".
- The terminal UI reloads on its refresh interval and on `r`.
- An edit carries the revision it was based on. If the record changed in the meantime the save is refused and the form keeps the user's text.
- A busy database is retried a bounded number of times and then reported.

## Launching

`internal/launcher` decides from the arguments, the environment, the remembered choice and the session (desktop, terminal, headless, piped) whether to open the dashboard, open the terminal UI, ask, print help or fail with a reason. A subcommand never opens an interface and a non-interactive session is never asked. The dashboard and the chooser share one window host so the toolkit is started once per process.

## Theming

A theme is a value of `theme.Tokens`: palette, semantic and severity colors, spacing, type scale, radius. `internal/gui/skin.go` maps the tokens onto the toolkit so no toolkit default color, size or icon remains, and `internal/theme` maps the same tokens to ANSI colors for the terminal UI. Icons are one set of original drawings defined in `internal/gui/skin.go`.

## Builds

| Build | Tags | Content |
|---|---|---|
| Dashboard | `sqlite_fts5 no_animations` | everything |
| No-GUI | `sqlite_fts5 nogui` | terminal UI and commands; no window toolkit is linked |

Decisions and their reasons are recorded in [adr/](adr/README.md).
