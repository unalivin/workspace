use crate::db::{Database, DbError};
use chrono::{DateTime, Utc};
use rusqlite::{params, OptionalExtension};
use serde::Serialize;

/// Fields that can be used for event-volume groupings. The enum, rather than a raw
/// SQL identifier, keeps dynamic report queries restricted to known columns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventDimension {
    Severity,
    Host,
    Service,
    Source,
}

impl EventDimension {
    fn select_expression(self) -> &'static str {
        match self {
            Self::Severity => "e.severity",
            Self::Host => "e.host",
            Self::Service => "e.service",
            Self::Source => "s.name",
        }
    }

    fn join_source(self) -> &'static str {
        if self == Self::Source {
            " JOIN sources s ON s.id = e.source_id"
        } else {
            ""
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DimensionBucket {
    pub value: String,
    pub event_count: i64,
    pub percentage: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EventVolumeReport {
    pub dimension: EventDimension,
    pub total_events: i64,
    pub buckets: Vec<DimensionBucket>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventReportFilter {
    pub from_utc: Option<String>,
    pub to_utc: Option<String>,
    pub source_id: Option<i64>,
    pub severity: Option<String>,
    pub limit: usize,
}

impl Default for EventReportFilter {
    fn default() -> Self {
        Self {
            from_utc: None,
            to_utc: None,
            source_id: None,
            severity: None,
            limit: 20,
        }
    }
}

impl EventReportFilter {
    fn validated(&self) -> Result<(Option<String>, Option<String>), DbError> {
        if self.limit == 0 || self.limit > 500 {
            return Err(invalid_query());
        }
        if let Some(level) = &self.severity {
            if !["debug", "info", "warn", "error", "critical"]
                .contains(&level.to_ascii_lowercase().as_str())
            {
                return Err(invalid_query());
            }
        }
        let normalize = |value: &Option<String>| -> Result<Option<String>, DbError> {
            value
                .as_deref()
                .map(|text| {
                    DateTime::parse_from_rfc3339(text)
                        .map(|date| date.with_timezone(&Utc).to_rfc3339())
                        .map_err(|_| invalid_query())
                })
                .transpose()
        };
        let from = normalize(&self.from_utc)?;
        let to = normalize(&self.to_utc)?;
        if let (Some(start), Some(end)) = (&from, &to) {
            let start = DateTime::parse_from_rfc3339(start).map_err(|_| invalid_query())?;
            let end = DateTime::parse_from_rfc3339(end).map_err(|_| invalid_query())?;
            if start >= end {
                return Err(invalid_query());
            }
        }
        Ok((from, to))
    }
}

/// Builds a stable top-N report from normalized event fields. Buckets are
/// ordered by count descending, then by value ascending for deterministic output.
pub fn event_volume_report(
    db: &Database,
    dimension: EventDimension,
    filter: &EventReportFilter,
) -> Result<EventVolumeReport, DbError> {
    let (from, to) = filter.validated()?;
    let dimension_sql = dimension.select_expression();
    let source_join = dimension.join_source();
    let mut conditions = Vec::new();
    let mut values = Vec::new();
    if let Some(value) = from {
        values.push(value);
        conditions.push(format!("e.timestamp_utc >= ?{}", values.len()));
    }
    if let Some(value) = to {
        values.push(value);
        conditions.push(format!("e.timestamp_utc < ?{}", values.len()));
    }
    if let Some(source_id) = filter.source_id {
        values.push(source_id);
        conditions.push(format!("e.source_id = ?{}", values.len()));
    }
    if let Some(severity) = &filter.severity {
        values.push(severity.to_ascii_lowercase());
        conditions.push(format!("lower(e.severity) = ?{}", values.len()));
    }
    let where_sql = if conditions.is_empty() {
        String::new()
    } else {
        format!(" WHERE {}", conditions.join(" AND "))
    };
    let total_sql = format!("SELECT COUNT(*) FROM events e{source_join}{where_sql}");
    let total_events: i64 = db
        .conn()
        .query_row(&total_sql, rusqlite::params_from_iter(values.iter()), |row| row.get(0))?;

    let limit_index = values.len() + 1;
    let buckets_sql = format!(
        "SELECT {dimension_sql}, COUNT(*) AS event_count FROM events e{source_join}{where_sql} \
         GROUP BY {dimension_sql} ORDER BY event_count DESC, {dimension_sql} COLLATE NOCASE LIMIT ?{limit_index}"
    );
    let mut bucket_values = values.clone();
    bucket_values.push(filter.limit as i64);
    let mut statement = db.conn().prepare(&buckets_sql)?;
    let rows = statement.query_map(rusqlite::params_from_iter(bucket_values.iter()), |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
    })?;
    let counts = rows.collect::<Result<Vec<_>, _>>()?;
    let buckets = counts
        .into_iter()
        .map(|(value, event_count)| DimensionBucket {
            value,
            event_count,
            percentage: if total_events == 0 {
                0.0
            } else {
                event_count as f64 * 100.0 / total_events as f64
            },
        })
        .collect();
    Ok(EventVolumeReport {
        dimension,
        total_events,
        buckets,
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RuleAlertCount {
    pub rule_id: i64,
    pub rule_name: String,
    pub total_alerts: i64,
    pub open_alerts: i64,
    pub latest_alert_at: Option<String>,
}

/// Groups alerts by their originating detection rule, retaining rules with no
/// alert history so their zero-activity state is visible in operational reports.
pub fn alert_counts_by_rule(db: &Database) -> Result<Vec<RuleAlertCount>, DbError> {
    let mut statement = db.conn().prepare(
        "SELECT r.id, r.name, COUNT(a.id),\
            SUM(CASE WHEN a.status = 'open' THEN 1 ELSE 0 END), MAX(a.created_at)\
         FROM detection_rules r LEFT JOIN alerts a ON a.rule_id = r.id\
         GROUP BY r.id, r.name ORDER BY COUNT(a.id) DESC, r.name COLLATE NOCASE",
    )?;
    let rows = statement.query_map([], |row| {
        Ok(RuleAlertCount {
            rule_id: row.get(0)?,
            rule_name: row.get(1)?,
            total_alerts: row.get(2)?,
            open_alerts: row.get::<_, Option<i64>>(3)?.unwrap_or(0),
            latest_alert_at: row.get(4)?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
}

/// Finds the timestamp of the latest received event, which can be used by
/// dashboards to distinguish an idle source from an active one.
pub fn latest_received_event(db: &Database) -> Result<Option<String>, DbError> {
    db.conn()
        .query_row("SELECT MAX(received_at) FROM events", [], |row| row.get(0))
        .optional()
        .map_err(DbError::from)
        .map(Option::flatten)
}

fn invalid_query() -> DbError {
    DbError::Sqlite(rusqlite::Error::InvalidQuery)
}
