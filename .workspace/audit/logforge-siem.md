# LogForge-SIEM Audit

## Measurement

Exact output of `scripts/measure.sh logforge-siem`:

```text
Project logforge-siem, language rust, size class L

production lines                 8239  (need >= 7000)  PASS
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

The requested `build-prompt-final.md` and its Appendix B are not present in the checkout or available project files. The authoritative eight criteria therefore cannot be reproduced verbatim, and passing status cannot be asserted. The following is a traceability index of the closest existing named integration tests, not a substitution for that missing acceptance table. Test output is pending a successful CI run for the current source revision.

| # | Appendix B criterion | Named test / expected evidence | Output |
|---|---|---|---|
| 1 | Unavailable: source text missing | `source_lifecycle_tests` (source lifecycle) | Not run for current revision |
| 2 | Unavailable: source text missing | `event_query_tests` (event query) | Not run for current revision |
| 3 | Unavailable: source text missing | `correlation_service_tests` (rule service) | Not run for current revision |
| 4 | Unavailable: source text missing | `alert_bulk_workflow_tests` (alert workflow) | Not run for current revision |
| 5 | Unavailable: source text missing | `case_management_tests` (case workflow) | Not run for current revision |
| 6 | Unavailable: source text missing | `retention_plan_tests` (retention planning) | Not run for current revision |
| 7 | Unavailable: source text missing | `audit_query_tests` (audit querying) | Not run for current revision |
| 8 | Unavailable: source text missing | `notification_outbox_tests` (notification outbox) | Not run for current revision |

## Required work steps

1. **Acceptance table: NOT COMPLETE.** The source Appendix B is missing. The traceability table above is explicitly provisional; fetch the specified prompt before mapping requirements or claiming acceptance.
2. **Deterministic retention: IMPLEMENTED, CI VERIFICATION PENDING.** `age_policy_prunes_old_events_even_when_event_count_is_below_limit` fixes time to `2026-11-21T00:00:00Z` and expects event IDs `[1]`; the corresponding expected assertions are in `crates/logforge-core/tests/retention_plan_tests.rs`.
3. **GUI screens: NOT COMPLETE.** Shared data-backed screens, tables, filter/sort/chooser/detail/export interactions and service states were added, but per-screen parity for all 11 screens (including all four states) is not demonstrated. GUI still requires complete review and current CI verification.
4. **Source comments: PASS for Rust source scan.** `grep -RInE '^[[:space:]]*(//|/\\*|\\*|///|//!|# )' logforge-siem/crates --include='*.rs'` returned only three Rust dereference expressions, not comments. Other source-language comment styles need audit before claiming repository-wide completion.
5. **README: PARTIAL.** README has “How it works” text and embeds `docs/architecture.svg`, `docs/flow.svg`, and `docs/data-model.svg`. Linked files checked: docs/README.md, SECURITY.md, CONTRIBUTING.md, LICENSE, and all three SVGs exist. Commands still need execution-based verification; this container has no cargo executable.
6. **CI/strip/shipped: NOT COMPLETE.** Latest confirmed complete three-platform CI is from an older revision. Current revision is pending/failing as of last query; do not strip. No smoke run or `scripts/measure.sh --shipped` has been recorded.

## Current verification

`cargo fmt --check` was unavailable locally because cargo is not installed. GitHub Actions is the Rust validation path. Latest CI annotation observed on the GUI commit: Clippy failed in `logforge-tui/src/lib.rs` because `page = 0` was assigned but overwritten before use. That assignment was removed and pushed; the retry run must be checked before recording a result. CI annotations from the previous successful commit contained only runner/action notices.
