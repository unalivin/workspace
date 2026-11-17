# Testing

## Running

```bash
make check             # everything below that the machine can run, with a summary
make test-nogui        # all tests of the no-GUI build
make test              # all tests of the workspace, dashboard included
make race-or-sanitize  # the no-GUI tests under AddressSanitizer (scripts/sanitize.sh)
make fuzz              # every fuzz target for FUZZTIME (default 5s each)
make fuzz FUZZTIME=2m
make shell             # shellcheck, shfmt and the bats tests of scripts/
make docs-check assets-check notices-check   # generated files are current
make vuln              # cargo audit and cargo deny
make screenshots       # render the dashboard screens into dist/screens
```

A single test: `cargo test -p secretsentry-core listing_filters`. Tests of one crate: `cargo test -p secretsentry-tui`.

The dashboard tests need no display. They run the real interface code, feed it pointer and key events, read the text it paints, and paint the result with a software renderer that is part of the dashboard crate. The dashboard crate is compiled with optimization in test builds so that they take seconds.

`make check` reports a step as skipped, with the reason, when its tool is missing. `scripts/check.sh --require-all` turns a skip into a failure; CI installs every tool.

## The sanitizer run

Rust has no race detector that works on the stable compiler, and the workspace forbids `unsafe`, which rules out data races in the application's own code. `make race-or-sanitize` therefore checks memory safety across the dependencies:

- On Linux and macOS the tests of the no-GUI build are compiled with AddressSanitizer. This covers the Rust code, including the `unsafe` code inside dependencies such as the SQLite binding. The SQLite C code is not instrumented; its allocations go through the checked allocator. On Linux the run also reports memory that is never freed.
- The sanitizer flag is unlocked on the pinned stable compiler with `RUSTC_BOOTSTRAP=1`. No nightly toolchain is installed.
- On Windows the sanitizer is not available for this toolchain. The same tests run as an optimized build with overflow checks and debug assertions.
- The dashboard crate is not part of the sanitizer run.

## Layers

| Layer | Where | What it proves |
|---|---|---|
| Unit | the `tests` module at the end of each file in `crates/secretsentry-core/src` | Validation, contrast math, quiet hours, launcher decisions, link rules, parsing |
| Property | `proptest!` blocks in the same modules | Parsers and normalizers hold their invariants on generated input |
| Database | `store.rs`, `records.rs`, `notify.rs`, `settings.rs` in `secretsentry-core` | Real SQLite files in temporary directories: pragmas, migrations, rollback, read-only mode, a busy database, the schema snapshot, hot queries use their indexes, the Overview views match a known fixture |
| Service | `service.rs` in `secretsentry-core` | A change through one connection is seen through another, moving the database, diagnostics, the capability list |
| Commands | `crates/secretsentry-app/src/tests.rs` | Every command through the real argument parser against a real database, including exit codes, prompts and help |
| Terminal UI | `crates/secretsentry-tui/src/tests.rs` | The model is driven with key presses; state and the painted screen are checked |
| Dashboard | `crates/secretsentry-gui/src/tests.rs` and the module tests beside it | The interface code is driven with events: every page, forms, confirmations, the skin, icons, the table layout |
| Capability matrix | `service.rs` and the tests of the three interface crates | Every capability is listed once, names a real command, is painted somewhere in the dashboard and is shown in the terminal UI; `docs/COMMANDS.md` matches |
| Fuzz | `fuzz/fuzz_targets`, `make fuzz` | The parsers hold their invariants on arbitrary input |
| Repository | `crates/secretsentry-app/tests/repository.rs` | Required files, links between documents, no email address, no source comments, settings documented, test names in the documents exist, the generated command tables are current, SQL only in the storage modules, one version number everywhere |
| Scripts | `scripts/tests/*.bats` | Setup, checks, packaging, release verification and publishing in dry-run and against fakes |

## Fuzz targets

`scripts/fuzz.sh` runs each target with `cargo fuzz` (libFuzzer) on Linux and macOS. CI runs them for five seconds each on Linux, which mainly proves that they build and start; run them longer by hand. The same invariants are also checked by the property tests on every platform.

| Target | Module | Invariant |
|---|---|---|
| `transfer_parse` | `transfer` | Import parsing of CSV and JSON never panics |
| `csv_parse` | `csv` | CSV parsing never panics and respects the row limit |
| `records_search_expression` | `records` | Any search text becomes a query SQLite accepts |
| `settings_normalize` | `settings` | A normalized setting value is stable and valid |
| `notify_safe_title` | `notify` | Notification titles stay single-line and inside the length limit |
| `theme_accent` | `theme` | Any accent color ends up readable on every theme |
| `guide_parse_and_search` | `guide` | Guide parsing and search never panic and anchors stay unique |
| `clock_parse` | `clock` | Time parsing never panics |
| `rules_compile` | `rules` | Any pattern is compiled or refused; an accepted rule never matches empty text and its secret group exists |
| `allow_glob` | `allow` | Any path pattern is translated to a usable matcher or refused |
| `scanner_scan_text` | `scanner` | Scanning any bytes never panics; previews show at most four characters; contexts are short and free of control characters |
| `git_parse` | `git` | Any bytes are read as a loose object, commit, tag, tree or pack index, or refused; names in a tree never contain a separator |
| `git_delta` | `git` | Any delta is applied within the size limit or refused with a named reason |

## Test data

- `testdata/schema.snapshot`: the expected schema. Update with `UPDATE_SNAPSHOT=1 cargo test -p secretsentry-core schema_matches` after adding a migration, and review the difference.
- `testdata/fixtures/overview.sql`: the fixture behind the Overview numbers.

Tests never touch the real user directories: each one gets a temporary home and database.

## Requirements and their tests

| Requirement | Tests |
|---|---|
| The three interfaces offer the same capabilities | `navigation_shortcuts_links_and_capabilities_are_complete`, `every_capability_names_a_real_command_and_every_command_has_help`, `every_dashboard_capability_is_reachable`, `every_terminal_capability_is_reachable`, `the_generated_command_tables_are_current` |
| A change in one interface appears in the others | `changes_made_by_another_connection_are_noticed_once`, `changes_from_another_window_keep_the_selection_and_unsaved_input` |
| Conflicting edits lose nothing | `a_stale_revision_or_a_removed_record_is_a_conflict`, `an_edit_that_lost_a_race_keeps_the_form_and_explains`, `findings_are_filtered_shown_in_detail_and_decided` |
| A second writer waits and is told, never corrupts | `a_second_writer_waits_then_reports_a_busy_database`, `a_failing_migration_is_rolled_back_completely`, `a_batch_is_created_completely_or_not_at_all` |
| Settings are validated, resettable and never dropped | `values_are_stored_listed_and_reset`, `stored_values_from_other_versions_are_kept_and_not_used`, `flag_beats_environment_beats_stored_beats_default`, `invalid_overrides_are_ignored_and_explained` |
| Themes meet the contrast rules | `every_theme_meets_wcag_aa_in_light_and_dark`, `severities_are_ordered_named_and_never_color_only`, `a_weak_custom_accent_is_adjusted_and_reported`, `any_accent_ends_up_readable` |
| The toolkit look is replaced | `every_theme_replaces_the_default_look_and_keeps_text_readable`, `severity_and_status_icons_exist_and_differ`, `every_icon_the_dashboard_draws_comes_from_the_application_set` |
| The window chrome leads somewhere | `the_top_bar_searches_and_its_chip_bell_and_help_lead_somewhere`, `a_collapsed_sidebar_explains_its_icons_on_hover`, `the_chooser_offers_both_front_ends_and_marks_the_default`, `record_table_columns_and_sort_are_saved_and_follow_other_windows` |
| The launcher never asks in scripts or without a display | `the_decision_table_holds`, `a_subcommand_always_runs_and_never_prompts`, `starting_without_a_command_follows_the_launch_decision` |
| Notifications follow the rules and leak nothing | `evaluation_checks_enabled_severity_quiet_hours_then_duplicates`, `quiet_hours_may_cross_midnight`, `publishing_stores_delivers_and_suppresses_duplicates`, `titles_are_cleaned_shortened_and_never_carry_secrets`, `the_system_notification_shows_a_count_and_titles_only_on_request` |
| Text handed to helper programs is data, not code | `delivery_commands_pass_text_as_data_never_as_code`, `terminal_windows_are_tried_in_order_and_arguments_are_quoted` |
| Start at sign-in is per user | `autostart_entries_are_placed_and_written_per_platform`, `install_status_and_uninstall_follow_the_file` |
| Links stay inside the repository | `only_links_below_the_repository_are_accepted`, `only_repository_links_reach_the_browser`, `about_opens_links_only_on_request_and_shows_the_guide_and_diagnostics` |
| Diagnostics report the build and the settings | `diagnostics_report_the_build_the_database_and_changed_settings` |
| Files are private to the user | `a_new_database_is_created_private_migrated_and_configured`, `files_are_written_whole_private_and_never_overwritten_silently`, `files_are_exported_privately_and_imported_back` |
| A foreign file is never overwritten | `files_that_are_not_databases_are_refused_and_left_untouched`, `moving_the_database_copies_or_switches_and_never_overwrites` |
| Demo data is labeled and removable | `demo_data_is_labelled_loaded_once_and_removed_alone` |
| Icons are valid in every format | `the_ico_file_lists_valid_pictures`, `the_icns_file_lists_valid_pictures`, `the_committed_assets_match_the_generator` |
| Hot queries use an index and search text is always a valid query | `hot_queries_use_an_index`, `search_expressions_are_always_valid_fts_queries` |
| Every main screen of the dashboard renders | `every_main_screen_renders_to_a_picture` |
| Release rules | `scripts/tests/release.bats`, `scripts/tests/publish.bats` |

## Not covered by automated tests

The dashboard in a real window on a real display with real input (the window host, the event loop and the graphics backend), a real terminal emulator, display of system notifications by the desktop, start at sign-in at a real sign-in, and starting a packaged download on a clean machine. The check that SQL is not built from text (`sql_stays_in_the_storage_modules_and_is_not_built_from_text`) reads the source line by line for SQL words inside text-building calls; it is not a parser, so review still matters. See [PLATFORM_SUPPORT.md](PLATFORM_SUPPORT.md).
