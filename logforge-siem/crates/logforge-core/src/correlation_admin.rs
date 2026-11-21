use crate::db::{Database, DbError};
use rusqlite::{params, OptionalExtension};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorrelationRule {
    pub id: i64,
    pub detection_rule_id: i64,
    pub name: String,
    pub severity: String,
    pub kind: String,
    pub entity_field: String,
    pub window_seconds: i64,
    pub threshold: i64,
    pub first_pattern: String,
    pub second_pattern: String,
    pub enabled: bool,
    pub created_at: String,
}

pub fn list_rules(db: &Database, enabled_only: bool) -> Result<Vec<CorrelationRule>, DbError> {
    let sql = if enabled_only {
        "SELECT c.id, c.detection_rule_id, d.name, d.severity, c.kind, c.entity_field, c.window_seconds, c.threshold, c.first_pattern, c.second_pattern, c.enabled, c.created_at FROM correlation_rules c JOIN detection_rules d ON d.id = c.detection_rule_id WHERE c.enabled = 1 ORDER BY c.id"
    } else {
        "SELECT c.id, c.detection_rule_id, d.name, d.severity, c.kind, c.entity_field, c.window_seconds, c.threshold, c.first_pattern, c.second_pattern, c.enabled, c.created_at FROM correlation_rules c JOIN detection_rules d ON d.id = c.detection_rule_id ORDER BY c.id"
    };
    let mut statement = db.conn().prepare(sql)?;
    let rows = statement.query_map([], |row| {
        Ok(CorrelationRule {
            id: row.get(0)?,
            detection_rule_id: row.get(1)?,
            name: row.get(2)?,
            severity: row.get(3)?,
            kind: row.get(4)?,
            entity_field: row.get(5)?,
            window_seconds: row.get(6)?,
            threshold: row.get(7)?,
            first_pattern: row.get(8)?,
            second_pattern: row.get(9)?,
            enabled: row.get::<_, i64>(10)? != 0,
            created_at: row.get(11)?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
}

pub fn set_enabled(db: &Database, id: i64, enabled: bool) -> Result<bool, DbError> {
    Ok(db.conn().execute(
        "UPDATE correlation_rules SET enabled = ?1 WHERE id = ?2",
        params![i64::from(enabled), id],
    )? != 0)
}

pub fn remove_rule(db: &Database, id: i64) -> Result<bool, DbError> {
    let tx = db.conn().unchecked_transaction()?;
    let detection_rule_id: Option<i64> = tx
        .query_row(
            "SELECT detection_rule_id FROM correlation_rules WHERE id = ?1",
            params![id],
            |row| row.get(0),
        )
        .optional()?;
    let Some(detection_rule_id) = detection_rule_id else {
        tx.commit()?;
        return Ok(false);
    };
    tx.execute("DELETE FROM correlation_rules WHERE id = ?1", params![id])?;
    tx.execute(
        "DELETE FROM correlation_state WHERE rule_key = ?1",
        params![format!("rule:{detection_rule_id}")],
    )?;
    tx.commit()?;
    Ok(true)
}
