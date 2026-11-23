# LogForge-SIEM Audit

## Measurement

Exact output of `scripts/measure.sh logforge-siem` at source audit:

```text
Project logforge-siem, language rust, size class L

production lines                 8318  (need >= 7000)  PASS
test functions                    185  (need >= 60)  PASS
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
| 1 | Unavailable: source text missing | `source_lifecycle_tests` (source lifecycle) | Aggregate workspace tests passed: Windows, Ubuntu; individual test output not retained |
| 2 | Unavailable: source text missing | `event_query_tests` (event query) | Aggregate workspace tests passed: Windows, Ubuntu; individual test output not retained |
| 3 | Unavailable: source text missing | `correlation_service_tests` (rule service) | Aggregate workspace tests passed: Windows, Ubuntu; individual test output not retained |
| 4 | Unavailable: source text missing | `alert_bulk_workflow_tests` (alert workflow) | Aggregate workspace tests passed: Windows, Ubuntu; individual test output not retained |
| 5 | Unavailable: source text missing | `case_management_tests` (case workflow) | Aggregate workspace tests passed: Windows, Ubuntu; individual test output not retained |
| 6 | Unavailable: source text missing | `retention_plan_tests` (retention planning) | Aggregate workspace tests passed: Windows, Ubuntu; individual test output not retained |
| 7 | Unavailable: source text missing | `audit_query_tests` (audit querying) | Aggregate workspace tests passed: Windows, Ubuntu; individual test output not retained |
| 8 | Unavailable: source text missing | `notification_outbox_tests` (notification outbox) | Aggregate workspace tests passed: Windows, Ubuntu; individual test output not retained |

## Required work steps

1. **Acceptance table: NOT COMPLETE.** The source Appendix B is missing. The traceability table above is explicitly provisional; fetch the specified prompt before mapping requirements or claiming acceptance.
2. **Deterministic retention: IMPLEMENTED; aggregate CI PASS on Windows and Ubuntu.** `age_policy_prunes_old_events_even_when_event_count_is_below_limit` fixes time to `2026-11-21T00:00:00Z` and expects event IDs `[1]`; see `crates/logforge-core/tests/retention_plan_tests.rs`.
3. **GUI screens: IN PROGRESS.** The 10 operational screens use service-backed tables with filter, sort, column chooser, detail, export and loading/empty/error/success handling. Overview now also shows a service-backed hourly table with the shared interactions and its own loading, empty, error/retry and success states. Full Appendix A parity and UI-level interaction tests remain to be verified; About and Settings are support views rather than counted operational screens.
4. **Source comments: PASS for Rust source scan.** `grep -RInE '^[[:space:]]*(//|/\\*|\\*|///|//!|# )' logforge-siem/crates --include='*.rs'` returned only Rust dereference expressions, not comments. Other source-language comment styles need audit before claiming repository-wide completion.
5. **README: PARTIAL.** README has “How it works” text and embeds `docs/architecture.svg`, `docs/flow.svg`, and `docs/data-model.svg`; the SVGs have rendered QA previews in `.workspace/rendered/`. Linked files checked: docs/README.md, SECURITY.md, CONTRIBUTING.md, LICENSE, and all three SVGs exist. Commands still need execution-based verification; there is no local cargo executable.
6. **CI/strip/shipped: NOT COMPLETE.** CI run `45190524538`: Windows **success**, Ubuntu **success**, macOS **queued** at time of this audit. Format check and Clippy passed on the Windows and Ubuntu jobs; both build and test (no-GUI/default) steps also passed. Do not strip until all three platforms are green. No smoke run or `scripts/measure.sh --shipped` has been recorded.

## Step 4: source comment scan

Command run from the repository root:

```sh
grep -RInE '^[[:space:]]*(//|///|//!|/\\*|\\*\\/)' logforge-siem/crates || true
grep -nE '^[[:space:]]*(--|/\\*|\\*\\/)' logforge-siem/migrations/*.sql || true
```

Exact combined grep result: **empty (no matching comment lines)**.

## CI diagnostics and fixes

The GUI commit first failed Clippy for an unused page assignment, then reported borrow conflicts, a missing `title` binding, invalid `&usize` slice indexing, and deprecated egui `ComboBox::from_id_source` calls. These issues were fixed in subsequent commits. `scripts/ci-check.sh` was improved to include compiler diagnostic summaries alongside beginning and ending output excerpts. Latest complete jobs (Windows/Ubuntu) passed; macOS remains queued. Earlier `cargo fmt --check` could not run locally because cargo is not installed; current CI format checks passed on Windows and Ubuntu.
