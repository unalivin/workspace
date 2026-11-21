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

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AlertBulkFilter {
    pub current_status: Option<String>,
    pub severity: Option<String>,
    pub created_before: Option<String>,
    pub created_after: Option<String>,
    pub alert_ids: Vec<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AlertWorkflowError {
    InvalidStatus,
    InvalidSeverity,
    InvalidTimeRange,
}

impl std::fmt::Display for AlertWorkflowError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidStatus => f.write_str("status must be open, acknowledged, or resolved"),
            Self::InvalidSeverity => {
                f.write_str("severity must be debug, info, warn, error, or critical")
            }
            Self::InvalidTimeRange => f.write_str("invalid alert time range"),
        }
    }
}

impl std::error::Error for AlertWorkflowError {}

/// Atomically updates alerts selected by a validated filter. When explicit ids
/// are supplied, other selector fields still apply as additional constraints.
pub fn bulk_update_alerts(
    db: &Database,
    filter: &AlertBulkFilter,
    new_status: &str,
) -> Result<usize, Box<dyn std::error::Error>> {
    if !matches!(new_status, "open" | "acknowledged" | "resolved") {
        return Err(Box::new(AlertWorkflowError::InvalidStatus));
    }
    if filter
        .current_status
        .as_deref()
        .is_some_and(|status| !matches!(status, "open" | "acknowledged" | "resolved"))
    {
        return Err(Box::new(AlertWorkflowError::InvalidStatus));
    }
    if filter.severity.as_deref().is_some_and(|severity| {
        !["debug", "info", "warn", "error", "critical"].contains(&severity)
    }) {
        return Err(Box::new(AlertWorkflowError::InvalidSeverity));
    }
    let parse = |value: &Option<String>| -> Result<Option<chrono::DateTime<chrono::FixedOffset>>, AlertWorkflowError> {
        value.as_deref().map(|text| {
            chrono::DateTime::parse_from_rfc3339(text)
                .map_err(|_| AlertWorkflowError::InvalidTimeRange)
        }).transpose()
    };
    let created_after = parse(&filter.created_after)?;
    let created_before = parse(&filter.created_before)?;
    if created_after
        .as_ref()
        .zip(created_before.as_ref())
        .is_some_and(|(after, before)| after >= before)
    {
        return Err(Box::new(AlertWorkflowError::InvalidTimeRange));
    }
    if filter.alert_ids.iter().any(|id| *id <= 0) {
        return Err(Box::new(AlertWorkflowError::InvalidTimeRange));
    }
    if filter.alert_ids.is_empty()
        && filter.current_status.is_none()
        && filter.severity.is_none()
        && created_after.is_none()
        && created_before.is_none()
    {
        return Err(Box::new(AlertWorkflowError::InvalidTimeRange));
    }
    let mut conditions = Vec::new();
    let mut values = Vec::new();
    if let Some(status) = &filter.current_status {
        values.push(rusqlite::types::Value::Text(status.clone()));
        conditions.push(format!("status = ?{}", values.len()));
    }
    if let Some(severity) = &filter.severity {
        values.push(rusqlite::types::Value::Text(severity.clone()));
        conditions.push(format!("severity = ?{}", values.len()));
    }
    if let Some(after) = created_after {
        values.push(rusqlite::types::Value::Text(after.with_timezone(&Utc).to_rfc3339()));
        conditions.push(format!("created_at >= ?{}", values.len()));
    }
    if let Some(before) = created_before {
        values.push(rusqlite::types::Value::Text(before.with_timezone(&Utc).to_rfc3339()));
        conditions.push(format!("created_at < ?{}", values.len()));
    }
    if !filter.alert_ids.is_empty() {
        let slots = filter.alert_ids.iter().map(|id| {
            values.push(rusqlite::types::Value::Integer(*id));
            format!("?{}", values.len())
        }).collect::<Vec<_>>();
        conditions.push(format!("id IN ({})", slots.join(", ")));
    }
    let now = Utc::now().to_rfc3339();
    let selector = shift_placeholders(&conditions.join(" AND "));
    let sql = format!("UPDATE alerts SET status = ?1, updated_at = ?2 WHERE {selector}");
    let mut all_values = vec![
        rusqlite::types::Value::Text(new_status.to_string()),
        rusqlite::types::Value::Text(now),
    ];
    all_values.extend(values);
    // Shift selector placeholders because the two update values occupy slots one and two.
    let shifted_sql = shift_placeholders(&sql);
    let tx = db.conn().unchecked_transaction()?;
    let updated = tx.execute(&shifted_sql, rusqlite::params_from_iter(all_values.iter()))?;
    tx.commit()?;
    Ok(updated)
}

fn shift_placeholders(sql: &str) -> String {
    let mut output = String::with_capacity(sql.len() + 8);
    let mut chars = sql.chars().peekable();
    while let Some(character) = chars.next() {
        output.push(character);
        if character == '?' {
            let mut digits = String::new();
            while chars.peek().is_some_and(char::is_ascii_digit) {
                digits.push(chars.next().expect("peeked digit"));
            }
            if !digits.is_empty() {
                let index = digits.parse::<usize>().unwrap_or(0).saturating_add(2);
                output.push_str(&index.to_string());
            }
        }
    }
    output
}
