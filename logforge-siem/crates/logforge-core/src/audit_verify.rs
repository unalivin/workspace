use crate::db::{Database, DbError};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, PartialEq, Eq)]
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
