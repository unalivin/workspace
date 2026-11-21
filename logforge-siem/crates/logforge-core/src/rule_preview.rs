use crate::db::DbError;
use crate::event_query::EventFilter;
use crate::model::LogEvent;
use crate::rule_engine::evaluate_with_reasons;
use crate::SiemService;
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RulePreviewMatch {
    pub rule_id: i64,
    pub rule_name: String,
    pub severity: String,
    pub matched_pattern: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EventPreview {
    pub event_id: i64,
    pub timestamp_utc: String,
    pub host: String,
    pub service: String,
    pub severity: String,
    pub message: String,
    pub matched_rules: Vec<RulePreviewMatch>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RuleMatchCount {
    pub rule_id: i64,
    pub rule_name: String,
    pub severity: String,
    pub matched_events: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RulePreviewReport {
    pub examined_events: usize,
    pub matched_events: usize,
    pub total_matches: usize,
    pub rules: Vec<RuleMatchCount>,
    pub events: Vec<EventPreview>,
}

/// Evaluates active detection rules against stored events without creating
/// alerts, updating event state, or writing to the audit trail. This lets an
/// analyst estimate the impact of existing rules over a bounded query window.
pub fn preview_detection_rules(
    service: &SiemService,
    filter: &EventFilter,
) -> Result<RulePreviewReport, DbError> {
    let events = service.search_events_filtered(filter)?;
    let rules = service.list_rules()?;
    let mut counts: BTreeMap<i64, RuleMatchCount> = BTreeMap::new();
    let mut preview_events = Vec::new();
    let mut total_matches = 0usize;
    for event in events.iter() {
        let matched_rules = evaluate_with_reasons(&rules, &event.message)
            .into_iter()
            .map(|matched| {
                let entry = counts.entry(matched.rule.id).or_insert_with(|| RuleMatchCount {
                    rule_id: matched.rule.id,
                    rule_name: matched.rule.name.clone(),
                    severity: matched.rule.severity.clone(),
                    matched_events: 0,
                });
                entry.matched_events += 1;
                total_matches += 1;
                RulePreviewMatch {
                    rule_id: matched.rule.id,
                    rule_name: matched.rule.name.clone(),
                    severity: matched.rule.severity.clone(),
                    matched_pattern: matched.matched_pattern,
                }
            })
            .collect::<Vec<_>>();
        if !matched_rules.is_empty() {
            preview_events.push(event_preview(event, matched_rules));
        }
    }
    let mut rule_counts = counts.into_values().collect::<Vec<_>>();
    rule_counts.sort_by(|left, right| {
        right
            .matched_events
            .cmp(&left.matched_events)
            .then_with(|| left.rule_name.cmp(&right.rule_name))
    });
    Ok(RulePreviewReport {
        examined_events: events.len(),
        matched_events: preview_events.len(),
        total_matches,
        rules: rule_counts,
        events: preview_events,
    })
}

fn event_preview(event: &LogEvent, matched_rules: Vec<RulePreviewMatch>) -> EventPreview {
    EventPreview {
        event_id: event.id,
        timestamp_utc: event.timestamp_utc.clone(),
        host: event.host.clone(),
        service: event.service.clone(),
        severity: event.severity.to_string(),
        message: event.message.clone(),
        matched_rules,
    }
}
