use crate::db::{Database, DbError};
use rusqlite::params;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetentionPolicy {
    pub name: String,
    pub max_days: i64,
    pub max_events: i64,
    pub max_bytes: i64,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetentionPlan {
    pub event_ids: Vec<i64>,
    pub count: usize,
    pub bytes_to_free: i64,
}

pub fn get_default_policy(db: &Database) -> Result<RetentionPolicy, DbError> {
    db.conn().query_row(
        "SELECT name, max_days, max_events, max_bytes, enabled FROM retention_policies WHERE id = 1",
        [],
        |row| Ok(RetentionPolicy {
            name: row.get(0)?,
            max_days: row.get(1)?,
            max_events: row.get(2)?,
            max_bytes: row.get(3)?,
            enabled: row.get::<_, i64>(4)? != 0,
        }),
    ).map_err(DbError::from)
}

pub fn plan_retention(
    db: &Database,
    max_days: i64,
    max_events: i64,
    max_bytes: i64,
    now_utc: &str,
) -> Result<RetentionPlan, DbError> {
    let now = chrono::DateTime::parse_from_rfc3339(now_utc)
        .map_err(|_| DbError::Sqlite(rusqlite::Error::InvalidQuery))?
        .with_timezone(&chrono::Utc);
    let cutoff = now - chrono::Duration::days(max_days.max(0));
    let mut statement = db.conn().prepare(
        "SELECT id, timestamp_utc, length(raw_line) + length(message) + length(fields_json) FROM events ORDER BY timestamp_utc DESC, id DESC",
    )?;
    let rows = statement.query_map([], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, i64>(2)?,
        ))
    })?;
    let mut kept_count = 0i64;
    let mut kept_bytes = 0i64;
    let mut event_ids = Vec::new();
    let mut bytes_to_free = 0i64;
    for row in rows {
        let (id, timestamp, bytes) = row?;
        let event_time = chrono::DateTime::parse_from_rfc3339(&timestamp)
            .map_err(|_| DbError::Sqlite(rusqlite::Error::InvalidQuery))?
            .with_timezone(&chrono::Utc);
        let within_age = event_time >= cutoff;
        let within_count = kept_count < max_events.max(0);
        let within_size = bytes >= 0 && kept_bytes.saturating_add(bytes) <= max_bytes.max(0);
        if within_age && within_count && within_size {
            kept_count += 1;
            kept_bytes = kept_bytes.saturating_add(bytes);
        } else {
            event_ids.push(id);
            bytes_to_free = bytes_to_free.saturating_add(bytes.max(0));
        }
    }
    event_ids.sort_unstable();
    Ok(RetentionPlan {
        count: event_ids.len(),
        event_ids,
        bytes_to_free,
    })
}

pub fn apply_retention_plan(db: &Database, plan: &RetentionPlan) -> Result<usize, DbError> {
    let tx = db.conn().unchecked_transaction()?;
    let mut deleted = 0;
    for id in &plan.event_ids {
        deleted += tx.execute("DELETE FROM events WHERE id = ?1", params![id])?;
    }
    tx.commit()?;
    Ok(deleted)
}

pub fn prune_excess_events(db: &Database, max_events: i64) -> Result<usize, DbError> {
    let total: i64 = db
        .conn()
        .query_row("SELECT COUNT(*) FROM events", [], |row| row.get(0))?;
    if total <= max_events {
        return Ok(0);
    }
    let to_delete = total - max_events;
    db.conn().execute(
        "DELETE FROM events WHERE id IN (SELECT id FROM events ORDER BY timestamp_utc ASC, id ASC LIMIT ?1)",
        params![to_delete],
    ).map_err(DbError::from)
}
