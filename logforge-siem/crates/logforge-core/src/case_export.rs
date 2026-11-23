use crate::audit_verify::{verify_audit_chain, AuditVerifyResult};
use crate::cases::{CaseManager, CaseNote, CaseRecord};
use crate::db::{Database, DbError};
use crate::model::{Alert, LogEvent};
use rusqlite::OptionalExtension;
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize)]
pub struct CaseExportBundle {
    pub exported_at: String,
    pub case: CaseRecord,
    pub notes: Vec<CaseNote>,
    pub alerts: Vec<Alert>,
    pub events: Vec<LogEvent>,
    pub audit_chain: AuditVerifyResult,
    pub content_sha256: String,
}

pub fn export_case_bundle(
    db: &Database,
    case_id: i64,
) -> Result<Option<CaseExportBundle>, DbError> {
    let manager = CaseManager::new(db);
    let Some(case) = manager.get(case_id)? else {
        return Ok(None);
    };
    let notes = manager.notes(case_id)?;
    let alerts = manager.alerts(case_id)?;
    let mut events = BTreeMap::new();
    for alert in &alerts {
        let event = db
            .conn()
            .query_row(
                "SELECT id, source_id, timestamp_utc, received_at, severity, host, service, message, raw_line, dedup_hash, redacted, fields_json FROM events WHERE id = ?1",
                [alert.event_id],
                |row| {
                    Ok(LogEvent {
                        id: row.get(0)?,
                        source_id: row.get(1)?,
                        timestamp_utc: row.get(2)?,
                        received_at: row.get(3)?,
                        severity: row.get(4)?,
                        host: row.get(5)?,
                        service: row.get(6)?,
                        message: row.get(7)?,
                        raw_line: row.get(8)?,
                        dedup_hash: row.get(9)?,
                        redacted: row.get::<_, i64>(10)? != 0,
                        fields_json: row.get(11)?,
                    })
                },
            )
            .optional()?;
        if let Some(event) = event {
            events.insert(event.id, event);
        }
    }
    let mut bundle = CaseExportBundle {
        exported_at: chrono::Utc::now().to_rfc3339(),
        case,
        notes,
        alerts,
        events: events.into_values().collect(),
        audit_chain: verify_audit_chain(db)?,
        content_sha256: String::new(),
    };
    bundle.content_sha256 =
        compute_bundle_hash(&bundle).map_err(|_| DbError::Sqlite(rusqlite::Error::InvalidQuery))?;
    Ok(Some(bundle))
}

pub fn export_case_bundle_redacted(
    db: &Database,
    case_id: i64,
) -> Result<Option<CaseExportBundle>, DbError> {
    let Some(mut bundle) = export_case_bundle(db, case_id)? else {
        return Ok(None);
    };
    for event in &mut bundle.events {
        event.message = crate::ingest::redact_secrets(&event.message).0;
        event.raw_line = "[omitted from redacted export]".to_string();
        event.fields_json = "{}".to_string();
        event.redacted = true;
    }
    bundle.content_sha256 =
        compute_bundle_hash(&bundle).map_err(|_| DbError::Sqlite(rusqlite::Error::InvalidQuery))?;
    Ok(Some(bundle))
}

pub fn verify_case_bundle(bundle: &CaseExportBundle) -> Result<bool, serde_json::Error> {
    Ok(compute_bundle_hash(bundle)? == bundle.content_sha256)
}

fn compute_bundle_hash(bundle: &CaseExportBundle) -> Result<String, serde_json::Error> {
    let canonical = serde_json::to_vec(&(
        &bundle.case,
        &bundle.notes,
        &bundle.alerts,
        &bundle.events,
        &bundle.audit_chain,
    ))?;
    use sha2::{Digest, Sha256};
    Ok(format!("{:x}", Sha256::digest(canonical)))
}
