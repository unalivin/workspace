#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ingest_deduplicates_redacts_and_triggers_alerts() {
        let db = Database::open_in_memory().expect("open db");
        let svc = SiemService::new(db);
        let src_id = svc
            .add_source("syslog", "file", "/var/log/auth.log")
            .expect("add source");
        svc.add_rule("SSH Brute Force", "warn", "Failed password")
            .expect("add rule");

        let lines = [
            "2026-11-21T09:00:00Z web-01 sshd: Failed password for root",
            "2026-11-21T09:00:00Z web-01 sshd: Failed password for root",
            "bad line password=malformed-secret",
        ];
        let summary = svc.ingest_lines(src_id, &lines).expect("ingest");
        assert_eq!(summary.received, 1);
        assert_eq!(summary.deduplicated, 1);
        assert_eq!(summary.malformed, 1);
        assert_eq!(summary.alerts_created, 1);
        let (malformed_count, malformed_raw): (i64, String) = svc
            .db()
            .conn()
            .query_row(
                "SELECT COUNT(*), raw_line FROM malformed_events WHERE source_id = ?1",
                params![src_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("malformed event persisted");
        assert_eq!(malformed_count, 1);
        assert!(!malformed_raw.contains("malformed-secret"));

        let hits = svc.search_events("Failed", 10).expect("search");
        assert_eq!(hits.len(), 1);

        let alerts = svc.list_alerts(Some("open")).expect("alerts");
        assert_eq!(alerts.len(), 1);
        svc.update_alert_status(alerts[0].id, "resolved")
            .expect("resolve");
        assert_eq!(svc.list_alerts(Some("open")).expect("open").len(), 0);
    }

    #[test]
    fn intake_policies_drop_newest_or_block_reader_at_capacity() {
        let db = Database::open_in_memory().expect("open db");
        let service = SiemService::new(db);
        let source = service
            .add_source("queue", "file", "memory")
            .expect("source");
        let lines = vec!["2026-11-21T09:00:00Z host app: same event"; MAX_BATCH_SIZE + 1];
        let dropped = service
            .ingest_with_policy(source, &lines, crate::intake_queue::FullPolicy::DropNewest)
            .expect("drop policy");
        assert_eq!(dropped.dropped, 1);
        assert_eq!(dropped.received + dropped.deduplicated, MAX_BATCH_SIZE);
        let blocked = service
            .ingest_with_policy(source, &lines, crate::intake_queue::FullPolicy::BlockReader)
            .expect("block policy");
        assert_eq!(blocked.dropped, 0);
        assert_eq!(blocked.deduplicated, MAX_BATCH_SIZE + 1);
    }

    #[test]
    fn demo_seed_is_idempotent() {
        let db = Database::open_in_memory().expect("open db");
        let svc = SiemService::new(db);
        svc.seed_demo_data().expect("seed 1");
        svc.seed_demo_data().expect("seed 2");
        let counts = svc.overview_counts().expect("counts");
        assert_eq!(counts.total_events, 3);
        assert_eq!(counts.open_alerts, 2);
    }
}
