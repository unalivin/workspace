use crate::db::DbError;
use rusqlite::types::Value;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EventFilter {
    pub query: Option<String>,
    pub source_id: Option<i64>,
    pub severity: Option<String>,
    pub host: Option<String>,
    pub service: Option<String>,
    pub from_utc: Option<String>,
    pub to_utc: Option<String>,
    pub redacted: Option<bool>,
    pub after_timestamp: Option<String>,
    pub after_id: Option<i64>,
    pub limit: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct QueryPlan {
    pub sql: String,
    pub parameters: Vec<Value>,
}

impl EventFilter {
    pub fn validate(&self) -> Result<(), DbError> {
        if self.limit > 500 {
            return Err(invalid_query());
        }
        if let Some(severity) = &self.severity {
            if !["debug", "info", "warn", "error", "critical"]
                .contains(&severity.to_ascii_lowercase().as_str())
            {
                return Err(invalid_query());
            }
        }
        if self.after_timestamp.is_some() != self.after_id.is_some() {
            return Err(invalid_query());
        }
        for date in [&self.from_utc, &self.to_utc, &self.after_timestamp]
            .into_iter()
            .flatten()
        {
            if chrono::DateTime::parse_from_rfc3339(date).is_err() {
                return Err(invalid_query());
            }
        }
        if let (Some(start), Some(end)) = (&self.from_utc, &self.to_utc) {
            let start = chrono::DateTime::parse_from_rfc3339(start).map_err(|_| invalid_query())?;
            let end = chrono::DateTime::parse_from_rfc3339(end).map_err(|_| invalid_query())?;
            if start > end {
                return Err(invalid_query());
            }
        }
        Ok(())
    }

    pub fn search_plan(&self) -> Result<QueryPlan, DbError> {
        self.validate()?;
        let mut conditions = Vec::new();
        let mut parameters = Vec::new();
        if let Some(query) = self
            .query
            .as_deref()
            .filter(|query| !query.trim().is_empty())
        {
            let compiled =
                crate::query_language::compile_query(query).map_err(|_| invalid_query())?;
            if !compiled.fts_match.is_empty() {
                push_condition(
                    &mut conditions,
                    &mut parameters,
                    "e.id IN (SELECT rowid FROM events_fts WHERE events_fts MATCH ?)",
                    Value::Text(compiled.fts_match),
                );
            }
            if let Some(value) = compiled.lower_timestamp {
                push_condition(
                    &mut conditions,
                    &mut parameters,
                    "e.timestamp_utc >= ?",
                    Value::Text(value),
                );
            }
            if let Some(value) = compiled.upper_timestamp {
                push_condition(
                    &mut conditions,
                    &mut parameters,
                    "e.timestamp_utc <= ?",
                    Value::Text(value),
                );
            }
            if let Some(value) = compiled.severity {
                push_condition(
                    &mut conditions,
                    &mut parameters,
                    "lower(e.severity) = ?",
                    Value::Text(value),
                );
            }
            if let Some(value) = compiled.host {
                push_condition(
                    &mut conditions,
                    &mut parameters,
                    "e.host = ?",
                    Value::Text(value),
                );
            }
        }
        if let Some(value) = self.source_id {
            push_condition(
                &mut conditions,
                &mut parameters,
                "e.source_id = ?",
                Value::Integer(value),
            );
        }
        if let Some(value) = &self.severity {
            push_condition(
                &mut conditions,
                &mut parameters,
                "lower(e.severity) = ?",
                Value::Text(value.to_ascii_lowercase()),
            );
        }
        if let Some(value) = &self.host {
            push_condition(
                &mut conditions,
                &mut parameters,
                "e.host = ?",
                Value::Text(value.clone()),
            );
        }
        if let Some(value) = &self.service {
            push_condition(
                &mut conditions,
                &mut parameters,
                "e.service = ?",
                Value::Text(value.clone()),
            );
        }
        if let Some(value) = &self.from_utc {
            push_condition(
                &mut conditions,
                &mut parameters,
                "e.timestamp_utc >= ?",
                Value::Text(value.clone()),
            );
        }
        if let Some(value) = &self.to_utc {
            push_condition(
                &mut conditions,
                &mut parameters,
                "e.timestamp_utc <= ?",
                Value::Text(value.clone()),
            );
        }
        if let Some(value) = self.redacted {
            push_condition(
                &mut conditions,
                &mut parameters,
                "e.redacted = ?",
                Value::Integer(i64::from(value)),
            );
        }
        if let (Some(timestamp), Some(id)) = (&self.after_timestamp, self.after_id) {
            parameters.push(Value::Text(timestamp.clone()));
            let timestamp_slot = parameters.len();
            parameters.push(Value::Integer(id));
            let id_slot = parameters.len();
            conditions.push(format!(
                "(e.timestamp_utc, e.id) < (?{timestamp_slot}, ?{id_slot})"
            ));
        }
        let mut sql = format!("SELECT e.id, e.source_id, e.timestamp_utc, e.received_at, e.severity, e.host, e.service, e.message, e.raw_line, e.dedup_hash, e.redacted, e.fields_json FROM events e");
        if !conditions.is_empty() {
            sql.push_str(" WHERE ");
            sql.push_str(&conditions.join(" AND "));
        }
        sql.push_str(" ORDER BY e.timestamp_utc DESC, e.id DESC");
        if self.limit > 0 {
            parameters.push(Value::Integer(self.limit as i64));
            sql.push_str(&format!(" LIMIT ?{}", parameters.len()));
        }
        Ok(QueryPlan { sql, parameters })
    }

    pub fn count_plan(&self) -> Result<QueryPlan, DbError> {
        let mut plan = self.search_plan()?;
        let select_end = plan.sql.find(" FROM events e").ok_or_else(invalid_query)?;
        plan.sql.replace_range(..select_end, "SELECT COUNT(*)");
        if let Some(order) = plan.sql.find(" ORDER BY e.timestamp_utc DESC, e.id DESC") {
            plan.sql.truncate(order);
        }
        if self.limit > 0 {
            plan.parameters.pop();
        }
        Ok(plan)
    }
}

fn push_condition(
    conditions: &mut Vec<String>,
    parameters: &mut Vec<Value>,
    clause: &str,
    value: Value,
) {
    parameters.push(value);
    conditions.push(clause.replacen('?', &format!("?{}", parameters.len()), 1));
}

fn invalid_query() -> DbError {
    DbError::Sqlite(rusqlite::Error::InvalidQuery)
}
