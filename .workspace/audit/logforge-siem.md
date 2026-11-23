# LogForge-SIEM Audit

## Measurement

Exact output of `scripts/measure.sh logforge-siem` at source audit:

```text
Project logforge-siem, language rust, size class L

production lines                 8494  (need >= 7000)  PASS
test functions                    187  (need >= 60)  PASS
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

The requested `build-prompt-final.md` and its Appendix B are not present in the checkout or available project files. The authoritative eight criteria therefore cannot be reproduced verbatim, and passing status cannot be asserted. The following is a traceability index of the closest existing named integration tests, not a substitution for that missing acceptance table. The latest complete test jobs passed in Windows and Ubuntu; per-test output was not available from the GitHub log download in this environment, so no individual assertion output is invented.

| # | Appendix B criterion | Named test / expected evidence | Output |
|---|---|---|---|
| 1 | Unavailable: Appendix text missing; provisional source lifecycle | `disabling_source_blocks_new_ingest_without_removing_existing_data` | Pending focused CI annotation |
| 2 | Unavailable: Appendix text missing; provisional event query | `filtered_search_supports_parameterized_facets_and_keyset_pages` | Pending focused CI annotation |
| 3 | Unavailable: Appendix text missing; provisional detection correlation | `ingested_events_drive_persistent_threshold_correlation_alerts` | Pending focused CI annotation |
| 4 | Unavailable: Appendix text missing; provisional alert workflow | `bulk_update_only_changes_alerts_matching_every_selector` | Pending focused CI annotation |
| 5 | Unavailable: Appendix text missing; provisional case management | `manages_case_lifecycle_notes_assignment_and_alert_links_atomically` | Pending focused CI annotation |
| 6 | Unavailable: Appendix text missing; provisional retention | `age_policy_prunes_old_events_even_when_event_count_is_below_limit` | Pending focused CI annotation |
| 7 | Unavailable: Appendix text missing; provisional audit | `audit_queries_filter_page_and_preserve_chain_integrity` | Pending focused CI annotation |
| 8 | Unavailable: Appendix text missing; provisional notifications | `generated_alerts_enqueue_and_retry_notifications_with_backoff` | Pending focused CI annotation |

## Required work steps

1. **Acceptance table: NOT COMPLETE.** The source Appendix B is missing. The traceability table above is explicitly provisional; fetch the specified prompt before mapping requirements or claiming acceptance.
2. **Deterministic retention: IMPLEMENTED; aggregate CI PASS on Windows and Ubuntu.** `age_policy_prunes_old_events_even_when_event_count_is_below_limit` fixes time to `2026-11-21T00:00:00Z` and expects event IDs `[1]`; see `crates/logforge-core/tests/retention_plan_tests.rs`.
3. **GUI screens: IN PROGRESS.** Overview and Search have screen-specific service-backed tables with shared filter, sort, column chooser, detail, CSV export, and loading/empty/error/success states. Saved Queries supports copying the selected query. Sources now provides a selected-row enable/disable action through `SiemService::set_source_enabled`; `source_lifecycle_tests::disabling_source_blocks_new_ingest_without_removing_existing_data` covers the lifecycle behavior. Schema queries live SQLite `PRAGMA table_info(events)` through `SiemService::event_schema`; `event_schema_is_read_from_the_migrated_sqlite_database` covers it. Detections now has an enable/disable action for either pattern or persistent correlation rules via their service APIs. Alerts provides an open → acknowledged → resolved action via `SiemService::update_alert_status`. Cases now loads selected case notes and linked alerts in its details panel and transitions case status through `CaseManager`. Retention now displays the active policy, limits, dry-run totals, estimated bytes, and removal reasons above its candidate table. Audit verifies the chain and displays entry count/validity before its service-backed table. Health now displays live ingestion-quality and source-freshness metrics above its source table. Full Appendix A parity and UI interaction tests remain to be verified. Full Appendix A parity and UI interaction tests remain to be verified.
4. **Source comments: PASS for Rust source scan.** `grep -RInE '^[[:space:]]*(//|/\\*|\\*|///|//!|# )' logforge-siem/crates --include='*.rs'` returned only Rust dereference expressions, not comments. Other source-language comment styles need audit before claiming repository-wide completion.
5. **README: PARTIAL.** README has “How it works” text and embeds `docs/architecture.svg`, `docs/flow.svg`, and `docs/data-model.svg`; the SVGs have rendered QA previews in `.workspace/rendered/`. Linked files checked: docs/README.md, SECURITY.md, CONTRIBUTING.md, LICENSE, and all three SVGs exist. Commands still need execution-based verification; there is no local cargo executable.
6. **CI/strip/shipped: NOT COMPLETE.** CI run `45190524538`: Windows **success**, Ubuntu **success**, macOS **queued** at time of this audit. These results predate the latest focused acceptance-test workflow and GUI overview table. To prevent format steps from pushing commits and multiplying queued matrix runs, CI now checks formatting without mutating/pushing the checkout and cancels superseded runs on this branch. The final source revision still needs green Ubuntu/Windows/macOS jobs. Do not strip until all three platforms are green. No smoke run or `scripts/measure.sh --shipped` has been recorded.

## Step 4: source comment scan

Command run from the repository root:

```sh
grep -RInE '^[[:space:]]*(//|///|//!|/\\*|\\*\\/)' logforge-siem/crates || true
grep -nE '^[[:space:]]*(--|/\\*|\\*\\/)' logforge-siem/migrations/*.sql || true
```

Exact combined grep result: **empty (no matching comment lines)**.

## CI diagnostics and fixes

The GUI commit first failed Clippy for an unused page assignment, then reported borrow conflicts, a missing `title` binding, invalid `&usize` slice indexing, and deprecated egui `ComboBox::from_id_source` calls. These issues were fixed in subsequent commits. `scripts/ci-check.sh` was improved to include compiler diagnostic summaries alongside beginning and ending output excerpts. Latest complete jobs (Windows/Ubuntu) passed; macOS remains queued. Earlier `cargo fmt --check` could not run locally because cargo is not installed; current CI format checks passed on Windows and Ubuntu.
