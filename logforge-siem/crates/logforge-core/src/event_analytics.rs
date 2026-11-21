use crate::db::{Database, DbError};
use chrono::{DateTime, Duration, SecondsFormat, Utc};
use rusqlite::params;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimeBucket {
    pub start_utc: String,
    pub event_count: i64,
    pub alert_count: i64,
    pub critical_count: i64,
    pub error_count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValueCount {
    pub value: String,
    pub event_count: i64,
    pub alert_count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeverityCount {
    pub severity: String,
    pub event_count: i64,
    pub alert_count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IngestionLatency {
    pub sample_count: i64,
    pub average_millis: i64,
    pub p95_millis: i64,
    pub max_millis: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnalyticsSnapshot {
    pub total_events: i64,
    pub total_alerts: i64,
    pub oldest_event: Option<String>,
    pub newest_event: Option<String>,
    pub top_hosts: Vec<ValueCount>,
    pub top_services: Vec<ValueCount>,
    pub severity_counts: Vec<SeverityCount>,
    pub ingestion_latency: IngestionLatency,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BucketSize {
    Minute,
    FifteenMinutes,
    Hour,
    Day,
}

impl BucketSize {
    pub fn seconds(self) -> i64 {
        match self {
            Self::Minute => 60,
            Self::FifteenMinutes => 15 * 60,
            Self::Hour => 60 * 60,
            Self::Day => 24 * 60 * 60,
        }
    }

    pub fn sql_format(self) -> &'static str {
        match self {
            Self::Minute => "%Y-%m-%dT%H:%M:00Z",
            Self::FifteenMinutes => "%Y-%m-%dT%H:",
            Self::Hour => "%Y-%m-%dT%H:00:00Z",
            Self::Day => "%Y-%m-%dT00:00:00Z",
        }
    }
}

pub fn event_rate_series(
    db: &Database,
    start_utc: &str,
    end_utc: &str,
    bucket_size: BucketSize,
) -> Result<Vec<TimeBucket>, DbError> {
    let start = parse_bound(start_utc)?;
    let end = parse_bound(end_utc)?;
    if start >= end || end.signed_duration_since(start).num_seconds() > 366 * 24 * 3600 {
        return Err(DbError::Sqlite(rusqlite::Error::InvalidQuery));
    }
    let step = bucket_size.seconds();
    let mut cursor_seconds = start.timestamp().div_euclid(step) * step;
    let end_seconds = end.timestamp();
    let mut buckets = Vec::new();
    while cursor_seconds < end_seconds {
        let bucket_start = DateTime::from_timestamp(cursor_seconds, 0)
            .ok_or(DbError::Sqlite(rusqlite::Error::InvalidQuery))?;
        let next = bucket_start + Duration::seconds(step);
        let start_text = bucket_start.to_rfc3339_opts(SecondsFormat::Secs, true);
        let next_text = next.to_rfc3339_opts(SecondsFormat::Secs, true);
        let (events, alerts, critical, errors): (i64, i64, i64, i64) = db.conn().query_row(
            "SELECT COUNT(e.id), COUNT(a.id), SUM(CASE WHEN e.severity = 'critical' THEN 1 ELSE 0 END), SUM(CASE WHEN e.severity = 'error' THEN 1 ELSE 0 END)
             FROM events e LEFT JOIN alerts a ON a.event_id = e.id
             WHERE e.timestamp_utc >= ?1 AND e.timestamp_utc < ?2",
            params![start_text, next_text],
            |row| Ok((row.get(0)?, row.get(1)?, row.get::<_, Option<i64>>(2)?.unwrap_or(0), row.get::<_, Option<i64>>(3)?.unwrap_or(0))),
        )?;
        buckets.push(TimeBucket {
            start_utc: start_text,
            event_count: events,
            alert_count: alerts,
            critical_count: critical,
            error_count: errors,
        });
        cursor_seconds = cursor_seconds.saturating_add(step);
    }
    Ok(buckets)
}

pub fn analytics_snapshot(db: &Database, limit: usize) -> Result<AnalyticsSnapshot, DbError> {
    let (total_events, total_alerts, oldest_event, newest_event): (i64, i64, Option<String>, Option<String>) = db.conn().query_row(
        "SELECT (SELECT COUNT(*) FROM events), (SELECT COUNT(*) FROM alerts), MIN(timestamp_utc), MAX(timestamp_utc) FROM events",
        [],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
    )?;
    let top_hosts = top_values(db, "host", limit)?;
    let top_services = top_values(db, "service", limit)?;
    let severity_counts = severity_distribution(db)?;
    let ingestion_latency = latency(db)?;
    Ok(AnalyticsSnapshot {
        total_events,
        total_alerts,
        oldest_event,
        newest_event,
        top_hosts,
        top_services,
        severity_counts,
        ingestion_latency,
    })
}

fn parse_bound(input: &str) -> Result<DateTime<Utc>, DbError> {
    DateTime::parse_from_rfc3339(input)
        .map(|date| date.with_timezone(&Utc))
        .map_err(|_| DbError::Sqlite(rusqlite::Error::InvalidQuery))
}

fn top_values(db: &Database, column: &str, limit: usize) -> Result<Vec<ValueCount>, DbError> {
    let (query, alert_join) = match column {
        "host" => ("SELECT e.host, COUNT(DISTINCT e.id), COUNT(DISTINCT a.id) FROM events e LEFT JOIN alerts a ON a.event_id = e.id GROUP BY e.host ORDER BY COUNT(DISTINCT e.id) DESC, e.host ASC LIMIT ?1", true),
        "service" => ("SELECT e.service, COUNT(DISTINCT e.id), COUNT(DISTINCT a.id) FROM events e LEFT JOIN alerts a ON a.event_id = e.id GROUP BY e.service ORDER BY COUNT(DISTINCT e.id) DESC, e.service ASC LIMIT ?1", true),
        _ => return Err(DbError::Sqlite(rusqlite::Error::InvalidQuery)),
    };
    let _ = alert_join;
    let mut statement = db.conn().prepare(query)?;
    let rows = statement.query_map(params![limit.clamp(1, 100) as i64], |row| {
        Ok(ValueCount {
            value: row.get(0)?,
            event_count: row.get(1)?,
            alert_count: row.get(2)?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
}

fn severity_distribution(db: &Database) -> Result<Vec<SeverityCount>, DbError> {
    let mut statement = db.conn().prepare(
        "SELECT levels.severity, COALESCE(e.event_count, 0), COALESCE(a.alert_count, 0)
         FROM (SELECT 'debug' severity UNION ALL SELECT 'info' UNION ALL SELECT 'warn' UNION ALL SELECT 'error' UNION ALL SELECT 'critical') levels
         LEFT JOIN (SELECT severity, COUNT(*) event_count FROM events GROUP BY severity) e ON e.severity = levels.severity
         LEFT JOIN (SELECT severity, COUNT(*) alert_count FROM alerts WHERE status != 'resolved' GROUP BY severity) a ON a.severity = levels.severity
         ORDER BY CASE levels.severity WHEN 'critical' THEN 0 WHEN 'error' THEN 1 WHEN 'warn' THEN 2 WHEN 'info' THEN 3 ELSE 4 END",
    )?;
    let rows = statement.query_map([], |row| {
        Ok(SeverityCount {
            severity: row.get(0)?,
            event_count: row.get(1)?,
            alert_count: row.get(2)?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
}

fn latency(db: &Database) -> Result<IngestionLatency, DbError> {
    let mut statement = db.conn().prepare(
        "SELECT CAST((julianday(received_at) - julianday(timestamp_utc)) * 86400000 AS INTEGER) AS delay
         FROM events WHERE received_at >= timestamp_utc ORDER BY delay",
    )?;
    let values = statement
        .query_map([], |row| row.get::<_, i64>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    if values.is_empty() {
        return Ok(IngestionLatency {
            sample_count: 0,
            average_millis: 0,
            p95_millis: 0,
            max_millis: 0,
        });
    }
    let sum = values
        .iter()
        .fold(0i128, |sum, value| sum + i128::from(*value));
    let p95_index = ((values.len() - 1) * 95).div_ceil(100);
    Ok(IngestionLatency {
        sample_count: values.len() as i64,
        average_millis: (sum / values.len() as i128) as i64,
        p95_millis: values[p95_index],
        max_millis: *values.last().expect("non-empty samples"),
    })
}
