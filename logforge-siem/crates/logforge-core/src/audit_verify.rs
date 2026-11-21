use crate::db::{Database, DbError};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct AuditVerifyResult {
    pub total_entries: usize,
    pub valid: bool,
    pub first_invalid_id: Option<i64>,
}

pub fn verify_audit_chain(db: &Database) -> Result<AuditVerifyResult, DbError> {
    let mut stmt = db.conn().prepare(
        "SELECT id, actor, action, target, created_at, prev_hash, entry_hash FROM audit_log ORDER BY id ASC",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, String>(3)?,
            r.get::<_, String>(4)?,
            r.get::<_, String>(5)?,
            r.get::<_, String>(6)?,
        ))
    })?;

    let mut expected_prev = "genesis".to_string();
    let mut total = 0usize;

    for row in rows {
        let (id, actor, action, target, created_at, prev_hash, entry_hash) = row?;
        total += 1;
        if prev_hash != expected_prev {
            return Ok(AuditVerifyResult {
                total_entries: total,
                valid: false,
                first_invalid_id: Some(id),
            });
        }
        let mut hasher = Sha256::new();
        hasher.update(prev_hash.as_bytes());
        hasher.update(actor.as_bytes());
        hasher.update(action.as_bytes());
        hasher.update(target.as_bytes());
        hasher.update(created_at.as_bytes());
        let computed = format!("{:x}", hasher.finalize());
        if computed != entry_hash {
            return Ok(AuditVerifyResult {
                total_entries: total,
                valid: false,
                first_invalid_id: Some(id),
            });
        }
        expected_prev = entry_hash;
    }

    Ok(AuditVerifyResult {
        total_entries: total,
        valid: true,
        first_invalid_id: None,
    })
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct AuditEntry {
    pub id: i64,
    pub actor: String,
    pub action: String,
    pub target: String,
    pub created_at: String,
    pub previous_hash: String,
    pub entry_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AuditFilter {
    pub actor: Option<String>,
    pub action: Option<String>,
    pub from_utc: Option<String>,
    pub to_utc: Option<String>,
    pub before_id: Option<i64>,
    pub limit: usize,
}

/// Lists audit records in reverse chronological order for interactive review.
/// Hash-chain verification remains a separate operation over the full chain.
pub fn query_audit_log(db: &Database, filter: &AuditFilter) -> Result<Vec<AuditEntry>, DbError> {
    if filter.limit == 0 || filter.limit > 1000 || filter.before_id.is_some_and(|id| id <= 0) {
        return Err(invalid_audit_query());
    }
    let validate_time = |value: &Option<String>| -> Result<Option<String>, DbError> {
        value
            .as_deref()
            .map(|text| {
                chrono::DateTime::parse_from_rfc3339(text)
                    .map(|time| time.with_timezone(&chrono::Utc).to_rfc3339())
                    .map_err(|_| invalid_audit_query())
            })
            .transpose()
    };
    let from = validate_time(&filter.from_utc)?;
    let to = validate_time(&filter.to_utc)?;
    if let (Some(from), Some(to)) = (&from, &to) {
        if from >= to {
            return Err(invalid_audit_query());
        }
    }
    let mut conditions = Vec::new();
    let mut values = Vec::new();
    if let Some(actor) = filter
        .actor
        .as_ref()
        .filter(|value| !value.trim().is_empty())
    {
        values.push(rusqlite::types::Value::Text(actor.trim().to_string()));
        conditions.push(format!("actor = ?{}", values.len()));
    }
    if let Some(action) = filter
        .action
        .as_ref()
        .filter(|value| !value.trim().is_empty())
    {
        values.push(rusqlite::types::Value::Text(action.trim().to_string()));
        conditions.push(format!("action = ?{}", values.len()));
    }
    if let Some(from) = from {
        values.push(rusqlite::types::Value::Text(from));
        conditions.push(format!("created_at >= ?{}", values.len()));
    }
    if let Some(to) = to {
        values.push(rusqlite::types::Value::Text(to));
        conditions.push(format!("created_at < ?{}", values.len()));
    }
    if let Some(before_id) = filter.before_id {
        values.push(rusqlite::types::Value::Integer(before_id));
        conditions.push(format!("id < ?{}", values.len()));
    }
    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!(" WHERE {}", conditions.join(" AND "))
    };
    values.push(rusqlite::types::Value::Integer(filter.limit as i64));
    let sql = format!(
        "SELECT id, actor, action, target, created_at, prev_hash, entry_hash \
         FROM audit_log{where_clause} ORDER BY id DESC LIMIT ?{}",
        values.len()
    );
    let mut statement = db.conn().prepare(&sql)?;
    let rows = statement.query_map(rusqlite::params_from_iter(values.iter()), |row| {
        Ok(AuditEntry {
            id: row.get(0)?,
            actor: row.get(1)?,
            action: row.get(2)?,
            target: row.get(3)?,
            created_at: row.get(4)?,
            previous_hash: row.get(5)?,
            entry_hash: row.get(6)?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
}

pub fn audit_action_counts(db: &Database) -> Result<Vec<(String, i64)>, DbError> {
    let mut statement = db.conn().prepare(
        "SELECT action, COUNT(*) FROM audit_log GROUP BY action ORDER BY COUNT(*) DESC, action COLLATE NOCASE",
    )?;
    let rows = statement.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
    rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
}

fn invalid_audit_query() -> DbError {
    DbError::Sqlite(rusqlite::Error::InvalidQuery)
}
