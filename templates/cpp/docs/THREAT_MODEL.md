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
| SQL injection through names, notes, search text or import files | Statements are constants with bound parameters; search text becomes a quoted expression; a scan of the storage modules | `"sql stays in the storage modules and is not built from text"`, `"search expressions are always valid full text queries"`, fuzz target `records_search_expression` |
| Malformed or huge import files exhaust memory or corrupt data | Size and row limits, strict parsing, one transaction, dry run | `"unreadable imports are refused whole"`, `"bad rows are listed with their number and good rows still load"`, `"a dry run reports and changes nothing"`, fuzz target `transfer_parse` |
| Command injection through text handed to helper programs | Arguments are passed as separate values without a shell; notification text is passed as data; the terminal launcher quotes every word | `"delivery commands pass text as data never as code"`, `"terminal windows are tried in order and arguments are quoted"` |
| A crafted link opens an arbitrary site | Only addresses under the configured repository are handed to the browser, and only when the user chooses the link; a bug report is previewed first | `"only links below the repository are accepted"`, `"only repository links reach the browser"`, `"about opens links only on request and shows the guide and diagnostics"` |
| Path traversal or overwriting a foreign file through the database location | The path is used as given by the user, a file that is not this application's database is refused, moves copy and never delete | `"files that are not databases are refused and left untouched"`, `"moving the database copies or switches and never overwrites"` |
| Other local users read the data | Files and directories are created for the current user only | `"a new database is created private migrated and configured"`, `"files are written whole private and never overwritten silently"`, `"files are exported privately and imported back"` |
| Private data leaks through notifications | Count only by default; titles are reduced to a safe set; never names or notes | `"the system notification shows a count and titles only on request"`, `"titles are cleaned shortened and never carry secrets"`, `"safe titles are bounded single line and free of long tokens"` |
| Private data leaks through bug reports | The report has fixed fields and is previewed; no paths, no record content | `"diagnostics report the build the database and changed settings"`, `"about opens links only on request and shows the guide and diagnostics"` |
| The start-at-sign-in entry runs something else | The entry is written from a fixed template; executable paths that are relative or contain quoting characters are refused | `"autostart entries are placed and written per platform"` |
| Two windows corrupt the database | WAL mode, a busy timeout, one transaction per change, revision check on edits | `"a second writer waits then reports a busy database"`, `"a stale revision or a removed record is a conflict"`, `"a batch is created completely or not at all"` |
| A damaged or hostile `config.json` | Size limit, strict parsing; a broken file is reported, not used | `"broken files are reported with the way out"`, `"oversized and missing files are not read"` |
| A vulnerable dependency | Vendored SQLite at a recorded version, exact versions in `vcpkg.json` on a fixed baseline, the component list with upstream commits; the OSV lookup and the license check in CI | `make vuln` |
| Memory errors in parsers of untrusted input (out-of-bounds access, use after free, overflow) | Standard containers, strings and views instead of manual memory management and pointer arithmetic, size limits before parsing; sanitizer and fuzz runs | `make race-or-sanitize`, `make fuzz`, `"parsing arbitrary bytes fails only with a usage error"`, `"utf8 validation never reads past the end of random bytes"` |
| A tampered download | The digest shown by GitHub can be compared; the builds are not signed | [INSTALLATION.md](INSTALLATION.md) |

## Accepted risks

- Anything that already runs as the user can read and change the database. The database is not encrypted.
- The downloads are unsigned, so the operating system cannot verify the publisher.
- The helper programs are found through `PATH`. A hostile `PATH` is outside the model, because it already means a compromised account.
- The background notifier relies on the notification service of the desktop; what that service does with the text is outside the application.
