use crate::db::{Database, DbError};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SourceCount {
    pub name: String,
    pub kind: String,
    pub event_count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SeverityCount {
    pub severity: String,
    pub event_count: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct IngestionStats {
    pub received: i64,
    pub deduplicated: i64,
    pub dropped: i64,
    pub malformed: i64,
    pub accepted: i64,
    pub duplicate_rate_percent: f64,
    pub malformed_rate_percent: f64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SourceHealth {
    pub source_id: i64,
    pub name: String,
    pub kind: String,
    pub enabled: bool,
    pub total_events: i64,
    pub last_event_at: Option<String>,
    pub last_received_at: Option<String>,
    pub critical_events: i64,
    pub error_events: i64,
}

pub fn top_sources(db: &Database) -> Result<Vec<SourceCount>, DbError> {
    let mut stmt = db
        .conn()
        .prepare("SELECT name, kind, event_count FROM v_top_sources")?;
    let rows = stmt.query_map([], |r| {
        Ok(SourceCount {
            name: r.get(0)?,
            kind: r.get(1)?,
            event_count: r.get(2)?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
}

/// Returns event volume by normalized severity, including zero-volume levels.
pub fn events_by_severity(db: &Database) -> Result<Vec<SeverityCount>, DbError> {
    let mut statement = db.conn().prepare(
        "SELECT severity, COUNT(e.id) FROM ( \
            SELECT 'debug' AS severity, 0 AS position UNION ALL \
            SELECT 'info', 1 UNION ALL SELECT 'warn', 2 UNION ALL \
            SELECT 'error', 3 UNION ALL SELECT 'critical', 4 \
        ) levels LEFT JOIN events e ON e.severity = levels.severity \
        GROUP BY levels.severity, levels.position ORDER BY levels.position",
    )?;
    let rows = statement.query_map([], |row| {
        Ok(SeverityCount {
            severity: row.get(0)?,
            event_count: row.get(1)?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
}

/// Provides a snapshot of source activity without exposing raw event content.
pub fn source_health(db: &Database) -> Result<Vec<SourceHealth>, DbError> {
    let mut statement = db.conn().prepare(
        "SELECT s.id, s.name, s.kind, s.enabled, COUNT(e.id), \
            MAX(e.timestamp_utc), MAX(e.received_at), \
            SUM(CASE WHEN e.severity = 'critical' THEN 1 ELSE 0 END), \
            SUM(CASE WHEN e.severity = 'error' THEN 1 ELSE 0 END) \
         FROM sources s LEFT JOIN events e ON e.source_id = s.id \
         GROUP BY s.id ORDER BY s.name COLLATE NOCASE, s.id",
    )?;
    let rows = statement.query_map([], |row| {
        Ok(SourceHealth {
            source_id: row.get(0)?,
            name: row.get(1)?,
            kind: row.get(2)?,
            enabled: row.get::<_, i64>(3)? != 0,
            total_events: row.get(4)?,
            last_event_at: row.get(5)?,
            last_received_at: row.get(6)?,
            critical_events: row.get::<_, Option<i64>>(7)?.unwrap_or(0),
            error_events: row.get::<_, Option<i64>>(8)?.unwrap_or(0),
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
}

/// Computes ingestion ratios defensively for empty or partially initialized databases.
pub fn ingestion_stats(db: &Database) -> Result<IngestionStats, DbError> {
    let (received, deduplicated, dropped, malformed): (i64, i64, i64, i64) = db.conn().query_row(
        "SELECT received, deduplicated, dropped, malformed FROM ingestion_stats WHERE id = 1",
        [],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
    )?;
    let denominator = received.max(0);
    let rate = |part: i64| {
        if denominator == 0 {
            0.0
        } else {
            part.max(0) as f64 * 100.0 / denominator as f64
        }
    };
    Ok(IngestionStats {
        received,
        deduplicated,
        dropped,
        malformed,
        accepted: (received - deduplicated - dropped - malformed).max(0),
        duplicate_rate_percent: rate(deduplicated),
        malformed_rate_percent: rate(malformed),
    })
}

/// Counts events received during a half-open RFC3339 time range [start, end).
pub fn count_events_in_range(
    db: &Database,
    start_utc: &str,
    end_utc: &str,
) -> Result<i64, DbError> {
    let start = chrono::DateTime::parse_from_rfc3339(start_utc)
        .map_err(|_| DbError::Sqlite(rusqlite::Error::InvalidQuery))?;
    let end = chrono::DateTime::parse_from_rfc3339(end_utc)
        .map_err(|_| DbError::Sqlite(rusqlite::Error::InvalidQuery))?;
    if start >= end {
        return Err(DbError::Sqlite(rusqlite::Error::InvalidQuery));
    }
    db.conn()
        .query_row(
            "SELECT COUNT(*) FROM events WHERE timestamp_utc >= ?1 AND timestamp_utc < ?2",
            rusqlite::params![
                start.with_timezone(&chrono::Utc).to_rfc3339(),
                end.with_timezone(&chrono::Utc).to_rfc3339()
            ],
            |row| row.get(0),
        )
        .map_err(DbError::from)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AlertStatusCount {
    pub status: String,
    pub alert_count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AlertSeverityCount {
    pub severity: String,
    pub open_count: i64,
    pub acknowledged_count: i64,
    pub resolved_count: i64,
}

/// Returns all alert workflow states, including states with no matching alerts.
pub fn alert_status_counts(db: &Database) -> Result<Vec<AlertStatusCount>, DbError> {
    let mut statement = db.conn().prepare(
        "SELECT states.status, COUNT(a.id) FROM ( \
            SELECT 'open' AS status, 0 AS position UNION ALL \
            SELECT 'acknowledged', 1 UNION ALL SELECT 'resolved', 2 \
        ) states LEFT JOIN alerts a ON a.status = states.status \
        GROUP BY states.status, states.position ORDER BY states.position",
    )?;
    let rows = statement.query_map([], |row| {
        Ok(AlertStatusCount {
            status: row.get(0)?,
            alert_count: row.get(1)?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
}

/// Produces a severity-by-workflow matrix in stable severity order.
pub fn alert_severity_matrix(db: &Database) -> Result<Vec<AlertSeverityCount>, DbError> {
    let mut statement = db.conn().prepare(
        "SELECT levels.severity, \
            SUM(CASE WHEN a.status = 'open' THEN 1 ELSE 0 END), \
            SUM(CASE WHEN a.status = 'acknowledged' THEN 1 ELSE 0 END), \
            SUM(CASE WHEN a.status = 'resolved' THEN 1 ELSE 0 END) \
         FROM (SELECT 'debug' AS severity, 0 AS position UNION ALL \
            SELECT 'info', 1 UNION ALL SELECT 'warn', 2 UNION ALL \
            SELECT 'error', 3 UNION ALL SELECT 'critical', 4) levels \
         LEFT JOIN alerts a ON a.severity = levels.severity \
         GROUP BY levels.severity, levels.position ORDER BY levels.position",
    )?;
    let rows = statement.query_map([], |row| {
        Ok(AlertSeverityCount {
            severity: row.get(0)?,
            open_count: row.get::<_, Option<i64>>(1)?.unwrap_or(0),
            acknowledged_count: row.get::<_, Option<i64>>(2)?.unwrap_or(0),
            resolved_count: row.get::<_, Option<i64>>(3)?.unwrap_or(0),
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
}

/// Counts alerts opened in the half-open UTC interval [start, end).
pub fn count_alerts_in_range(
    db: &Database,
    start_utc: &str,
    end_utc: &str,
) -> Result<i64, DbError> {
    let start = chrono::DateTime::parse_from_rfc3339(start_utc)
        .map_err(|_| DbError::Sqlite(rusqlite::Error::InvalidQuery))?;
    let end = chrono::DateTime::parse_from_rfc3339(end_utc)
        .map_err(|_| DbError::Sqlite(rusqlite::Error::InvalidQuery))?;
    if start >= end {
        return Err(DbError::Sqlite(rusqlite::Error::InvalidQuery));
    }
    db.conn()
        .query_row(
            "SELECT COUNT(*) FROM alerts WHERE created_at >= ?1 AND created_at < ?2",
            rusqlite::params![
                start.with_timezone(&chrono::Utc).to_rfc3339(),
                end.with_timezone(&chrono::Utc).to_rfc3339()
            ],
            |row| row.get(0),
        )
        .map_err(DbError::from)
}
