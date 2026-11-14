# Threat model

## What is protected

| Asset | Where |
|---|---|
| The records and notifications | the SQLite database file |
| Settings, including the database location | the database and `config.json` |
| The user's session | the process runs with the user's rights and can start a browser, a terminal and a notification helper |

## Trust boundaries

1. **Files the user imports** (CSV, JSON): untrusted content.
2. **The database file**: usually written by the application, but it can be replaced or edited by anything that runs as the user.
3. **Environment variables and command-line arguments**: set by the user or by a script the user runs.
4. **Helper programs**: the browser opener, the terminal emulator and the notification tool of the operating system.
5. **The release download**: travels over the network before the user runs it.

There is no network listener, no remote service and no second user.

## Threats and mitigations

| Threat | Mitigation | Proof |
|---|---|---|
| SQL injection through names, notes, search text or import files | Bound parameters everywhere; search text becomes a quoted expression; static scan of the source | `tests/sql_test.go`, `internal/records` fuzz target `FuzzSearchExpressionIsAlwaysAValidQuery` |
| Malformed or huge import files exhaust memory or corrupt data | Size and row limits, strict parsing, one transaction, dry run first | `internal/records` tests `TestParseRejectsBadInput`, `TestImportUpdatesRejectsAndSupportsDryRun`, fuzz target `FuzzParse` |
| Command injection through text handed to helper programs | Arguments are passed as separate values without a shell; notification text is passed as data; the terminal launcher quotes every word | `internal/platform` tests, `internal/notify` test `TestCommandDelivererPassesTextAsDataNotCode` |
| A crafted link opens an arbitrary site | Only addresses under the configured repository are opened, after a confirmation with the full address | `internal/meta` test `TestIsRepositoryURL`, `internal/app` test `TestExternalLinksStayInsideTheRepository` |
| Path traversal or overwriting a foreign file through the database location | The path is used as given by the user, a file that is not this application's database is refused, moves copy and never delete | `internal/store` tests `TestOpenLeavesForeignFilesUntouched`, `internal/app` test `TestMoveDatabase` |
| Other local users read the data | Files and directories are created for the current user only | `internal/store` test `TestDatabaseFileIsCreatedWithRestrictivePermissions`, `internal/userconfig` tests |
| Private data leaks through notifications | Count only by default; titles are reduced to a safe set; never names or notes | `internal/notify` tests `TestSummary`, `TestSafeTitle`, `TestPublishStoresOnlySafeTitles` |
| Private data leaks through bug reports or diagnostics | The report has fixed fields and is previewed; no paths, no record content | `internal/app` test `TestDiagnosticsAndIssueReportCarryNoPrivateData` |
| The start-at-sign-in entry runs something else | The entry is written from a fixed template; unsafe executable paths are refused | `internal/notify` test `TestAutostartRejectsUnsafeExecutablePaths` |
| Two windows corrupt the database | WAL mode, busy timeout, retries, revision check on edits | `internal/store` and `internal/records` concurrency tests |
| A damaged or hostile `config.json` | Size limit, strict parsing, validation; a broken file is reported, not used | `internal/userconfig` test `TestLoadRejectsBrokenFiles` |
| A vulnerable dependency | Pinned versions, `govulncheck` in CI | `make vuln` |
| A tampered download | The digest shown by GitHub can be compared; the builds are not signed | [INSTALLATION.md](INSTALLATION.md) |

## Accepted risks

- Anything that already runs as the user can read and change the database. The database is not encrypted.
- The downloads are unsigned, so the operating system cannot verify the publisher.
- The helper programs are found through `PATH`. A hostile `PATH` is outside the model, because it already means a compromised account.
- The background notifier relies on the notification service of the desktop; what that service does with the text is outside the application.
