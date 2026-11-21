use crate::db::{Database, DbError};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct DiagnosticsReport {
    pub app_name: &'static str,
    pub version: &'static str,
    pub os: &'static str,
    pub arch: &'static str,
    pub schema_version: u32,
}

pub fn collect_diagnostics(db: &Database) -> Result<DiagnosticsReport, DbError> {
    let schema_version: u32 = db
        .conn()
        .query_row("PRAGMA user_version", [], |r| r.get(0))?;
    Ok(DiagnosticsReport {
        app_name: "LogForge-SIEM",
        version: "0.1.0-dev",
        os: std::env::consts::OS,
        arch: std::env::consts::ARCH,
        schema_version,
    })
}
