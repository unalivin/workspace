# Architecture

## Overview

```text
     crates/templateapp-app  (main.rs)      process entry, wiring of the hooks
                   |
     crates/templateapp-app  (library)      argument parsing, commands, launcher call
              /    |     \
  templateapp-gui  templateapp-tui  (command handlers)
              \    |     /
     templateapp-core::service              one service layer for all three interfaces
        /      |       |       \
  records  settings  notify   theme         domain modules of templateapp-core
        \      |       |
     templateapp-core::store                SQLite, migrations, data_version polling
```

The dashboard, the terminal UI and the commands call the same functions of `templateapp-core`, most of them in its `service` module. None of them contains business rules or SQL. That is what keeps them in step.

## Crates and modules

The Cargo workspace has four crates. The library names used in `use` lines are `app_core`, `app_cli`, `app_tui` and `app_gui`.

| Crate | Responsibility |
|---|---|
| `crates/templateapp-core` | Everything that is not presentation: the modules in the next table |
| `crates/templateapp-app` | The binary `templateapp` (`main.rs`) and the command line (`cli.rs`, `records.rs`, `notifications.rs`, `config.rs`); the feature `gui` links the dashboard and is on by default; `build.rs` embeds the icon into the Windows executable |
| `crates/templateapp-tui` | Terminal UI on ratatui with crossterm |
| `crates/templateapp-gui` | Dashboard on egui with eframe: the skin that replaces the toolkit look (`skin.rs`), icons (`icons.rs`), tables and forms, pages, the window host (`host.rs`), the chooser, and a software renderer for tests and screenshots (`render.rs`) |
| `fuzz` | The fuzz targets, a separate package outside the workspace |

| Module of `templateapp-core` | Responsibility |
|---|---|
| `meta` | Name, slug, version, application identifier, repository address; the single source of every link; the embedded license, notices and usage guide |
| `userconfig` | The user configuration directory and `config.json` (database path, remembered interface) |
| `store` | Opening SQLite with the required pragmas, migrations, read-only mode, the busy timeout, `PRAGMA data_version` |
| `settings` | Setting definitions, validation, stored values, overrides and their source |
| `theme` | Design tokens, the seven themes, contrast math, severity colors, the terminal palette |
| `records`, `transfer`, `csv` | Records, full-text search, overview numbers, demo data; import and export |
| `notify`, `notifier` | Notification rules and the inbox; the background notifier, start at sign-in, delivery to the operating system |
| `launcher` | The decision which interface to open, session detection, the terminal prompt |
| `guide` | Parsing and searching the embedded usage guide |
| `service` | Service layer: opening and moving the database, appearance, diagnostics, links, the capability matrix |
| `platform` | Directories per operating system, opening the browser and a terminal window |
| `brand`, `png` | The application icon, drawn in code, and its file formats |
| `clock`, `files`, `error`, `words`, `testkit` | Time without a time zone database, private file writes, the error type, plurals, temporary directories for tests |

Other directories:

| Path | Content |
|---|---|
| `migrations` | Numbered SQL migrations, embedded into the binary |
| `crates/*/examples` | Generators: `genassets` (`assets/`), `gendocs` (`docs/COMMANDS.md`), `gennotices` (`THIRD_PARTY_NOTICES.md`), `screenshots` (the dashboard screens as PNG files) |
| `scripts` | Setup, checks, sanitizer and fuzz runs, packaging, release verification, publishing, and their bats tests |

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
- The dashboard and the terminal UI read `PRAGMA data_version` on the refresh interval (the setting `data.refresh_seconds`, two seconds by default). When another connection has written, the visible page is reloaded, the selected row and any open form are kept, and the notice "Updated by another window" appears. The terminal UI also reloads on `r`.
- An edit carries the revision it was based on. If the record changed in the meantime the save is refused and the form keeps the user's text.
- A busy database is waited for up to five seconds (the SQLite busy timeout) and then reported; the form keeps the user's text.

## Launching

The `launcher` module decides from the arguments, the environment, the remembered choice and the session (desktop, terminal, headless, piped) whether to open the dashboard, open the terminal UI, ask, print help or fail with a reason. A subcommand never opens an interface and a non-interactive session is never asked. The dashboard and the chooser use one event loop, which the toolkit keeps for the whole process, so the chooser can close and the dashboard can open in the same run.

## Theming

A theme is a value of `theme::Tokens`: palette, semantic and severity colors, spacing, type scale, radius. `skin.rs` in the dashboard crate maps the tokens onto the toolkit style so that colors, sizes and corner radii come from the theme, and the `theme` module maps the same tokens to ANSI colors for the terminal UI. Icons are one set of original drawings defined as outlines in `icons.rs`.

The interface follows one direction, and a change to it should fit these five lines:

- Product: a desktop tool for records that carry a status and a severity, used by one operator.
- Character: plain, exact, calm.
- World: an operations logbook. Surfaces are cool grey paper, text is near-black ink, and one cobalt accent marks the current place and the main action.
- Where color goes: the breakdown of enabled records on the Overview and the count of open records. Navigation, top bar and settings stay grey.
- Signature: severity as an icon and a word in its own color, next to the count as a figure.

The rules that follow from it:

- Spacing sits on a four pixel grid: 4, 8, 12, 16, 24. Rows are 32 high, 24 in compact density. Text sizes are 12, 14, 16 and 24, multiplied by the text size setting.
- The seven themes share one grey ramp. A named theme moves the hue of the greys slightly and sets the accent; severity colors never change with the theme.
- Severity and status are always an icon and a word. Color is an addition, never the only carrier.
- The Overview draws only stored data.
- Text and icons meet WCAG AA contrast on every surface of every theme; the `theme` module tests this for all themes, both modes and custom accents.

## Builds

| Build | Cargo features | Content |
|---|---|---|
| Dashboard | default (`gui`) | everything |
| No-GUI | `--no-default-features` | terminal UI and commands; the dashboard crate and the window toolkit are not compiled or linked |

SQLite is compiled in by the `bundled` feature of `rusqlite`, which includes FTS5, so neither build needs a system SQLite.

Decisions and their reasons are recorded in [adr/](adr/README.md).
