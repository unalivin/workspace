# Testing

## Running

```bash
make check             # everything below that the machine can run, with a summary
make test-nogui        # all tests of the no-GUI build
make test              # all tests, dashboard included
make race-or-sanitize  # the no-GUI tests under the sanitizers (scripts/sanitize.sh)
make fuzz              # every fuzz target for FUZZTIME (default 5s each)
make fuzz FUZZTIME=2m
make lint lint-gui     # cppcheck and clang-tidy, warnings as errors
make shell             # shellcheck, shfmt and the bats tests of scripts/
make docs-check assets-check notices-check   # generated files are current
make vuln              # licenses, pinned versions, known vulnerabilities
make screenshots       # render the dashboard screens into dist/screens
make window-check      # Linux: open the real window on a virtual display (needs xvfb)
```

The tests are one program, `templateapp_tests`, written with doctest. `make test` runs it through CTest together with a replay of every fuzz target. A single test, by its name:

```bash
build/nogui/bin/templateapp_tests -tc="the decision table holds"
build/nogui/bin/templateapp_tests -sf="*store_test*"
```

Test names are sentences. A document cites a test by that sentence in quotes inside a code span, and `"tests named in documents exist"` fails when the name is not a test.

The dashboard tests need no display. They run the real interface code, feed it pointer and key events, read the text it paints, and paint the result with a software renderer that is part of the project (`src/gui/render.cpp`).

`make check` reports a step as skipped, with the reason, when its tool is missing. `scripts/check.sh --require-all` turns a skip into a failure; CI installs every tool.

## Static analysis

- Every compiler runs with warnings as errors: `-Wall -Wextra -Wpedantic -Wshadow -Wconversion -Wsign-conversion -Wnon-virtual-dtor -Wold-style-cast -Woverloaded-virtual` on GCC and Clang, `/W4 /permissive-` on MSVC. The vendored SQLite is compiled without them.
- `clang-format` checks the layout (`.clang-format`).
- `cppcheck` runs on every C++ file with the warning, performance and portability checks.
- `clang-tidy` runs on every C++ file with the checks listed in `.clang-tidy`, all as errors. It needs the compile commands of the Ninja generator, so it runs on Linux and macOS.

There is no suppression comment in the code. A check that produced only noise for this code base is switched off for everyone in `.clang-tidy`:

| Switched off | Why |
|---|---|
| `bugprone-easily-swappable-parameters` | Reports every function that takes two values of one type, for example two coordinates |
| `bugprone-implicit-widening-of-multiplication-result`, `bugprone-signed-bitwise`, `bugprone-suspicious-enum-usage`, `clang-analyzer-optin.core.EnumCastOutOfRange`, `performance-enum-size`, `performance-no-int-to-ptr` | Report the flag enumerations and identifier casts that the Dear ImGui interface requires, and pixel arithmetic whose range is checked before the multiplication |
| `bugprone-throwing-static-initialization`, `bugprone-exception-escape` | Report every constant table that owns memory and every function that can run out of memory |
| `bugprone-unchecked-optional-access` | Does not follow a check made through a test assertion or a helper |
| `performance-inefficient-vector-operation` | Asks for a reservation in loops of a few elements |
| `portability-avoid-pragma-once` | The headers use `#pragma once`, which every supported compiler implements |

## The sanitizer run

`make race-or-sanitize` builds the tests of the no-GUI build with runtime checks and runs them, together with the replay of the fuzz seeds:

- On Linux and macOS, first with AddressSanitizer and UndefinedBehaviorSanitizer (preset `asan`), then with ThreadSanitizer (preset `tsan`). The compiled-in SQLite is instrumented too, and undefined behavior stops the test instead of printing a note. On Linux the first run also reports memory that is never freed.
- On Windows the sanitizer runtimes are not used, because the build links the static runtime library. The same tests run as an optimized build with assertions, checked containers and `/sdl` (preset `checked`).
- The dashboard is not part of the sanitizer run. The libraries from vcpkg are not instrumented.

The application starts no thread of its own: the dashboard, the terminal UI and the notifier each run one loop, and other windows are other processes that meet in the database. The terminal library reads the keyboard on a thread of its own, which exists only in a real terminal. The ThreadSanitizer run therefore has little to find today; it is part of the suite so that the first thread added to the code is checked from its first commit.

## Layers

| Layer | Where | What it proves |
|---|---|---|
| Unit | `tests/core` | Validation, contrast math, quiet hours, launcher decisions, link rules, parsing |
| Generated input | the same files, with the generator in `tests/support/random.hpp` | Parsers and normalizers hold their invariants on thousands of generated inputs with fixed seeds |
| Database | `store_test.cpp`, `records_test.cpp`, `notify_test.cpp`, `settings_test.cpp` in `tests/core` | Real SQLite files in temporary directories: pragmas, migrations, rollback, read-only mode, a busy database, the schema snapshot, hot queries use their indexes, the Overview views match a known fixture |
| Service | `tests/core/service_test.cpp` | A change through one connection is seen through another, moving the database, diagnostics, the capability list |
| Commands | `tests/app/cli_test.cpp` | Every command through the real argument parser against a real database, including exit codes, prompts and help |
| Terminal UI | `tests/tui` | The model is driven with key presses; state and the painted screen are checked |
| Dashboard | `tests/gui`, with the driver in `tests/guikit` | The interface code is driven with events: every page, forms, confirmations, the skin, icons, the table layout |
| Capability matrix | `service_test.cpp`, `cli_test.cpp` and the tests of the two interfaces | Every capability is listed once, names a real command, is painted somewhere in the dashboard and is shown in the terminal UI; `docs/COMMANDS.md` matches |
| Fuzz | `fuzz/targets`, `make fuzz` and the replay in `make test` | The parsers hold their invariants on arbitrary input |
| Repository | `tests/app/repository_test.cpp` | Required files, links between documents, no email address, no source comments, settings documented, test names in the documents exist, the generated command tables are current, SQL only in the storage modules, one version number everywhere |
| Scripts | `scripts/tests/*.bats` | Setup, checks, packaging, release verification and publishing in dry-run and against fakes |

## Fuzz targets

Each file in `fuzz/targets` is one target, built in two ways from the same code:

- **Replay** (every platform, part of `make test`): the target runs on its seed inputs in `fuzz/seeds/<target>` and on a few thousand inputs that a fixed generator derives from them. No special compiler is needed.
- **libFuzzer** (`make fuzz`, preset `fuzz`): built with Clang, libFuzzer, AddressSanitizer and UndefinedBehaviorSanitizer on Linux. CI runs each target for five seconds, which mainly proves that they build and start; run them longer by hand. New inputs are kept in `fuzz/corpus/<target>` and a failing input is written to `fuzz/artifacts/`; neither directory is committed. A failing input becomes a regression test by copying it into `fuzz/seeds/<target>`.

| Target | Module | Invariant |
|---|---|---|
| `transfer_parse` | `transfer` | Import parsing of CSV and JSON fails only with a usage error |
| `csv_parse` | `csv` | CSV parsing fails only with a usage error and respects the row limit |
| `records_search_expression` | `records` | Any search text becomes a query SQLite accepts |
| `settings_normalize` | `settings` | A normalized setting value is stable and valid |
| `notify_safe_title` | `notify` | Notification titles stay single-line and inside the length limit |
| `theme_accent` | `theme` | Any accent color ends up readable on every theme |
| `guide_parse_and_search` | `guide` | Guide parsing and search always finish and anchors stay unique |
| `clock_parse` | `clock` | Time parsing accepts or rejects and never fails otherwise |

## Test data

- `testdata/schema.snapshot`: the expected schema. After adding a migration, update it with `UPDATE_SNAPSHOT=1 build/nogui/bin/templateapp_tests -tc="schema matches the committed snapshot"` and review the difference.
- `testdata/fixtures/overview.sql`: the fixture behind the Overview numbers.
- `fuzz/seeds`: the seed inputs of the fuzz targets.

Tests never touch the real user directories: each one gets a temporary home and database.

## Requirements and their tests

| Requirement | Tests |
|---|---|
| The three interfaces offer the same capabilities | `"navigation shortcuts links and capabilities are complete"`, `"every capability names a real command and every command has help"`, `"every dashboard capability is reachable"`, `"every terminal capability is reachable"`, `"the generated command tables are current"` |
| A change in one interface appears in the others | `"changes made by another connection are noticed once"`, `"changes from another window keep the selection and unsaved input"` |
| Conflicting edits lose nothing | `"a stale revision or a removed record is a conflict"`, `"an edit that lost a race keeps the form and explains"`, `"an edit that lost a race keeps the form open and explains"` |
| A second writer waits and is told, never corrupts | `"a second writer waits then reports a busy database"`, `"a failing migration is rolled back completely"`, `"a batch is created completely or not at all"` |
| Settings are validated, resettable and never dropped | `"values are stored listed and reset"`, `"stored values from other versions are kept and not used"`, `"flag beats environment beats stored beats default"`, `"invalid overrides are ignored and explained"` |
| Themes meet the contrast rules | `"every theme meets wcag aa in light and dark"`, `"severities are ordered named and never color only"`, `"a weak custom accent is adjusted and reported"`, `"any accent ends up readable and color parsing survives noise"` |
| The toolkit look is replaced | `"every theme replaces the default look and keeps text readable"`, `"severity and status icons exist and differ"`, `"every icon the dashboard draws comes from the application set"` |
| The window chrome leads somewhere | `"the top bar searches and its chip, bell and help lead somewhere"`, `"a collapsed sidebar explains its icons on hover"`, `"the chooser offers both front ends and marks the default"`, `"record table columns and sort are saved and follow other windows"` |
| The launcher never asks in scripts or without a display | `"the decision table holds"`, `"a subcommand always runs and never prompts"`, `"starting without a command follows the launch decision"` |
| Notifications follow the rules and leak nothing | `"evaluation checks enabled severity quiet hours then duplicates"`, `"quiet hours may cross midnight"`, `"publishing stores delivers and suppresses duplicates"`, `"titles are cleaned shortened and never carry secrets"`, `"the system notification shows a count and titles only on request"` |
| Text handed to helper programs is data, not code | `"delivery commands pass text as data never as code"`, `"terminal windows are tried in order and arguments are quoted"` |
| Start at sign-in is per user | `"autostart entries are placed and written per platform"`, `"install status and uninstall follow the file"` |
| Links stay inside the repository | `"only links below the repository are accepted"`, `"only repository links reach the browser"`, `"about opens links only on request and shows the guide and diagnostics"` |
| Diagnostics report the build and the settings | `"diagnostics report the build the database and changed settings"` |
| Files are private to the user | `"a new database is created private migrated and configured"`, `"files are written whole private and never overwritten silently"`, `"files are exported privately and imported back"` |
| A foreign file is never overwritten | `"files that are not databases are refused and left untouched"`, `"moving the database copies or switches and never overwrites"` |
| Demo data is labeled and removable | `"demo data is labelled loaded once and removed alone"` |
| Icons are valid in every format | `"the ico file lists valid pictures"`, `"the committed assets match the generator"` |
| Hot queries use an index and search text is always a valid query | `"hot queries use an index"`, `"search expressions are always valid full text queries"` |
| Every main screen of the dashboard renders | `"every main screen renders to a picture"` |
| Release rules | `scripts/tests/release.bats`, `scripts/tests/publish.bats` |

## Not covered by automated tests

The dashboard in a real window on a real display with real input (the window host, the event loop and the graphics backend), a real terminal emulator, display of system notifications by the desktop, start at sign-in at a real sign-in, and starting a packaged download on a clean machine. The check that SQL is not built from text (`"sql stays in the storage modules and is not built from text"`) reads the source line by line for SQL words inside text-building calls; it is not a parser, so review still matters. See [PLATFORM_SUPPORT.md](PLATFORM_SUPPORT.md).
