use crate::db::{Database, DbError};
use chrono::Utc;
use rusqlite::params;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaseLink {
    pub id: i64,
    pub alert_id: i64,
    pub case_ref: String,
    pub note: String,
    pub created_at: String,
}

pub fn link_alert_to_case(
    db: &Database,
    alert_id: i64,
    case_ref: &str,
    note: &str,
) -> Result<i64, DbError> {
    let now = Utc::now().to_rfc3339();
    db.conn().execute(
        "INSERT INTO case_links (alert_id, case_ref, note, created_at) VALUES (?1, ?2, ?3, ?4)",
        params![alert_id, case_ref, note, now],
    )?;
    db.conn().execute(
        "UPDATE alerts SET case_ref = ?1, updated_at = ?2 WHERE id = ?3",
        params![case_ref, now, alert_id],
    )?;
    Ok(db.conn().last_insert_rowid())
}

pub fn list_case_links(db: &Database, alert_id: i64) -> Result<Vec<CaseLink>, DbError> {
    let mut stmt = db.conn().prepare(
        "SELECT id, alert_id, case_ref, note, created_at FROM case_links WHERE alert_id = ?1 ORDER BY id DESC",
    )?;
    let rows = stmt.query_map(params![alert_id], |r| {
        Ok(CaseLink {
            id: r.get(0)?,
            alert_id: r.get(1)?,
            case_ref: r.get(2)?,
            note: r.get(3)?,
            created_at: r.get(4)?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
}
