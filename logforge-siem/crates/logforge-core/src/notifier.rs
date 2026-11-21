use crate::db::{Database, DbError};
use crate::model::Severity;
use chrono::{Timelike, Utc};
use rusqlite::{params, OptionalExtension};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotificationRule {
    pub id: i64,
    pub event_type: String,
    pub min_severity: String,
    pub enabled: bool,
    pub quiet_start_hour: u8,
    pub quiet_end_hour: u8,
}

pub fn is_quiet_hour(hour: u8, start: u8, end: u8) -> bool {
    if start == end {
        return false;
    }
    if start < end {
        hour >= start && hour < end
    } else {
        hour >= start || hour < end
    }
}

pub fn severity_rank(s: &str) -> u8 {
    match Severity::parse(s) {
        Some(Severity::Debug) => 1,
        Some(Severity::Info) => 2,
        Some(Severity::Warn) => 3,
        Some(Severity::Error) => 4,
        Some(Severity::Critical) => 5,
        None => 0,
    }
}

pub fn should_notify(rule: &NotificationRule, severity: &str, current_hour_utc: u8) -> bool {
    if !rule.enabled {
        return false;
    }
    if is_quiet_hour(current_hour_utc, rule.quiet_start_hour, rule.quiet_end_hour) {
        return false;
    }
    severity_rank(severity) >= severity_rank(&rule.min_severity)
}

pub fn upsert_notification_rule(
    db: &Database,
    event_type: &str,
    min_severity: &str,
    enabled: bool,
    quiet_start_hour: u8,
    quiet_end_hour: u8,
) -> Result<(), DbError> {
    db.conn().execute(
        "INSERT INTO notification_rules (event_type, min_severity, enabled, quiet_start_hour, quiet_end_hour)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(event_type) DO UPDATE SET
           min_severity = excluded.min_severity,
           enabled = excluded.enabled,
           quiet_start_hour = excluded.quiet_start_hour,
           quiet_end_hour = excluded.quiet_end_hour",
        params![
            event_type,
            min_severity,
            if enabled { 1 } else { 0 },
            quiet_start_hour as i64,
            quiet_end_hour as i64
        ],
    )?;
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingNotification {
    pub id: i64,
    pub alert_id: i64,
    pub event_type: String,
    pub severity: String,
    pub payload_json: String,
    pub attempt_count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct OutboxCounts {
    pub queued: i64,
    pub claimed: i64,
    pub delivered: i64,
    pub dead: i64,
}

pub fn enqueue_alert_notification(
    db: &Database,
    alert_id: i64,
    event_type: &str,
    severity: &str,
    payload: &serde_json::Value,
    now_utc: &str,
) -> Result<bool, DbError> {
    let hour = chrono::DateTime::parse_from_rfc3339(now_utc)
        .map_err(|_| DbError::Sqlite(rusqlite::Error::InvalidQuery))?
        .with_timezone(&Utc)
        .hour() as u8;
    let mut statement = db.conn().prepare(
        "SELECT id, event_type, min_severity, enabled, quiet_start_hour, quiet_end_hour FROM notification_rules WHERE event_type IN (?1, '*') ORDER BY id",
    )?;
    let rules = statement
        .query_map(params![event_type], |row| {
            Ok(NotificationRule {
                id: row.get(0)?,
                event_type: row.get(1)?,
                min_severity: row.get(2)?,
                enabled: row.get::<_, i64>(3)? != 0,
                quiet_start_hour: row.get::<_, i64>(4)? as u8,
                quiet_end_hour: row.get::<_, i64>(5)? as u8,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    if !rules.iter().any(|rule| should_notify(rule, severity, hour)) {
        return Ok(false);
    }
    let payload = serde_json::to_string(payload)
        .map_err(|_| DbError::Sqlite(rusqlite::Error::InvalidQuery))?;
    Ok(db.conn().execute(
        "INSERT INTO notification_outbox (alert_id, event_type, severity, payload_json, state, next_attempt_at, created_at)
         VALUES (?1, ?2, ?3, ?4, 'queued', ?5, ?5) ON CONFLICT(alert_id, event_type) DO NOTHING",
        params![alert_id, event_type, severity, payload, now_utc],
    )? != 0)
}

pub fn claim_due_notifications(
    db: &Database,
    now_utc: &str,
    limit: usize,
) -> Result<Vec<PendingNotification>, DbError> {
    if chrono::DateTime::parse_from_rfc3339(now_utc).is_err() || limit == 0 {
        return Err(DbError::Sqlite(rusqlite::Error::InvalidQuery));
    }
    let tx = db.conn().unchecked_transaction()?;
    let mut statement = tx.prepare(
        "SELECT id, alert_id, event_type, severity, payload_json, attempt_count FROM notification_outbox
         WHERE state = 'queued' AND next_attempt_at <= ?1 ORDER BY id LIMIT ?2",
    )?;
    let rows = statement
        .query_map(params![now_utc, limit.min(100) as i64], |row| {
            Ok(PendingNotification {
                id: row.get(0)?,
                alert_id: row.get(1)?,
                event_type: row.get(2)?,
                severity: row.get(3)?,
                payload_json: row.get(4)?,
                attempt_count: row.get(5)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    drop(statement);
    for notification in &rows {
        tx.execute(
            "UPDATE notification_outbox SET state = 'claimed', attempt_count = attempt_count + 1, next_attempt_at = ?2 WHERE id = ?1 AND state = 'queued'",
            params![notification.id, now_utc],
        )?;
    }
    tx.commit()?;
    Ok(rows)
}

pub fn finish_notification(
    db: &Database,
    id: i64,
    now_utc: &str,
    delivery_error: Option<&str>,
) -> Result<bool, DbError> {
    let now = chrono::DateTime::parse_from_rfc3339(now_utc)
        .map_err(|_| DbError::Sqlite(rusqlite::Error::InvalidQuery))?
        .with_timezone(&Utc);
    let attempts: Option<i64> = db
        .conn()
        .query_row(
            "SELECT attempt_count FROM notification_outbox WHERE id = ?1 AND state = 'claimed'",
            params![id],
            |row| row.get(0),
        )
        .optional()?;
    let Some(attempts) = attempts else {
        return Ok(false);
    };
    match delivery_error {
        None => Ok(db.conn().execute(
            "UPDATE notification_outbox SET state = 'delivered', delivered_at = ?1, last_error = '' WHERE id = ?2 AND state = 'claimed'",
            params![now.to_rfc3339(), id],
        )? != 0),
        Some(error) => {
            let safe_error = crate::ingest::redact_secrets(error).0;
            if attempts >= 8 {
                Ok(db.conn().execute(
                    "UPDATE notification_outbox SET state = 'dead', last_error = ?1 WHERE id = ?2 AND state = 'claimed'",
                    params![safe_error, id],
                )? != 0)
            } else {
                let delay = 30i64.saturating_mul(1i64 << (attempts.saturating_sub(1).clamp(0, 6) as u32)).min(3600);
                let next = (now + chrono::Duration::seconds(delay)).to_rfc3339();
                Ok(db.conn().execute(
                    "UPDATE notification_outbox SET state = 'queued', next_attempt_at = ?1, last_error = ?2 WHERE id = ?3 AND state = 'claimed'",
                    params![next, safe_error, id],
                )? != 0)
            }
        }
    }
}

pub fn outbox_counts(db: &Database) -> Result<OutboxCounts, DbError> {
    db.conn().query_row(
        "SELECT SUM(state = 'queued'), SUM(state = 'claimed'), SUM(state = 'delivered'), SUM(state = 'dead') FROM notification_outbox",
        [],
        |row| Ok(OutboxCounts {
            queued: row.get::<_, Option<i64>>(0)?.unwrap_or(0),
            claimed: row.get::<_, Option<i64>>(1)?.unwrap_or(0),
            delivered: row.get::<_, Option<i64>>(2)?.unwrap_or(0),
            dead: row.get::<_, Option<i64>>(3)?.unwrap_or(0),
        }),
    ).map_err(DbError::from)
}

/// Recovers claims abandoned by a crashed worker. A row is eligible once its
/// recorded claim time is older than `stale_before_utc`; exhausted messages
/// become dead letters rather than being retried forever.
pub fn recover_stale_claims(
    db: &Database,
    stale_before_utc: &str,
    now_utc: &str,
    limit: usize,
) -> Result<usize, DbError> {
    if limit == 0
        || chrono::DateTime::parse_from_rfc3339(stale_before_utc).is_err()
        || chrono::DateTime::parse_from_rfc3339(now_utc).is_err()
    {
        return Err(DbError::Sqlite(rusqlite::Error::InvalidQuery));
    }
    let tx = db.conn().unchecked_transaction()?;
    let affected = tx.execute(
        "UPDATE notification_outbox \
         SET state = CASE WHEN attempt_count >= 8 THEN 'dead' ELSE 'queued' END, \
             next_attempt_at = ?1, last_error = 'claim expired' \
         WHERE state = 'claimed' AND next_attempt_at <= ?2 \
           AND id IN (SELECT id FROM notification_outbox WHERE state = 'claimed' AND next_attempt_at <= ?2 ORDER BY id LIMIT ?3)",
        params![now_utc, stale_before_utc, limit.min(1000) as i64],
    )?;
    tx.commit()?;
    Ok(affected)
}

/// Manually requeues selected dead-letter notifications after operator review.
/// Attempts are reset only for explicitly selected ids, and payloads are kept.
pub fn retry_dead_notifications(
    db: &Database,
    ids: &[i64],
    now_utc: &str,
) -> Result<usize, DbError> {
    if chrono::DateTime::parse_from_rfc3339(now_utc).is_err()
        || ids.iter().any(|id| *id <= 0)
    {
        return Err(DbError::Sqlite(rusqlite::Error::InvalidQuery));
    }
    if ids.is_empty() {
        return Ok(0);
    }
    let mut conditions = Vec::with_capacity(ids.len());
    let mut values = Vec::with_capacity(ids.len() + 1);
    values.push(rusqlite::types::Value::Text(now_utc.to_string()));
    for id in ids {
        values.push(rusqlite::types::Value::Integer(*id));
        conditions.push(format!("?{}", values.len()));
    }
    let sql = format!(
        "UPDATE notification_outbox SET state = 'queued', attempt_count = 0, \
         next_attempt_at = ?1, last_error = '' WHERE state = 'dead' AND id IN ({})",
        conditions.join(", ")
    );
    db.conn()
        .execute(&sql, rusqlite::params_from_iter(values.iter()))
        .map_err(DbError::from)
}
