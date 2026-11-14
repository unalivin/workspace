# Testing

## Running

```bash
make check             # everything below that the machine can run, with a summary
make test-nogui        # all tests of the no-GUI build
make test              # all tests of the dashboard build (needs the windowing headers)
make race-or-sanitize  # tests under the race detector
make fuzz              # every fuzz target for FUZZTIME (default 5s each)
make fuzz FUZZTIME=2m
make shell             # shellcheck, shfmt and the bats tests of scripts/
make docs-check assets-check notices-check   # generated files are current
make vuln              # govulncheck
make screenshots       # render the dashboard screens into dist/screens
```

A single test: `go test -tags "sqlite_fts5 nogui" ./internal/inventory/ -run TestEndpointTransfer`.

The dashboard tests use the toolkit's headless test driver, so they run without a display. They take about half a minute.

No test connects to the internet. TLS checks are tested against servers that the tests start on the loopback interface, with certificates from an authority generated at run time (`internal/certgen`). No key or certificate is stored in the repository.

`make check` reports a step as skipped, with the reason, when its tool is missing. `scripts/check.sh --require-all` turns a skip into a failure; CI installs every tool.

## Layers

| Layer | Where | What it proves |
|---|---|---|
| Unit | `internal/*/*_test.go` | Validation, contrast math, quiet hours, launcher decisions, link rules, parsing |
| TLS | `internal/tlscheck` | Real handshakes with local servers: trusted, untrusted, expired, self-signed, wrong name, weak key, old protocol versions, a stalled handshake, a server that does not speak TLS, hostile text in a certificate |
| Database | `internal/store`, `internal/inventory`, `internal/notify`, `internal/settings` | Real SQLite files in temporary directories: pragmas on every pooled connection, migrations, rollback, read-only mode, busy handling, concurrent writers, the schema snapshot |
| SQL policy | `tests/sql_test.go`, `internal/sqlscan` | No query is built from non-constant text; hot queries use their indexes; the views match a known fixture |
| Service | `internal/app` | Every view action is planned and executed, checks need an authorized endpoint and disclose their connections, a change through one interface's calls is seen through another's, conflicting edits, moving the database, diagnostics without private data |
| Commands | `internal/cli` | Every command through the real argument parser against a real database, including exit codes, prompts and help |
| Terminal UI | `internal/tui` | The model is driven with key presses and the rendered screen is compared |
| Dashboard | `internal/gui` | The real widgets are driven through the headless driver: every page, forms, dialogs, the skin, the window host |
| Capability matrix | `internal/app`, `internal/cli`, `internal/tui`, `internal/gui` | Every capability is listed and reachable in each interface; `docs/COMMANDS.md` matches |
| Fuzz | `make fuzz` | The parsers hold their invariants on arbitrary input |
| Repository | `tests/repo_test.go`, `tests/docs_test.go` | Required files, links between documents, no email address, no source comments, settings documented, tests named in documents exist, network access only in `internal/tlscheck`, verification never disabled |
| Scripts | `scripts/tests/*.bats` | Setup, checks, packaging, release verification and publishing in dry-run and against fakes |

## Fuzz targets

| Target | Package | Invariant |
|---|---|---|
| `FuzzParseEndpoints` | `internal/inventory` | Parsing an endpoint list never panics and accepted rows are valid endpoints |
| `FuzzParseTarget` | `internal/inventory` | Any text is either refused or becomes a valid host and port |
| `FuzzParsePEM` | `internal/tlscheck` | PEM parsing never panics, stays inside its limits and yields printable descriptions |
| `FuzzPrintable` | `internal/tlscheck` | Filtered text is valid, printable, bounded and stable |
| `FuzzNormalize` | `internal/settings` | A normalized setting value is stable and valid |
| `FuzzSafeTitle` | `internal/notify` | Notification titles stay inside the safe character set and length |
| `FuzzAdjustForContrast` | `internal/theme` | An adjusted accent meets the contrast rule or is rejected |
| `FuzzParseAndSearch` | `internal/guide` | Guide parsing and search never panic |

## Test data

- `testdata/schema.snapshot`: the expected schema. Update with `go test -tags sqlite_fts5 ./internal/store/ -update` after adding a migration, and review the difference.
- `testdata/fixtures/overview.sql`: the fixture behind the Overview numbers and the view tests.
- `testdata/sqlscan/`: source files with and without string-built SQL for the scanner tests.

Tests never touch the real user directories: each one gets a temporary home and database.

## Requirements and their tests

| Requirement | Test |
|---|---|
| A host is contacted only after the user confirmed authorization | `TestEndpointLifecycleNeedsAuthorization`, `TestCheckRunsOnlyAfterAnAuthorizedEndpointAndDisclosesTheConnection`, `TestEndpointTransferRoundTripAndImportRules` |
| Every network action discloses its connections and asks, also with confirmations off | `TestCheckRunsOnlyAfterAnAuthorizedEndpointAndDisclosesTheConnection`, `TestWatchChecksDueEndpointsOnlyAfterConfirmation`, `TestRemoveWithoutConfirmationWhenTheSettingIsOffButChecksStillAsk`, `TestNetworkChecksAskEvenWhenConfirmationsAreOff` |
| Only the check package opens connections; verification is never disabled | `TestNetworkAccessStaysInTheCheckPackage`, `TestCheckUsesTheGivenDialerAndNothingElse` |
| Demo endpoints are never contacted; demo data is labeled and removable | `TestDemoEndpointsAreNeverContacted`, `TestDemoDataIsLabeledAndRemovable` |
| A trusted chain is accepted and its details recorded | `TestCheckAcceptsATrustedChain`, `TestRecordStoresTheChainAndAlertsOnceForEachThreshold` |
| An untrusted chain is recorded and never shown as valid | `TestCheckKeepsVerificationOnAndStillRecordsAnUntrustedChain`, `TestCheckReportsASelfSignedCertificateAndAWeakKey` |
| A name mismatch and an expired certificate are reported | `TestCheckReportsAHostnameMismatch`, `TestCheckReportsAnExpiredCertificate` |
| Accepted TLS versions are observed when probing is on | `TestCheckObservesAcceptedProtocolVersions` |
| Unreachable, slow and non-TLS servers are reported within the timeout | `TestCheckReportsAnUnreachableHost`, `TestCheckStopsAtTheTimeout`, `TestCheckReportsAServerThatDoesNotSpeakTLS`, `TestClampTimeout` |
| Text from servers cannot forge the display | `TestCheckNeutralizesHostileTextInCertificates`, `TestPrintableKeepsTextAndLimitsLength` |
| Expiry alerts fire once per certificate and threshold | `TestRecordStoresTheChainAndAlertsOnceForEachThreshold`, `TestHousekeepingChecksDueEndpointsAndRaisesExpiryAlerts` |
| Validation alerts fire on the transition; a changed certificate is detected | `TestRecordDetectsChangesAndValidationTransitions` |
| Disabled endpoints are left out of counts and alerts | `TestDisabledEndpointsAreLeftOutOfCountsAndAlerts` |
| History is pruned and unreferenced certificates are removed | `TestRecordPrunesHistoryAndRemovesUnreferencedCertificates` |
| The Overview counts equal the lists they open, in every interface | `TestOverviewBucketsMatchTheCertificateFilters`, `TestOverviewTilesAndEveryViewShowTheSameNumbers`, `TestViewsMatchTheFixture`, `TestFirstStartAndOverview` |
| Additional roots come from the configured file | `TestAdditionalTrustedRootsComeFromTheConfiguredFile` |
| PEM import tracks certificates and never stores keys | `TestParsePEM`, `TestImportCertificatesTracksFilesAndStopTrackingReleasesThem` |
| Reports hold the same rows in every format and are safe to open | `TestReportsRenderTheSameRowsInEveryFormat`, `TestRenderGuardsSpreadsheetFormulasAndTableSyntax` |
| Every view action exists in every interface | `TestEveryActionIsPlannedExecutedAndListedAsACapability`, `TestEveryCapabilityHasACommand`, `TestEveryViewActionKeyStartsItsAction`, `TestEveryViewActionHasAButtonAndACapability` |
| The three interfaces offer the same capabilities | `TestCapabilityMatrixIsComplete`, `TestEveryCapabilityHasACommand`, `TestEveryTerminalCapabilityIsReachable`, `TestEveryDashboardCapabilityIsReachable` |
| A change in one interface appears in the others | `TestChangesMadeInOneFrontEndReachTheOther`, `TestExternalChangesKeepSelectionAndInput`, `TestExternalChangesAppearOnTheNextTick` |
| Conflicting edits lose nothing | `TestChangesMadeInOneFrontEndReachTheOther`, `TestConflictingEditKeepsTheForm` |
| Two writers never corrupt data | `TestConcurrentWritersNeverCorruptData`, `TestConcurrentMigrationFromTwoConnections`, `TestBusyDatabaseIsReportedAndRetried` |
| Settings are validated, resettable and never dropped | `TestSetGetResetRoundTrip`, `TestListKeepsUnknownAndInvalidStoredValues`, `TestOverridesAreReportedWithTheirSource` |
| The endpoint table follows the column and sort settings | `TestEndpointTableFollowsTheColumnAndSortSettings`, `TestEndpointColumnsFollowTheSettingAndBarsAreDrawn` |
| Themes meet the contrast rules | `TestEveryThemeMeetsContrastRules`, `TestSeverityColorsIgnoreTheAccentAndStayDistinct`, `TestCustomAccentIsKeptAdjustedOrRejected` |
| The toolkit look is replaced | `TestSkinUsesTheTokens`, `TestApplicationIconsComeFromOneSet` |
| The launcher never asks in scripts or without a display | `TestDecide`, `TestDecideNeverPromptsWithoutAnInteractiveSession`, `TestLaunchDecisions` |
| Alert rules are applied and titles are safe | `TestEvaluate`, `TestInQuietHours`, `TestPublishAppliesRulesAndDeduplicates`, `TestPublishStoresOnlySafeTitles`, `TestSummary` |
| The notifier only reads | `TestNotifierNeverWritesToTheDatabase`, `TestNotifierUsesSettingsAndReadsWithoutWriting` |
| Start at sign-in is per user and safe | `TestAutostartInstallUninstallStatus`, `TestAutostartRejectsUnsafeExecutablePaths` |
| Links stay inside the repository | `TestIsRepositoryURL`, `TestExternalLinksStayInsideTheRepository`, `TestOpenRepositoryURLOnlyOpensRepositoryLinksAsOneArgument` |
| Bug reports and diagnostics carry no private data | `TestDiagnosticsAndIssueReportCarryNoPrivateData` |
| Files are private to the user and existing files are not replaced | `TestDatabaseFileIsCreatedWithRestrictivePermissions`, `TestSaveLoadRoundTripWithRestrictivePermissions`, `TestEveryActionIsPlannedExecutedAndListedAsACapability` |
| A foreign file is never overwritten | `TestOpenLeavesForeignFilesUntouched`, `TestOpenRejectsAForeignFile` |
| Icons are valid in every format | `TestICOListsValidPictures`, `TestICNSListsValidPictures`, `TestCommittedAssetsMatchTheGenerator` |
| No SQL from strings; hot queries use their indexes | `TestNoSQLIsBuiltFromStrings`, `TestHotQueriesUseTheirIndexes` |
| Documents name tests that exist and commands that exist | `TestTestsNamedInDocumentsExist`, `TestGeneratedCommandTablesAreCurrent`, `TestCommandReferenceIsDocumented` |
| Release rules | `scripts/tests/release.bats`, `scripts/tests/publish.bats` |

## Not covered by automated tests

Servers on the internet and validation by the trust store of the operating system, the dashboard on a real display with real input, a real terminal emulator, the Windows console attachment, display of system notifications by the desktop, start at sign-in at a real sign-in, and starting a packaged download on a clean machine. See [PLATFORM_SUPPORT.md](PLATFORM_SUPPORT.md).
