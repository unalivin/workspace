# LogForge-SIEM Audit

## Measurement

Exact output of `scripts/measure.sh logforge-siem` at source audit:

```text
Project logforge-siem, language rust, size class L

production lines                 8626  (need >= 7000)  PASS
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
3. **GUI screens: IN PROGRESS.** Overview and Search have screen-specific service-backed tables with shared filter, sort, column chooser, detail, CSV export, and loading/empty/error/success states. Saved Queries supports copying the selected query. Sources now provides a selected-row enable/disable action through `SiemService::set_source_enabled`; `source_lifecycle_tests::disabling_source_blocks_new_ingest_without_removing_existing_data` covers the lifecycle behavior. Schema queries live SQLite `PRAGMA table_info(events)` through `SiemService::event_schema`; `event_schema_is_read_from_the_migrated_sqlite_database` covers it. Detections now has an enable/disable action for either pattern or persistent correlation rules via their service APIs. Alerts provides an open → acknowledged → resolved action via `SiemService::update_alert_status`. Cases now loads selected case notes and linked alerts in its details panel and transitions case status through `CaseManager`. Retention now displays the active policy, limits, dry-run totals, estimated bytes, and removal reasons above its candidate table. Audit verifies the chain and displays entry count/validity before its service-backed table. Health now displays live ingestion-quality and source-freshness metrics above its source table. The shared table's CSV action writes filtered visible columns to a user-entered or default file path and reports success/failure; clipboard copy remains available. `filter_sort_and_visible_columns_apply_to_exported_rows` tests the shared filter, sorting, column chooser selection and CSV payload. Appendix A text is unavailable, so acceptance mapping remains unverified.

| Screen | Service-backed data | Shared table behavior |
|---|---|---|
| Overview | `overview_counts`, `event_rate_series` | Hourly metrics table and chart; filter, sort, chooser, details, CSV export; loading/empty/error/retry/success |
| Search | `search_events_filtered` with `EventFilter` | Filter text also reaches SQLite query; shared table interactions/states |
| Saved Queries | `list_saved_queries` | Select/copy query; shared table interactions/states |
| Sources | `list_sources`, `set_source_enabled` | Enable/disable selected source; shared table interactions/states |
| Schema | `SiemService::event_schema` via SQLite `PRAGMA table_info(events)` | Live schema; shared table interactions/states |
| Detections | `list_rules`, `correlation_admin::list_rules` | Enable/disable selected rule; shared table interactions/states |
| Alerts | `list_alerts`, `update_alert_status` | Status transition; shared table interactions/states |
| Cases | `CaseManager::list/get/notes/alerts/set_status` | Case details and lifecycle controls; shared table interactions/states |
| Retention | `get_default_policy`, `plan_retention_with_reasons` | Dry-run candidates and reasons; shared table interactions/states |
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
