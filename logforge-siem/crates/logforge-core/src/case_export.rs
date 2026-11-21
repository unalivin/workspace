use crate::audit_verify::{verify_audit_chain, AuditVerifyResult};
use crate::cases::{CaseManager, CaseNote, CaseRecord};
use crate::db::{Database, DbError};
use crate::model::{Alert, LogEvent};
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CaseExportBundle {
    pub exported_at: String,
    pub case: CaseRecord,
    pub notes: Vec<CaseNote>,
    pub alerts: Vec<Alert>,
    pub events: Vec<LogEvent>,
    pub audit_chain: AuditVerifyResult,
}

/// Collects a portable incident package containing only records associated
/// with the selected case and a whole-database audit-chain integrity result.
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
    Ok(Some(CaseExportBundle {
        exported_at: chrono::Utc::now().to_rfc3339(),
        case,
        notes,
        alerts,
        events: events.into_values().collect(),
        audit_chain: verify_audit_chain(db)?,
    }))
}

use rusqlite::OptionalExtension;
