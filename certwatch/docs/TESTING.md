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

A single test: `go test -tags "sqlite_fts5 nogui" ./internal/records/ -run TestImport`.

The dashboard tests use the toolkit's headless test driver, so they run with the `nogui` tag and without a display. They take a few minutes.

`make check` reports a step as skipped, with the reason, when its tool is missing. `scripts/check.sh --require-all` turns a skip into a failure; CI installs every tool.

## Layers

| Layer | Where | What it proves |
|---|---|---|
| Unit | `internal/*/*_test.go` | Validation, contrast math, quiet hours, launcher decisions, link rules, parsing |
| Database | `internal/store`, `internal/records`, `internal/notify`, `internal/settings` | Real SQLite files in temporary directories: pragmas on every pooled connection, migrations, rollback, read-only mode, busy handling, concurrent writers, the schema snapshot |
| SQL policy | `tests/sql_test.go`, `internal/sqlscan` | No query is built from non-constant text; hot queries use their indexes; the views match a known fixture |
| Service | `internal/app` | A change through one interface's calls is seen through another's, conflicting edits, moving the database, diagnostics without private data |
| Commands | `internal/cli` | Every command through the real argument parser against a real database, including exit codes, prompts and help |
| Terminal UI | `internal/tui` | The model is driven with key presses and the rendered screen is compared |
| Dashboard | `internal/gui` | The real widgets are driven through the headless driver: every page, forms, dialogs, the skin, the window host |
| Capability matrix | `internal/app`, `internal/cli`, `internal/tui`, `internal/gui` | Every capability is listed and reachable in each interface; `docs/COMMANDS.md` matches |
| Fuzz | `make fuzz` | The parsers hold their invariants on arbitrary input |
| Repository | `tests/repo_test.go`, `tests/docs_test.go` | Required files, links between documents, no email address, no source comments, settings documented |
| Scripts | `scripts/tests/*.bats` | Setup, checks, packaging, release verification and publishing in dry-run and against fakes |

## Fuzz targets

| Target | Package | Invariant |
|---|---|---|
| `FuzzParse` | `internal/records` | Import parsing never panics and accepted rows are valid |
| `FuzzSearchExpressionIsAlwaysAValidQuery` | `internal/records` | Any search text becomes a query SQLite accepts |
| `FuzzNormalize` | `internal/settings` | A normalized setting value is stable and valid |
| `FuzzSafeTitle` | `internal/notify` | Notification titles stay inside the safe character set and length |
| `FuzzAdjustForContrast` | `internal/theme` | An adjusted accent meets the contrast rule or is rejected |
| `FuzzParseAndSearch` | `internal/guide` | Guide parsing and search never panic |

## Test data

- `testdata/schema.snapshot`: the expected schema. Update with `go test -tags sqlite_fts5 ./internal/store/ -update` after adding a migration, and review the difference.
- `testdata/fixtures/`: import files and the fixture behind the Overview numbers.
- `testdata/sqlscan/`: source files with and without string-built SQL for the scanner tests.

Tests never touch the real user directories: each one gets a temporary home and database.

## Requirements and their tests

| Requirement | Test |
|---|---|
| The three interfaces offer the same capabilities | `TestCapabilityMatrixIsComplete`, `TestEveryCapabilityHasACommand`, `TestEveryTerminalCapabilityIsReachable`, `TestEveryDashboardCapabilityIsReachable` |
| A change in one interface appears in the others | `TestChangesMadeInOneFrontEndReachTheOther`, `TestExternalChangesKeepSelectionAndInput`, `TestExternalChangesAppearOnTheNextTick` |
| Conflicting edits lose nothing | `TestConflictingEditsAreDetected`, `TestConflictingEditKeepsTheForm` |
| Two writers never corrupt data | `TestConcurrentWritersNeverCorruptData`, `TestConcurrentMigrationFromTwoConnections`, `TestBusyDatabaseIsReportedAndRetried` |
| Settings are validated, resettable and never dropped | `TestSetGetResetRoundTrip`, `TestListKeepsUnknownAndInvalidStoredValues`, `TestOverridesAreReportedWithTheirSource` |
| Themes meet the contrast rules | `TestEveryThemeMeetsContrastRules`, `TestSeverityColorsIgnoreTheAccentAndStayDistinct`, `TestCustomAccentIsKeptAdjustedOrRejected` |
| The toolkit look is replaced | `TestSkinUsesTheTokens`, `TestApplicationIconsComeFromOneSet` |
| The launcher never asks in scripts or without a display | `TestDecide`, `TestDecideNeverPromptsWithoutAnInteractiveSession`, `TestLaunchDecisions` |
| Notifications follow the rules and leak nothing | `TestEvaluate`, `TestInQuietHours`, `TestPublishAppliesRulesAndDeduplicates`, `TestPublishStoresOnlySafeTitles`, `TestSummary` |
| The notifier only reads | `TestNotifierNeverWritesToTheDatabase` |
| Start at sign-in is per user and safe | `TestAutostartInstallUninstallStatus`, `TestAutostartRejectsUnsafeExecutablePaths` |
| Links stay inside the repository | `TestIsRepositoryURL`, `TestExternalLinksStayInsideTheRepository`, `TestOpenRepositoryURLOnlyOpensRepositoryLinksAsOneArgument` |
| Bug reports and diagnostics carry no private data | `TestDiagnosticsAndIssueReportCarryNoPrivateData` |
| Files are private to the user | `TestDatabaseFileIsCreatedWithRestrictivePermissions`, `TestSaveLoadRoundTripWithRestrictivePermissions` |
| A foreign file is never overwritten | `TestOpenLeavesForeignFilesUntouched`, `TestOpenRejectsAForeignFile` |
| Demo data is labeled and removable | `TestDemoDataIsLabelledIdempotentAndRemovable` |
| Icons are valid in every format | `TestICOListsValidPictures`, `TestICNSListsValidPictures`, `TestCommittedAssetsMatchTheGenerator` |
| No SQL from strings | `TestNoSQLIsBuiltFromStrings` |
| Release rules | `scripts/tests/release.bats`, `scripts/tests/publish.bats` |

## Not covered by automated tests

The dashboard on a real display with real input, a real terminal emulator, the Windows console attachment, display of system notifications by the desktop, start at sign-in at a real sign-in, and starting a packaged download on a clean machine. See [PLATFORM_SUPPORT.md](PLATFORM_SUPPORT.md).
