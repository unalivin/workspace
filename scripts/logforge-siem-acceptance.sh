#!/usr/bin/env bash
set -euo pipefail

project_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/../logforge-siem" && pwd)"
cd "$project_dir"

run_test() {
  local target="$1"
  local test_name="$2"
  cargo test -p logforge-core --test "$target" -- --exact "$test_name"
}

run_test source_lifecycle_tests disabling_source_blocks_new_ingest_without_removing_existing_data
run_test event_query_tests filtered_search_supports_parameterized_facets_and_keyset_pages
run_test correlation_service_tests ingested_events_drive_persistent_threshold_correlation_alerts
run_test alert_bulk_workflow_tests bulk_update_only_changes_alerts_matching_every_selector
run_test case_management_tests manages_case_lifecycle_notes_assignment_and_alert_links_atomically
run_test retention_plan_tests age_policy_prunes_old_events_even_when_event_count_is_below_limit
run_test audit_query_tests audit_queries_filter_page_and_preserve_chain_integrity
run_test notification_outbox_tests generated_alerts_enqueue_and_retry_notifications_with_backoff
