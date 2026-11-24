#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrations_apply_and_pass_integrity_checks() {
        let db = Database::open_in_memory().expect("open in-memory db");
        let version: u32 = db
            .conn()
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .expect("user_version");
        assert_eq!(version, 8);

        let integrity: String = db
            .conn()
            .query_row("PRAGMA integrity_check", [], |r| r.get(0))
            .expect("integrity_check");
        assert_eq!(integrity, "ok");

        let fk_violations: usize = db
            .conn()
            .prepare("PRAGMA foreign_key_check")
            .expect("prepare fk")
            .query_map([], |_| Ok(()))
            .expect("query fk")
            .count();
        assert_eq!(fk_violations, 0);
    }

    #[test]
    fn hot_queries_use_indexes() {
        let db = Database::open_in_memory().expect("open db");
        let plan_ts: String = db
            .conn()
            .query_row(
                "EXPLAIN QUERY PLAN SELECT id FROM events WHERE timestamp_utc >= '2026-01-01T00:00:00Z'",
                [],
                |r| r.get(3),
            )
            .expect("explain timestamp");
        assert!(
            plan_ts.contains("idx_events_timestamp"),
            "expected idx_events_timestamp, got: {plan_ts}"
        );

        let plan_alert: String = db
            .conn()
            .query_row(
                "EXPLAIN QUERY PLAN SELECT id FROM alerts WHERE status = 'open' AND severity = 'critical'",
                [],
                |r| r.get(3),
            )
            .expect("explain alerts");
        assert!(
            plan_alert.contains("idx_alerts_status_severity"),
            "expected idx_alerts_status_severity, got: {plan_alert}"
        );
    }

    #[test]
    fn overview_views_match_fixture() {
        let db = Database::open_in_memory().expect("open db");
        db.conn()
            .execute(
                "INSERT INTO sources (name, kind, location, enabled, created_at)
                 VALUES ('auth-log', 'synthetic', 'demo', 1, '2026-11-21T00:00:00Z')",
                [],
            )
            .expect("insert source");
        db.conn()
            .execute(
                "INSERT INTO events (source_id, timestamp_utc, received_at, severity, host, service, message, raw_line, dedup_hash, redacted)
                 VALUES (1, '2026-11-21T00:00:01Z', '2026-11-21T00:00:01Z', 'warn', 'web-01', 'sshd', 'Failed password', 'raw', 'h1', 0)",
                [],
            )
            .expect("insert event");

        let (total_events, active_sources, open_alerts): (i64, i64, i64) = db
            .conn()
            .query_row(
                "SELECT total_events, active_sources, open_alerts FROM v_overview_counts",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .expect("query v_overview_counts");
        assert_eq!(total_events, 1);
        assert_eq!(active_sources, 1);
        assert_eq!(open_alerts, 0);
    }
}
