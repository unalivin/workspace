# Architecture

## Overview

```text
     src/app/main.cpp                       process entry, wiring of the hooks
                   |
     src/app  (library app_cli)             argument parsing, commands, launcher call
              /    |     \
      src/gui   src/tui   (command handlers)
              \    |     /
     src/core/service                       one service layer for all three interfaces
        /      |       |       \
  records  settings  notify   theme         domain modules of src/core
        \      |       |
     src/core/store                         SQLite, migrations, data_version polling
```

The dashboard, the terminal UI and the commands call the same functions of `src/core`, most of them in its `service` module. None of them contains business rules or SQL. That is what keeps them in step.

## Libraries and modules

The CMake project builds one executable from static libraries: three without the dashboard and two more with it. Everything is in the namespace `app`.

| Directory | Target | Responsibility |
|---|---|---|
| `src/core` | `app_core` | Everything that is not presentation: the modules in the next table |
| `src/app` | `app_cli`, `templateapp` | The command line (`cli.cpp`, `records_commands.cpp`, `notification_commands.cpp`, `config_commands.cpp`) and the executable (`main.cpp`); with `PROJECT_GUI=ON` it links the dashboard; on Windows the version resource and the icon are compiled in from `packaging/app.rc.in` |
| `src/tui` | `app_tui` | Terminal UI: a model that paints a grid of styled cells (`model.cpp`, the pages, `widgets.cpp`), and the one file that talks to FTXUI for the terminal, the keyboard and the event loop (`screen.cpp`) |
| `src/gui` | `app_gui_view`, `app_gui` | Dashboard on Dear ImGui: the skin that replaces the toolkit look (`skin.cpp`), icons (`icons.cpp`), the overview tally (`ledger.cpp`), tables and forms, pages, the chooser, and a software renderer for tests and screenshots (`render.cpp`) are `app_gui_view`, which needs no window system; the window host on GLFW and OpenGL 3 (`host.cpp`) is `app_gui` |
| `third_party/sqlite` | `sqlite3` | The SQLite amalgamation, compiled with FTS5 |
| `fuzz` | `fuzz_<target>` | The fuzz targets |

| Module of `src/core` | Responsibility |
|---|---|
| `meta` | Name, slug, version, application identifier, repository address; the single source of every link |
| `embedded` | The files that the build embeds: migrations, the license, the notices and the usage guide |
| `userconfig` | The user configuration directory and `config.json` (database path, remembered interface) |
| `store` | Opening SQLite with the required pragmas, migrations, read-only mode, the busy timeout, `PRAGMA data_version`; the only code that calls the SQLite C interface, behind the classes `Store` and `Statement` |
| `settings` | Setting definitions, validation, stored values, overrides and their source |
| `theme` | Design tokens, the seven themes, contrast math, severity colors, the terminal palette |
| `records`, `transfer`, `csv` | Records, full-text search, overview numbers, demo data; import and export |
| `notify`, `notifier` | Notification rules and the inbox; the background notifier, start at sign-in, delivery to the operating system |
| `launcher` | The decision which interface to open, session detection, the terminal prompt |
| `guide` | Parsing and searching the embedded usage guide |
| `service` | Service layer: opening and moving the database, appearance, diagnostics, links, the capability matrix |
| `platform` | Directories per operating system, starting helper programs, opening the browser and a terminal window, the console on Windows |
| `brand`, `png` | The application icon, drawn in code, and its file formats |
| `clock`, `files`, `error`, `text` | Time without a time zone database, private file writes, the error type, text helpers |

Other directories:

| Path | Content |
|---|---|
| `migrations` | Numbered SQL migrations, embedded into the binary by `cmake/Embed.cmake` |
| `tools` | Generators: `genassets` (`assets/`), `gendocs` (`docs/COMMANDS.md`), `gennotices` (`THIRD_PARTY_NOTICES.md`), `screenshots` (the dashboard screens as PNG files) |
| `tests` | The test program, see [TESTING.md](TESTING.md) |
| `cmake`, `packaging` | Warning and sanitizer settings, embedding, CPack packaging and its templates |
| `third_party` | The vendored SQLite, the component list `components.tsv` and the license texts |
| `scripts` | Setup, build, checks, sanitizer and fuzz runs, supply chain checks, packaging, release verification, publishing, and their bats tests |

## Error handling and ownership

- A failure is an `AppError` with a kind (validation, not found, conflict, usage, storage, input and output) and a message that says what happened and what to do. The command line turns a usage error into exit code 2 and every other failure into exit code 1; the interfaces show the message.
- Objects own through values, standard containers and standard smart pointers. The handles of SQLite, GLFW and Dear ImGui are held by classes that release them in their destructors. There is no `new`, no `delete` and no owning raw pointer.
- Text is UTF-8 in `std::string`; paths cross into `std::filesystem::path` in one place (`files::from_utf8`, `files::shown`), so that Windows paths outside ASCII keep their characters.

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

The `launcher` module decides from the arguments, the environment, the remembered choice and the session (desktop, terminal, headless, piped) whether to open the dashboard, open the terminal UI, ask, print help or fail with a reason. A subcommand never opens an interface and a non-interactive session is never asked. The chooser and the dashboard are two windows that are opened one after the other by the same host code, so the chooser can close and the dashboard can open in the same run.

## Theming

A theme is a value of `theme::Tokens`: palette, semantic and severity colors, spacing, type scale, radius. `skin.cpp` in `src/gui` maps the tokens onto the toolkit style so that colors, sizes and corner radii come from the theme, and the `theme` module maps the same tokens to ANSI colors for the terminal UI. Icons are one set of original drawings defined as outlines in `icons.cpp`.

The interface follows one direction, and a change to it should fit these five lines:

- Product: a desktop tool for records that carry a status and a severity, used by one operator.
- Character: plain, exact, calm.
- World: an operations logbook. Surfaces are cool grey paper, text is near-black ink, and one cobalt accent marks the current place and the main action.
- Where color goes: the breakdown of enabled records on the Overview and the count of open records. Navigation, top bar and settings stay grey.
- Signature: the tally square. One square stands for a fixed number of records, in the color of its severity, next to the count as a figure.

The rules that follow from it:

- Spacing sits on a four pixel grid: 4, 8, 12, 16, 24. Rows are 32 high, 24 in compact density. Text sizes are 12, 14, 16, 24 and 28, multiplied by the text size setting.
- One text family in two weights, regular and medium, plus a monospaced face for counts. The build embeds both weights from `src/gui/fonts`, and `skin.cpp` installs them when a window is created.
- The seven themes share one grey ramp. A named theme moves the hue of the greys slightly and sets the accent; severity colors never change with the theme.
- Severity and status are always an icon and a word. Color is an addition, never the only carrier.
- The Overview draws only stored data. The tally (`ledger.cpp`) and the table next to it read the same rows.
- Text and icons meet WCAG AA contrast on every surface of every theme; the `theme` module tests this for all themes, both modes and custom accents.

## Builds

| Build | CMake option | Content |
|---|---|---|
| Dashboard | `PROJECT_GUI=ON` (presets `dev`, `release`) | everything |
| No-GUI | `PROJECT_GUI=OFF` (presets `nogui`, `release-nogui`) | terminal UI and commands; `src/gui`, Dear ImGui and GLFW are not compiled or linked |

SQLite is compiled from `third_party/sqlite` with FTS5, so neither build needs a system SQLite. The other libraries come from vcpkg at the versions pinned in `vcpkg.json` and are linked statically; on Windows the C++ runtime is linked statically too, so the release is one executable without companion files.

Decisions and their reasons are recorded in [adr/](adr/README.md).
