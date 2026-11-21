use crate::db::{Database, DbError};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StorageMetrics {
    pub page_count: i64,
    pub page_size: i64,
    pub total_bytes: i64,
    pub events_count: i64,
    pub alerts_count: i64,
}

pub fn collect_storage_metrics(db: &Database) -> Result<StorageMetrics, DbError> {
    let page_count: i64 = db.conn().query_row("PRAGMA page_count", [], |r| r.get(0))?;
    let page_size: i64 = db.conn().query_row("PRAGMA page_size", [], |r| r.get(0))?;
    let events_count: i64 = db
        .conn()
        .query_row("SELECT COUNT(*) FROM events", [], |r| r.get(0))?;
    let alerts_count: i64 = db
        .conn()
        .query_row("SELECT COUNT(*) FROM alerts", [], |r| r.get(0))?;
    Ok(StorageMetrics {
        page_count,
        page_size,
        total_bytes: page_count * page_size,
        events_count,
        alerts_count,
    })
}
