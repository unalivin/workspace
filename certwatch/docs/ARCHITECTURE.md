# Architecture

## Overview

```text
            cmd/certwatch            process entry, console handling, wiring
                   |
              internal/cli             argument parsing, commands, launcher call
              /    |     \
   internal/gui  internal/tui  (command handlers)
              \    |     /
              internal/app             one service layer for all three interfaces
        /      |       |       \
 inventory settings  notify   theme    domain packages
     |  \      |       |
     |     internal/store              SQLite, migrations, data_version polling
     |
 internal/tlscheck                     the only package that opens connections
```

The dashboard, the terminal UI and the commands call the same functions of `internal/app`. None of them contains business rules or SQL. That is what keeps them in step.

## Packages

| Package | Responsibility |
|---|---|
| `cmd/certwatch` | `main`, signal handling, Windows console attachment, build-tag switch between the dashboard build and the no-GUI build |
| `internal/meta` | Name, slug, version, application identifier, repository address; the single source of every link |
| `internal/userconfig` | The user configuration directory and `config.json` (database path, remembered interface) |
| `internal/store` | Opening SQLite with the required pragmas, migrations, read-only mode, busy retries, `PRAGMA data_version` |
| `internal/settings` | Setting definitions, validation, stored values, overrides and their source |
| `internal/theme` | Design tokens, the seven themes, contrast math, severity colors, the terminal palette |
| `internal/tlscheck` | The TLS handshake, chain and name validation, protocol probing, findings, PEM parsing, and the filter for text that comes from servers. The only package that opens connections |
| `internal/inventory` | Endpoints, certificates, observations, findings, expiry alerts, reports, demo data, import and export of the endpoint list |
| `internal/notify` | Alert rules, the alert inbox, the background notifier, start at sign-in, delivery to the operating system |
| `internal/certgen` | A certificate authority generated at run time for tests and for the demo data |
| `internal/testkit` | Test helpers: temporary databases, a settable clock, a local TLS server |
| `internal/words` | Singular and plural forms of counted nouns |
| `internal/launcher` | The decision which interface to open, session detection, the terminal prompt |
| `internal/guide` | Parsing and searching the embedded usage guide |
| `internal/app` | Service layer: opening and moving the database, the views and their actions, checks and the scheduler, appearance, diagnostics, links, the capability matrix |
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
| `endpoints` | host, port, optional server name, note, enabled, demo flag, revision, time of the authorization confirmation, the latest observation |
| `certificates` | one row per distinct certificate, keyed by SHA-256 fingerprint: subject, issuer, names, validity, key, signature, the PEM text, whether it was imported |
| `observations` | one row per check of an endpoint: outcome, error, leaf certificate, chain and name status, TLS version, cipher suite, accepted versions, and the previous certificate when it changed |
| `observation_certificates` | the chain of an observation in order, and whether each certificate was sent by the server or came from the trust store |
| `findings` | the findings of an observation with code, severity and detail |
| `expiry_alerts` | which expiry threshold has already raised an alert for which certificate |
| `reports` | generated reports with kind, format, row count, SHA-256 digest and content |
| `notification_rules` | one optional row per event type; absent means the default rule |
| `notifications` | the alerts: event type, severity, safe title, link to an endpoint or a certificate, de-duplication key, read time |
| `endpoint_status` | view: every endpoint joined with its latest observation and leaf certificate |
| `tracked_certificates` | view: certificates that an enabled endpoint presents now, plus imported ones |
| `latest_findings` | view: the findings of the latest observation of every enabled endpoint |
| `overview_attention` | view: the Overview counts for every interface |

Observations beyond `checks.history_keep` per endpoint are removed after a check, and certificates that nothing refers to any more are removed with them.

`testdata/schema.snapshot` is the expected schema; a test fails when a migration changes it without updating the snapshot. Migrations are applied in one transaction each and are never edited after release; a change is a new numbered file.

## Staying in sync

- Every write goes through the service layer into the database. There is no in-memory state shared between interfaces.
- The dashboard polls `PRAGMA data_version` about every two seconds. When another connection has written, it reloads the visible page, keeps the selected row and any open form, and shows "Updated by another window".
- The terminal UI reloads on its refresh interval and on `r`.
- An edit carries the revision it was based on. If the endpoint changed in the meantime the save is refused and the form keeps the user's text.
- A busy database is retried a bounded number of times and then reported.

## A check

1. An interface asks the service layer for the plan of a network action. The plan names every connection.
2. The interface shows the plan and asks. Commands need `--yes`.
3. `internal/app` reads the timeout, the probing switch and the trust file from the settings and calls `tlscheck.Check` for each endpoint that is enabled and not demo data.
4. `tlscheck` makes one handshake with verification on. If verification fails it takes the presented chain from the verification error and evaluates chain and name separately. With probing on it makes one more handshake per TLS version.
5. `inventory.Record` stores the observation, the chain and the findings in one transaction, compares the leaf with the previous successful observation, and raises alerts: expiry thresholds once per certificate, validation failures on the transition from good to bad, and certificate changes.
6. The interface reloads; other open windows notice through `data_version`.

The dashboard and the terminal UI run checks off their event loop and show a busy state. The scheduler in both, and `certwatch watch`, call the same function for the endpoints whose last check is older than the interval.

## Views

Certificates, Endpoints, Expiry Timeline, Chains, Configuration and Reports are described as data in `internal/app/views.go`: filters, columns, and actions with their form fields. The commands, the keys of the terminal UI and the buttons of the dashboard are generated from that description. Overview and Alerts have layouts of their own. See [adr/0010-views-as-data.md](adr/0010-views-as-data.md).

## Launching

`internal/launcher` decides from the arguments, the environment, the remembered choice and the session (desktop, terminal, headless, piped) whether to open the dashboard, open the terminal UI, ask, print help or fail with a reason. A subcommand never opens an interface and a non-interactive session is never asked. The dashboard and the chooser share one window host so the toolkit is started once per process.

## Theming

A theme is a value of `theme.Tokens`: palette, semantic and severity colors, spacing, type scale, corner radii. `internal/gui/skin.go` maps the tokens onto the toolkit so no toolkit default color, size or icon remains, and `internal/theme` maps the same tokens to ANSI colors for the terminal UI. Icons are one set of original drawings defined in `internal/gui/skin.go`.

The interface follows one direction, and a change to it should fit these five lines:

- Product: a desktop monitor for TLS certificates and the endpoints that present them, used by one operator.
- Character: watchful, exact, quiet.
- World: a duty log of deadlines. Surfaces are cool grey paper, text is near-black ink, and one cobalt accent marks the current place and the main action.
- Where color goes: the tally of tracked certificates by time left on the Overview, and the time-left bars of the expiry timeline. Navigation, top bar and settings stay grey.
- Signature: the tally square. One square stands for a fixed number of certificates, in the color of its expiry period, next to the count as a figure.

The rules that follow from it:

- Spacing sits on a four pixel grid: 4, 8, 12, 16, 24, 32. Rows are 32 high, 24 in compact density. Text sizes are 12, 14, 16, 24 and 28, multiplied by the text size setting.
- One text family in two weights, regular and medium, plus a monospaced face for counts.
- Corner radii are 4 for small marks, 8 for controls and 12 for panels. The high contrast theme has square corners.
- The seven themes share one grey ramp. A named theme moves the hue of the greys slightly and sets the accent; severity colors never change with the theme.
- Severity and status are always an icon and a word. Color is an addition, never the only carrier.
- The Overview draws only stored data. The tally reads the same counts as the figures beside it, and every row opens the list it counts.
- Text and icons meet WCAG AA contrast on every surface of every theme; `internal/theme` tests this for all themes, both modes and custom accents.

## Builds

| Build | Tags | Content |
|---|---|---|
| Dashboard | `sqlite_fts5 no_animations` | everything |
| No-GUI | `sqlite_fts5 nogui` | terminal UI and commands; no window toolkit is linked |

Decisions and their reasons are recorded in [adr/](adr/README.md).
