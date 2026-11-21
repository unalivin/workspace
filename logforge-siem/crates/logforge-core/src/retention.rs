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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetentionPlan {
    pub event_ids: Vec<i64>,
    pub count: usize,
}

pub fn plan_retention(
    db: &Database,
    max_days: i64,
    max_events: i64,
    now_utc: &str,
) -> Result<RetentionPlan, DbError> {
    let age_cutoff = chrono::DateTime::parse_from_rfc3339(now_utc)
        .map_err(|_| DbError::Sqlite(rusqlite::Error::InvalidQuery))?
        .with_timezone(&chrono::Utc)
        - chrono::Duration::days(max_days.max(0));
    let cutoff = age_cutoff.to_rfc3339();
    let mut statement = db.conn().prepare(
        "SELECT id FROM events WHERE timestamp_utc < ?1 ORDER BY timestamp_utc ASC, id ASC",
    )?;
    let old = statement
        .query_map(params![cutoff], |row| row.get::<_, i64>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    let mut keep = std::collections::HashSet::new();
    if max_events > 0 {
        let mut stmt = db
            .conn()
            .prepare("SELECT id FROM events ORDER BY timestamp_utc DESC, id DESC LIMIT ?1")?;
        for id in stmt.query_map(params![max_events], |row| row.get::<_, i64>(0))? {
            keep.insert(id?);
        }
    }
    let mut ids: Vec<i64> = old.into_iter().filter(|id| !keep.contains(id)).collect();
    let total: i64 = db
        .conn()
        .query_row("SELECT COUNT(*) FROM events", [], |row| row.get(0))?;
    if total - ids.len() as i64 > max_events.max(0) {
        let mut stmt = db
            .conn()
            .prepare("SELECT id FROM events ORDER BY timestamp_utc ASC, id ASC")?;
        for id in stmt.query_map([], |row| row.get::<_, i64>(0))? {
            let id = id?;
            if ids.len() as i64 >= total - max_events.max(0) {
                break;
            }
            if !keep.contains(&id) && !ids.contains(&id) {
                ids.push(id);
            }
        }
    }
    ids.sort_unstable();
    Ok(RetentionPlan {
        count: ids.len(),
        event_ids: ids,
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
