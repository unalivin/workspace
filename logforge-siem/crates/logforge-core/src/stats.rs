use crate::db::{Database, DbError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceCount {
    pub name: String,
    pub kind: String,
    pub event_count: i64,
}

pub fn top_sources(db: &Database) -> Result<Vec<SourceCount>, DbError> {
    let mut stmt = db
        .conn()
        .prepare("SELECT name, kind, event_count FROM v_top_sources")?;
    let rows = stmt.query_map([], |r| {
        Ok(SourceCount {
            name: r.get(0)?,
            kind: r.get(1)?,
            event_count: r.get(2)?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
}
