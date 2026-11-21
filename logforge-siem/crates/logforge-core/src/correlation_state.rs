use crate::db::{Database, DbError};
use chrono::{DateTime, Duration, Utc};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingEvent {
    pub event_id: i64,
    pub timestamp_utc: String,
    pub action: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorrelationResult {
    pub event_ids: Vec<i64>,
    pub emitted: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SequenceSpec<'a> {
    pub first_action: &'a str,
    pub second_action: &'a str,
    pub window_seconds: i64,
}

fn load(
    db: &Database,
    rule: &str,
    key: &str,
) -> Result<(Vec<PendingEvent>, Option<String>), DbError> {
    db.conn().query_row(
        "SELECT pending_events, last_emitted_at FROM correlation_state WHERE rule_key = ?1 AND correlation_key = ?2",
        params![rule, key],
        |row| {
            let json: String = row.get(0)?;
            let last: Option<String> = row.get(1)?;
            Ok((json, last))
        },
    ).optional()?.map(|(json, last)| {
        let events = serde_json::from_str(&json).map_err(|_| DbError::Sqlite(rusqlite::Error::InvalidQuery))?;
        Ok((events, last))
    }).transpose().map(|state| state.unwrap_or_default())
}

fn save(
    db: &Database,
    rule: &str,
    key: &str,
    events: &[PendingEvent],
    emitted_at: Option<&str>,
    now: &str,
) -> Result<(), DbError> {
    let json = serde_json::to_string(events)
        .map_err(|_| DbError::Sqlite(rusqlite::Error::InvalidQuery))?;
    db.conn().execute(
        "INSERT INTO correlation_state (rule_key, correlation_key, pending_events, last_emitted_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(rule_key, correlation_key) DO UPDATE SET pending_events = excluded.pending_events,
         last_emitted_at = COALESCE(excluded.last_emitted_at, correlation_state.last_emitted_at), updated_at = excluded.updated_at",
        params![rule, key, json, emitted_at, now],
    )?;
    Ok(())
}

pub fn evaluate_threshold(
    db: &Database,
    rule: &str,
    key: &str,
    window_seconds: i64,
    threshold: usize,
    incoming: &[PendingEvent],
    now: &str,
) -> Result<CorrelationResult, DbError> {
    if window_seconds <= 0 || threshold == 0 {
        return Ok(CorrelationResult {
            event_ids: Vec::new(),
            emitted: false,
        });
    }
    let now = DateTime::parse_from_rfc3339(now)
        .map_err(|_| DbError::Sqlite(rusqlite::Error::InvalidQuery))?
        .with_timezone(&Utc);
    let (mut pending, last_emitted) = load(db, rule, key)?;
    for event in incoming {
        DateTime::parse_from_rfc3339(&event.timestamp_utc)
            .map_err(|_| DbError::Sqlite(rusqlite::Error::InvalidQuery))?;
        if !pending
            .iter()
            .any(|stored| stored.event_id == event.event_id)
        {
            pending.push(event.clone());
        }
    }
    pending.sort_by_key(|event| event.timestamp_utc.clone());
    let cutoff = now - Duration::seconds(window_seconds);
    pending.retain(|event| {
        DateTime::parse_from_rfc3339(&event.timestamp_utc)
            .is_ok_and(|time| time.with_timezone(&Utc) > cutoff)
    });
    let last_is_recent = last_emitted
        .as_deref()
        .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
        .is_some_and(|time| {
            now.signed_duration_since(time.with_timezone(&Utc))
                .num_seconds()
                < window_seconds
        });
    let emitted = pending.len() >= threshold && !last_is_recent;
    let event_ids = if emitted {
        pending.iter().map(|event| event.event_id).collect()
    } else {
        Vec::new()
    };
    let emitted_time = emitted.then(|| now.to_rfc3339());
    save(
        db,
        rule,
        key,
        &pending,
        emitted_time.as_deref(),
        &now.to_rfc3339(),
    )?;
    Ok(CorrelationResult { event_ids, emitted })
}

pub fn evaluate_sequence(
    db: &Database,
    rule: &str,
    key: &str,
    spec: SequenceSpec<'_>,
    incoming: &[PendingEvent],
    now: &str,
) -> Result<CorrelationResult, DbError> {
    let SequenceSpec {
        first_action,
        second_action,
        window_seconds,
    } = spec;
    if window_seconds <= 0 {
        return Ok(CorrelationResult {
            event_ids: Vec::new(),
            emitted: false,
        });
    }
    let now = DateTime::parse_from_rfc3339(now)
        .map_err(|_| DbError::Sqlite(rusqlite::Error::InvalidQuery))?
        .with_timezone(&Utc);
    let (mut pending, last_emitted) = load(db, rule, key)?;
    for event in incoming {
        if !pending
            .iter()
            .any(|stored| stored.event_id == event.event_id)
        {
            pending.push(event.clone());
        }
    }
    pending.sort_by_key(|event| event.timestamp_utc.clone());
    let cutoff = now - Duration::seconds(window_seconds);
    pending.retain(|event| {
        DateTime::parse_from_rfc3339(&event.timestamp_utc)
            .is_ok_and(|time| time.with_timezone(&Utc) > cutoff)
    });
    let first = pending.iter().find(|event| event.action == first_action);
    let second = pending.iter().find(|event| {
        event.action == second_action
            && first.is_some_and(|start| event.timestamp_utc >= start.timestamp_utc)
    });
    let last_is_recent = last_emitted
        .as_deref()
        .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
        .is_some_and(|time| {
            now.signed_duration_since(time.with_timezone(&Utc))
                .num_seconds()
                < window_seconds
        });
    let emitted = first.is_some() && second.is_some() && !last_is_recent;
    let event_ids = if emitted {
        vec![first.unwrap().event_id, second.unwrap().event_id]
    } else {
        Vec::new()
    };
    let emitted_time = emitted.then(|| now.to_rfc3339());
    save(
        db,
        rule,
        key,
        &pending,
        emitted_time.as_deref(),
        &now.to_rfc3339(),
    )?;
    Ok(CorrelationResult { event_ids, emitted })
}
