use crate::db::{Database, DbError};
use chrono::Utc;
use rusqlite::params;

pub fn bulk_acknowledge_alerts(db: &Database, severity: &str) -> Result<usize, DbError> {
    let now = Utc::now().to_rfc3339();
    let updated = db.conn().execute(
        "UPDATE alerts SET status = 'acknowledged', updated_at = ?1 WHERE status = 'open' AND severity = ?2",
        params![now, severity],
    )?;
    Ok(updated)
}

pub fn count_open_alerts_by_severity(db: &Database) -> Result<Vec<(String, i64)>, DbError> {
    let mut stmt = db
        .conn()
        .prepare("SELECT severity, alert_count FROM v_alerts_by_severity")?;
    let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
    rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
}
