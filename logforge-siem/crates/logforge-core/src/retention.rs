use crate::db::{Database, DbError};
use rusqlite::params;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetentionPolicy {
    pub name: String,
    pub max_days: i64,
    pub max_events: i64,
    pub enabled: bool,
}

pub fn get_default_policy(db: &Database) -> Result<RetentionPolicy, DbError> {
    db.conn()
        .query_row(
            "SELECT name, max_days, max_events, enabled FROM retention_policies WHERE id = 1",
            [],
            |r| {
                Ok(RetentionPolicy {
                    name: r.get(0)?,
                    max_days: r.get(1)?,
                    max_events: r.get(2)?,
                    enabled: r.get::<_, i64>(3)? != 0,
                })
            },
        )
        .map_err(DbError::from)
}

pub fn prune_excess_events(db: &Database, max_events: i64) -> Result<usize, DbError> {
    let total: i64 = db
        .conn()
        .query_row("SELECT COUNT(*) FROM events", [], |r| r.get(0))?;
    if total <= max_events {
        return Ok(0);
    }
    let to_delete = total - max_events;
    let deleted = db.conn().execute(
        "DELETE FROM events WHERE id IN (SELECT id FROM events ORDER BY timestamp_utc ASC LIMIT ?1)",
        params![to_delete],
    )?;
    Ok(deleted)
}
