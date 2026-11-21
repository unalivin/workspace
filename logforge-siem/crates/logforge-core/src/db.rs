use rusqlite::{params, Connection};
use std::path::Path;
use std::time::Duration;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DbError {
    #[error("database error: {0}")]
    Sqlite(#[from] rusqlite::Error),
}

pub struct Migration {
    pub version: u32,
    pub sql: &'static str,
}

const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        sql: include_str!("../../../migrations/0001_initial.sql"),
    },
    Migration {
        version: 2,
        sql: include_str!("../../../migrations/0002_category_colors_and_retention.sql"),
    },
    Migration {
        version: 3,
        sql: include_str!("../../../migrations/0003_malformed_events.sql"),
    },
    Migration {
        version: 4,
        sql: include_str!("../../../migrations/0004_correlation_state.sql"),
    },
    Migration {
        version: 5,
        sql: include_str!("../../../migrations/0005_retention_bytes.sql"),
    },
    Migration {
        version: 6,
        sql: include_str!("../../../migrations/0006_cases.sql"),
    },
];

pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn open(path: &Path) -> Result<Self, DbError> {
        let conn = Connection::open(path)?;
        Self::configure_and_migrate(conn)
    }

    pub fn open_in_memory() -> Result<Self, DbError> {
        let conn = Connection::open_in_memory()?;
        Self::configure_and_migrate(conn)
    }

    fn configure_and_migrate(conn: Connection) -> Result<Self, DbError> {
        conn.busy_timeout(Duration::from_secs(5))?;
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA foreign_keys = ON;",
        )?;

        let current_version: u32 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;

        for migration in MIGRATIONS {
            if migration.version > current_version {
                let tx = conn.unchecked_transaction()?;
                tx.execute_batch(migration.sql)?;
                tx.pragma_update(None, "user_version", migration.version)?;
                tx.commit()?;
            }
        }

        Ok(Self { conn })
    }

    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    pub fn data_version(&self) -> Result<i64, DbError> {
        Ok(self
            .conn
            .query_row("PRAGMA data_version", [], |row| row.get(0))?)
    }

    pub fn get_setting(&self, key: &str) -> Result<Option<String>, DbError> {
        let mut stmt = self
            .conn
            .prepare("SELECT value FROM settings WHERE key = ?1")?;
        let mut rows = stmt.query(params![key])?;
        if let Some(row) = rows.next()? {
            Ok(Some(row.get(0)?))
        } else {
            Ok(None)
        }
    }

    pub fn set_setting(&self, key: &str, value: &str, now_utc: &str) -> Result<(), DbError> {
        self.conn.execute(
            "INSERT INTO settings (key, value, updated_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
            params![key, value, now_utc],
        )?;
        Ok(())
    }
}

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
        assert_eq!(version, 6);

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
