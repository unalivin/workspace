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
| SQL injection through names, notes, search text or import files | Statements are constants with bound parameters; search text becomes a quoted expression; a scan of the storage modules | `sql_stays_in_the_storage_modules_and_is_not_built_from_text`, `search_expressions_are_always_valid_fts_queries`, fuzz target `records_search_expression` |
| Malformed or huge import files exhaust memory or corrupt data | Size and row limits, strict parsing, one transaction, dry run | `unreadable_imports_are_refused_whole`, `bad_rows_are_listed_with_their_number_and_good_rows_still_load`, `a_dry_run_reports_and_changes_nothing`, fuzz target `transfer_parse` |
| Command injection through text handed to helper programs | Arguments are passed as separate values without a shell; notification text is passed as data; the terminal launcher quotes every word | `delivery_commands_pass_text_as_data_never_as_code`, `terminal_windows_are_tried_in_order_and_arguments_are_quoted` |
| A crafted link opens an arbitrary site | Only addresses under the configured repository are handed to the browser, and only when the user chooses the link; a bug report is previewed first | `only_links_below_the_repository_are_accepted`, `only_repository_links_reach_the_browser`, `about_opens_links_only_on_request_and_shows_the_guide_and_diagnostics` |
| Path traversal or overwriting a foreign file through the database location | The path is used as given by the user, a file that is not this application's database is refused, moves copy and never delete | `files_that_are_not_databases_are_refused_and_left_untouched`, `moving_the_database_copies_or_switches_and_never_overwrites` |
| Other local users read the data | Files and directories are created for the current user only | `a_new_database_is_created_private_migrated_and_configured`, `files_are_written_whole_private_and_never_overwritten_silently`, `files_are_exported_privately_and_imported_back` |
| Private data leaks through notifications | Count only by default; titles are reduced to a safe set; never names or notes | `the_system_notification_shows_a_count_and_titles_only_on_request`, `titles_are_cleaned_shortened_and_never_carry_secrets`, `safe_titles_are_bounded_single_line_and_free_of_long_tokens` |
| Private data leaks through bug reports | The report has fixed fields and is previewed; no paths, no record content | `diagnostics_report_the_build_the_database_and_changed_settings`, `about_opens_links_only_on_request_and_shows_the_guide_and_diagnostics` |
| The start-at-sign-in entry runs something else | The entry is written from a fixed template; executable paths that are relative or contain quoting characters are refused | `autostart_entries_are_placed_and_written_per_platform` |
| Two windows corrupt the database | WAL mode, a busy timeout, one transaction per change, revision check on edits | `a_second_writer_waits_then_reports_a_busy_database`, `a_stale_revision_or_a_removed_record_is_a_conflict`, `a_batch_is_created_completely_or_not_at_all` |
| A damaged or hostile `config.json` | Size limit, strict parsing; a broken file is reported, not used | `broken_files_are_reported_with_the_way_out`, `oversized_and_missing_files_are_not_read` |
| A vulnerable dependency | Exact versions in `Cargo.toml` and `Cargo.lock`; `cargo audit` and `cargo deny` in CI | `make vuln` |
| A tampered download | The digest shown by GitHub can be compared; the builds are not signed | [INSTALLATION.md](INSTALLATION.md) |

## Accepted risks

- Anything that already runs as the user can read and change the database. The database is not encrypted.
- The downloads are unsigned, so the operating system cannot verify the publisher.
- The helper programs are found through `PATH`. A hostile `PATH` is outside the model, because it already means a compromised account.
- The background notifier relies on the notification service of the desktop; what that service does with the text is outside the application.
