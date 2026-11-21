use crate::model::LogEvent;
use chrono::{DateTime, Utc};
use std::collections::{HashMap, VecDeque};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorrelationHit {
    pub host: String,
    pub count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowHit {
    pub key: String,
    pub count: usize,
    pub event_ids: Vec<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SequenceHit {
    pub key: String,
    pub first_event_id: i64,
    pub second_event_id: i64,
}

pub fn correlate_by_host(events: &[LogEvent], min_count: usize) -> Vec<CorrelationHit> {
    let mut counts: HashMap<String, usize> = HashMap::new();
    for ev in events {
        *counts.entry(ev.host.clone()).or_insert(0) += 1;
    }
    let mut hits: Vec<CorrelationHit> = counts
        .into_iter()
        .filter(|(_, count)| *count >= min_count)
        .map(|(host, count)| CorrelationHit { host, count })
        .collect();
    hits.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.host.cmp(&b.host)));
    hits
}

pub fn threshold_window<I>(events: I, window_seconds: i64, threshold: usize) -> Vec<WindowHit>
where
    I: IntoIterator<Item = (String, DateTime<Utc>, i64)>,
{
    if window_seconds <= 0 || threshold == 0 {
        return Vec::new();
    }
    let mut ordered: Vec<_> = events.into_iter().collect();
    ordered.sort_by_key(|(_, timestamp, id)| (*timestamp, *id));
    let mut windows: HashMap<String, VecDeque<(DateTime<Utc>, i64)>> = HashMap::new();
    let mut emitted = HashMap::<String, DateTime<Utc>>::new();
    let mut hits = Vec::new();
    for (key, timestamp, id) in ordered {
        let queue = windows.entry(key.clone()).or_default();
        while queue.front().is_some_and(|(at, _)| {
            timestamp.signed_duration_since(*at).num_seconds() >= window_seconds
        }) {
            queue.pop_front();
        }
        queue.push_back((timestamp, id));
        let last = emitted.get(&key).copied();
        if queue.len() >= threshold
            && last.is_none_or(|at| {
                timestamp.signed_duration_since(at).num_seconds() >= window_seconds
            })
        {
            hits.push(WindowHit {
                key: key.clone(),
                count: queue.len(),
                event_ids: queue.iter().map(|(_, event_id)| *event_id).collect(),
            });
            emitted.insert(key, timestamp);
        }
    }
    hits
}

pub fn ordered_sequence<I>(
    events: I,
    first: &str,
    second: &str,
    window_seconds: i64,
) -> Vec<SequenceHit>
where
    I: IntoIterator<Item = (String, String, DateTime<Utc>, i64)>,
{
    if window_seconds <= 0 {
        return Vec::new();
    }
    let mut ordered: Vec<_> = events.into_iter().collect();
    ordered.sort_by_key(|(_, _, timestamp, id)| (*timestamp, *id));
    let mut first_seen: HashMap<String, VecDeque<(DateTime<Utc>, i64)>> = HashMap::new();
    let mut hits = Vec::new();
    for (key, action, timestamp, id) in ordered {
        let queue = first_seen.entry(key.clone()).or_default();
        while queue.front().is_some_and(|(at, _)| {
            timestamp.signed_duration_since(*at).num_seconds() > window_seconds
        }) {
            queue.pop_front();
        }
        if action == first {
            queue.push_back((timestamp, id));
        }
        if action == second {
            if let Some((_, first_id)) = queue.front().copied() {
                hits.push(SequenceHit {
                    key,
                    first_event_id: first_id,
                    second_event_id: id,
                });
            }
            queue.clear();
        }
    }
    hits
}
