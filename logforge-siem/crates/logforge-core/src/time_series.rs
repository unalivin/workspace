use crate::db::{Database, DbError};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TimeBucket {
    pub hour_utc: String,
    pub event_count: i64,
}

pub fn events_per_hour(db: &Database, limit: usize) -> Result<Vec<TimeBucket>, DbError> {
    let mut stmt = db.conn().prepare(
        "SELECT substr(timestamp_utc, 1, 13) || ':00Z' AS bucket, COUNT(*)
         FROM events
         GROUP BY bucket
         ORDER BY bucket DESC
         LIMIT ?1",
    )?;
    let rows = stmt.query_map([limit as i64], |r| {
        Ok(TimeBucket {
            hour_utc: r.get(0)?,
            event_count: r.get(1)?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
}
