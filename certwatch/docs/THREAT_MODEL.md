# Threat model

CertWatch is a defensive monitoring tool. It reads what a TLS server presents during a handshake and keeps a local record. It sends nothing to a server after the handshake, has no scanning of ports or address ranges, and connects only to hosts the user has listed and confirmed.

## What is protected

| Asset | Where |
|---|---|
| The endpoint list, which shows which hosts the user is responsible for, and the notes | the SQLite database file |
| Observations, findings, alerts and stored reports | the SQLite database file |
| Settings, including the database location and the file with additional trusted roots | the database and `config.json` |
| The user's session | the process runs with the user's rights and can start a browser, a terminal and a notification helper |
| The user's network position | connections leave from the user's computer and carry its address |
| Third parties | hosts that the user has no permission to test |

Certificates themselves are public data. Private keys are never read from a server and never stored.

## Trust boundaries

1. **Remote TLS servers**: everything a server sends is untrusted, including the text inside its certificates and the way it behaves during the handshake.
2. **Files the user imports** (endpoint lists as CSV or JSON, PEM files, the trust file): untrusted content.
3. **The database file**: usually written by the application, but it can be replaced or edited by anything that runs as the user.
4. **Environment variables and command-line arguments**: set by the user or by a script the user runs.
5. **Helper programs**: the browser opener, the terminal emulator and the notification tool of the operating system.
6. **The operating system's trust store and name resolver**: decide which roots are trusted and which address a name has.
7. **The release download**: travels over the network before the user runs it.

There is no network listener, no remote service and no second user.

## Threats and mitigations

| Threat | Mitigation | Proof |
|---|---|---|
| The tool is used, by mistake or on purpose, against hosts the user may not test | An endpoint is stored only with an explicit authorization confirmation, also on import and when the host or port changes; every network action lists its connections and asks, even when other confirmations are off; automatic checks are off by default; one handshake per check; no ranges, wildcards or port lists | `internal/inventory` `TestEndpointLifecycleNeedsAuthorization`, `TestEndpointTransferRoundTripAndImportRules`; `internal/app` `TestCheckRunsOnlyAfterAnAuthorizedEndpointAndDisclosesTheConnection`; `internal/cli` `TestWatchChecksDueEndpointsOnlyAfterConfirmation`; `internal/tui` `TestRemoveWithoutConfirmationWhenTheSettingIsOffButChecksStillAsk`; `internal/gui` `TestNetworkChecksAskEvenWhenConfirmationsAreOff` |
| A connection is made from somewhere other than the check, or to something other than the listed endpoint | Only `internal/tlscheck` may open connections, and it uses the dialer it is given; demo endpoints are never contacted | `tests` `TestNetworkAccessStaysInTheCheckPackage`; `internal/tlscheck` `TestCheckUsesTheGivenDialerAndNothingElse`; `internal/app` `TestDemoEndpointsAreNeverContacted` |
| An untrusted or mismatching certificate is reported as valid | Verification is never turned off; the chain of a failed verification is taken from the verification error and recorded as not trusted; the source may not contain the switch that disables verification | `internal/tlscheck` `TestCheckKeepsVerificationOnAndStillRecordsAnUntrustedChain`, `TestCheckReportsAHostnameMismatch`, `TestCheckReportsAnExpiredCertificate`, `TestCheckReportsASelfSignedCertificateAndAWeakKey`; `tests` `TestNetworkAccessStaysInTheCheckPackage` |
| A hostile server puts terminal escape sequences, line breaks or direction overrides into certificate fields to forge what the user sees | Text from certificates and from handshake errors is reduced to printable characters and a bounded length before it is stored or shown | `internal/tlscheck` `TestCheckNeutralizesHostileTextInCertificates`, `TestPrintableKeepsTextAndLimitsLength`, fuzz target `FuzzPrintable` |
| A hostile server stalls the handshake or answers with something that is not TLS | A timeout of at most 60 seconds bounds every connection and handshake; failures are recorded as failed checks | `internal/tlscheck` `TestCheckStopsAtTheTimeout`, `TestClampTimeout`, `TestCheckReportsAServerThatDoesNotSpeakTLS`, `TestCheckReportsAnUnreachableHost` |
| SQL injection through host names, notes, certificate fields, search text or import files | Bound parameters everywhere; static scan of the source | `tests/sql_test.go` `TestNoSQLIsBuiltFromStrings` |
| Malformed or huge import files exhaust memory or corrupt data | Size and row limits, strict parsing, one transaction, preview first | `internal/inventory` `TestEndpointTransferRoundTripAndImportRules`, `TestParseTarget`, fuzz targets `FuzzParseEndpoints` and `FuzzParseTarget`; `internal/tlscheck` `TestParsePEM`, fuzz target `FuzzParsePEM` |
| A private key in an imported PEM file ends up in the database | Only certificate blocks are read; every other block is ignored | `internal/tlscheck` `TestParsePEM`; `internal/inventory` `TestImportCertificatesTracksFilesAndStopTrackingReleasesThem` |
| A report opened in a spreadsheet runs a formula, or breaks a Markdown table | Cells that start with a formula character are prefixed; table syntax is escaped | `internal/inventory` `TestRenderGuardsSpreadsheetFormulasAndTableSyntax` |
| Command injection through text handed to helper programs | Arguments are passed as separate values without a shell; notification text is passed as data; the terminal launcher quotes every word | `internal/platform` tests, `internal/notify` `TestCommandDelivererPassesTextAsDataNotCode` |
| A crafted link opens an arbitrary site | Only addresses under the configured repository are opened, after a confirmation with the full address | `internal/meta` `TestIsRepositoryURL`, `internal/app` `TestExternalLinksStayInsideTheRepository` |
| Overwriting a foreign file through the database location or an export | A file that is not this application's database is refused; moves copy and never delete; exports do not replace an existing file unless told to | `internal/store` `TestOpenLeavesForeignFilesUntouched`, `internal/app` `TestMoveDatabase`, `TestEveryActionIsPlannedExecutedAndListedAsACapability`, `internal/cli` `TestCertificateImportReportsAndFiles` |
| Other local users read the data | The database, the configuration and exported files are created for the current user only | `internal/store` `TestDatabaseFileIsCreatedWithRestrictivePermissions`, `internal/userconfig` `TestSaveLoadRoundTripWithRestrictivePermissions`, `internal/app` `TestEveryActionIsPlannedExecutedAndListedAsACapability` |
| Host names leak through system notifications | Count only by default; titles are opt-in and reduced to a safe character set and length | `internal/notify` `TestSummary`, `TestSafeTitle`, `TestPublishStoresOnlySafeTitles` |
| Private data leaks through bug reports or diagnostics | The report has fixed fields and is previewed; no paths, no host names | `internal/app` `TestDiagnosticsAndIssueReportCarryNoPrivateData` |
| The start-at-sign-in entry runs something else | The entry is written from a fixed template; unsafe executable paths are refused | `internal/notify` `TestAutostartRejectsUnsafeExecutablePaths` |
| The background notifier changes data or opens connections | It opens the database read-only and contains no check | `internal/notify` `TestNotifierNeverWritesToTheDatabase`, `internal/app` `TestNotifierUsesSettingsAndReadsWithoutWriting` |
| Two windows corrupt the database | WAL mode, busy timeout, retries, revision check on edits | `internal/inventory` `TestConcurrentWritersNeverCorruptData`, `internal/store` `TestConcurrentMigrationFromTwoConnections`, `TestBusyDatabaseIsReportedAndRetried` |
| A damaged or hostile `config.json` | Size limit, strict parsing, validation; a broken file is reported, not used | `internal/userconfig` `TestLoadRejectsBrokenFiles` |
| A vulnerable dependency | Pinned versions, `govulncheck` in CI | `make vuln` |
| A tampered download | The digest shown by GitHub can be compared; the builds are not signed | [INSTALLATION.md](INSTALLATION.md) |

## Accepted risks

- **A check is visible.** The name lookup tells the resolver which host is monitored, the handshake sends the server name in clear text, and the server sees the address of the user's computer. The application states the lookup and the connection before it makes them.
- **The authorization confirmation is a statement by the user.** The application cannot verify it. It makes the decision explicit and records when it was given.
- **Validation by the operating system.** With the default trust store on macOS and Windows, chain validation is done by the operating system, which may fetch a missing intermediate certificate or revocation data from addresses named in the certificate. The application does not control these requests and its tests do not cover them. On Linux, validation happens inside the application without network access.
- **Revocation is not checked** by the application. A revoked certificate that is otherwise valid is shown as valid.
- **The trust file widens trust.** Whoever can change the setting `checks.trust_file` or the file it names can make an untrusted chain appear valid. Both are protected only by the user's account.
- **Name resolution is trusted.** A resolver that returns a wrong address leads to a check of the wrong server; the certificate must still be valid for the monitored name to pass.
- Anything that already runs as the user can read and change the database. The database is not encrypted.
- The downloads are unsigned, so the operating system cannot verify the publisher.
- The helper programs are found through `PATH`. A hostile `PATH` is outside the model, because it already means a compromised account.
- The background notifier relies on the notification service of the desktop; what that service does with the text is outside the application.
