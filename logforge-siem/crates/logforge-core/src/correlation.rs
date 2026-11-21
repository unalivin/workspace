use crate::model::LogEvent;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorrelationHit {
    pub host: String,
    pub count: usize,
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
