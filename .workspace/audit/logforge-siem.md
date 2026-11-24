# LogForge-SIEM Audit

## Measurement

Exact output of `scripts/measure.sh logforge-siem` at source audit:

```text
Project logforge-siem, language rust, size class L

production lines                 8664  (need >= 7000)  PASS
test functions                    188  (need >= 60)  PASS
screens in GUI and TUI             11  (need >= 9)  PASS
migrations                          8  (need >= 2)  PASS
SQL tables                         20  (need >= 8)  PASS
icon.svg bytes                   1520  (need >= 1500)  PASS
icon PNG sizes                      7  (need >= 7)  PASS
.ico and .icns files                2  (need >= 2)  PASS
diagrams                            3  (need >= 3)  PASS

logforge-siem meets the floor.
```

## Eight acceptance criteria

The requested `build-prompt-final.md` and its Appendix B are not present in the checkout or available project files. The authoritative eight criteria therefore cannot be reproduced verbatim, and passing status cannot be asserted. The following is a traceability index of the closest existing named integration tests, not a substitution for that missing acceptance table. CI run `45245031756` on commit `723fa0f` passed on Ubuntu, Windows, and macOS; its Ubuntu check annotations expose each exact acceptance test output listed below.

| # | Appendix B criterion | Named test / expected evidence | Output |
|---|---|---|---|
| 1 | Unavailable: Appendix text missing; provisional source lifecycle | `disabling_source_blocks_new_ingest_without_removing_existing_data` | `test disabling_source_blocks_new_ingest_without_removing_existing_data ... ok` |
| 2 | Unavailable: Appendix text missing; provisional event query | `filtered_search_supports_parameterized_facets_and_keyset_pages` | `test filtered_search_supports_parameterized_facets_and_keyset_pages ... ok` |
| 3 | Unavailable: Appendix text missing; provisional detection correlation | `ingested_events_drive_persistent_threshold_correlation_alerts` | `test ingested_events_drive_persistent_threshold_correlation_alerts ... ok` |
| 4 | Unavailable: Appendix text missing; provisional alert workflow | `bulk_update_only_changes_alerts_matching_every_selector` | `test bulk_update_only_changes_alerts_matching_every_selector ... ok` |
| 5 | Unavailable: Appendix text missing; provisional case management | `manages_case_lifecycle_notes_assignment_and_alert_links_atomically` | `test manages_case_lifecycle_notes_assignment_and_alert_links_atomically ... ok` |
| 6 | Unavailable: Appendix text missing; provisional retention | `age_policy_prunes_old_events_even_when_event_count_is_below_limit` | `test age_policy_prunes_old_events_even_when_event_count_is_below_limit ... ok` |
| 7 | Unavailable: Appendix text missing; provisional audit | `audit_queries_filter_page_and_preserve_chain_integrity` | `test audit_queries_filter_page_and_preserve_chain_integrity ... ok` |
| 8 | Unavailable: Appendix text missing; provisional notifications | `generated_alerts_enqueue_and_retry_notifications_with_backoff` | `test generated_alerts_enqueue_and_retry_notifications_with_backoff ... ok` |

## Required work steps

1. **Acceptance table: NOT COMPLETE.** The source Appendix B is missing. The traceability table above is explicitly provisional; fetch the specified prompt before mapping requirements or claiming acceptance.
2. **Deterministic retention: IMPLEMENTED; aggregate CI PASS on Windows and Ubuntu.** `age_policy_prunes_old_events_even_when_event_count_is_below_limit` fixes time to `2026-11-21T00:00:00Z` and expects event IDs `[1]`; see `crates/logforge-core/tests/retention_plan_tests.rs`.
3. **GUI screens: IMPLEMENTED against the available requirements; CI PASS.** All 11 operational screens now read and mutate through `SiemService` APIs; the GUI screen renderer and Overview component no longer access the database directly. Shared tables provide filtering, sorting, column selection, record details, CSV export, and loading/empty/error/success states. The Overview, Health, Audit, Detections, Retention, and Cases follow-up work was kept in separate screen-scoped commits (`abf949e`, `905febd`, `9a89869`, `1c7161b`, `e757734`, `c440b28`), followed by removal of the unreachable duplicate legacy screen paths (`1c2bf2f`). Sources and detections have selected-row enable/disable actions; alerts support open → acknowledged → resolved; cases show notes and linked alerts and support status transitions; retention shows policy limits and dry-run estimates; audit shows chain verification; Health shows ingestion quality and source freshness. Saved Queries supports selecting and copying a query. CSV export respects the active filter and visible columns and reports success/failure. The shipped UI build and tests passed in CI run `45430629438`. The requested Appendix A acceptance mapping remains unverifiable because that source text is unavailable.

| Screen | Service-layer data | Shared table behavior / screen action |
|---|---|---|
| Overview | `overview_counts`, `event_rate_series`, `count_events_in_range` | Hourly metrics table and chart; filter, sort, chooser, details, CSV export; loading/empty/error/retry/success |
| Search | `search_events_filtered` with `EventFilter` | Server-side query filter plus shared table interactions/states |
| Saved Queries | `list_saved_queries` | Select/copy query; shared table interactions/states |
| Sources | `list_sources`, `set_source_enabled` | Enable/disable selected source; shared table interactions/states |
| Schema | `event_schema` via migrated SQLite schema query | Live schema table; shared table interactions/states |
| Detections | `list_rules`, `list_correlation_rules`, enable methods | Enable/disable selected rule; shared table interactions/states |
| Alerts | `list_alerts`, `update_alert_status` | Status transition; shared table interactions/states |
| Cases | `list_cases`, `get_case`, `case_notes`, `case_alerts`, `set_case_status` | Case detail and lifecycle controls; shared table interactions/states |
| Retention | `default_retention_policy`, `plan_retention_with_reasons` | Dry-run candidates and reasons; shared table interactions/states |
| Audit | `query_audit_log`, `verify_audit_chain` | Chain verification and evidence rows; shared table interactions/states |
| Health | `source_health`, `ingestion_stats`, `source_liveness` | Live health counters and source table; shared table interactions/states |

4. **Source comments: PASS for the requested scope.** The Rust comment-line scan in `logforge-siem/crates` and SQL comment-line scan in `logforge-siem/migrations/*.sql` both returned no matches. Exact commands and combined output are recorded below. This is limited to the directories and file types requested.
5. **README: STATIC AUDIT PASS; shipped CLI smoke PASS.** “How it works” embeds the three rendered SVG diagrams and explains one demo line from `demo-auth` through parsing, UTC normalization, redaction, SHA-256 deduplication, SQLite/FTS5 and the matching alert/audit record. README-relative links, image targets, Cargo package/binary names and CLI declarations passed the Python source audit. CI run `45418548129` executed `--version`, demo load, overview and the `search "Failed"` example against the same smoke database on Ubuntu. The interactive `logforge-app --ui tui` launch is source-validated, not run in a non-interactive CI terminal.
6. **CI/strip/shipped: PASS.** After CI run `45245031756` passed Ubuntu, Windows and macOS on `723fa0f`, `scripts/strip-dev-files.sh logforge-siem` ran and was committed as `dd0b7cb`. Post-strip CI run `45417504371` passed Ubuntu, Windows and macOS, including the README `search "Failed"` smoke added in commit `35e6a03`; Ubuntu passed build, both test steps, all four CLI smoke commands, and `scripts/measure.sh --shipped logforge-siem`. The shipped-only measurement was also run locally and printed `No development files in logforge-siem.`

## Step 4: source comment scan

Command run from the repository root:

```sh
grep -RInE '^[[:space:]]*(//|///|//!|/\\*|\\*\\/)' logforge-siem/crates || true
grep -nE '^[[:space:]]*(--|/\\*|\\*\\/)' logforge-siem/migrations/*.sql || true
```

Exact combined grep result: **empty (no matching comment lines)**.

## CI diagnostics and fixes

The GUI commit first failed Clippy for an unused page assignment, then reported borrow conflicts, a missing `title` binding, invalid `&usize` slice indexing, and deprecated egui `ComboBox::from_id_source` calls; subsequent commits fixed them. `scripts/ci-check.sh` includes compiler diagnostic summaries alongside beginning and ending output excerpts. CI `45245031756` passed on Ubuntu, Windows and macOS; post-strip run `45417504371` also passed on all three platforms, including the Ubuntu README command smoke and shipped-only measurement.
