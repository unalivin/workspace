use crate::db::{Database, DbError};
use chrono::{DateTime, Utc};
use rusqlite::params;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HostRisk {
    pub host: String,
    pub score: u8,
    pub band: String,
    pub total_events: i64,
    pub critical_events: i64,
    pub error_events: i64,
    pub warning_events: i64,
    pub open_alerts: i64,
    pub distinct_services: i64,
    pub risk_factors: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HostRiskReport {
    pub since_utc: String,
    pub host_count: usize,
    pub high_risk_hosts: usize,
    pub hosts: Vec<HostRisk>,
}

/// Produces an explainable host-risk ranking from recent event severity,
/// unresolved alerts, and service diversity. This is an operational heuristic,
/// not a replacement for deterministic detection rules.
pub fn score_host_risk(
    db: &Database,
    since_utc: &str,
    limit: usize,
) -> Result<HostRiskReport, DbError> {
    let since = DateTime::parse_from_rfc3339(since_utc)
        .map_err(|_| DbError::Sqlite(rusqlite::Error::InvalidQuery))?
        .with_timezone(&Utc)
        .to_rfc3339();
    if limit == 0 || limit > 500 {
        return Err(DbError::Sqlite(rusqlite::Error::InvalidQuery));
    }
    let mut statement = db.conn().prepare(
        "SELECT e.host, COUNT(DISTINCT e.id), \
            SUM(CASE WHEN e.severity = 'critical' THEN 1 ELSE 0 END), \
            SUM(CASE WHEN e.severity = 'error' THEN 1 ELSE 0 END), \
            SUM(CASE WHEN e.severity = 'warn' THEN 1 ELSE 0 END), \
            COUNT(DISTINCT CASE WHEN a.status IN ('open', 'acknowledged') THEN a.id END), \
            COUNT(DISTINCT e.service) \
         FROM events e LEFT JOIN alerts a ON a.event_id = e.id \
         WHERE e.timestamp_utc >= ?1 GROUP BY e.host",
    )?;
    let rows = statement.query_map(params![since], |row| {
        let total_events: i64 = row.get(1)?;
        let critical_events = row.get::<_, Option<i64>>(2)?.unwrap_or(0);
        let error_events = row.get::<_, Option<i64>>(3)?.unwrap_or(0);
        let warning_events = row.get::<_, Option<i64>>(4)?.unwrap_or(0);
        let open_alerts: i64 = row.get(5)?;
        let distinct_services: i64 = row.get(6)?;
        let score = calculate_host_risk(
            critical_events,
            error_events,
            warning_events,
            open_alerts,
            distinct_services,
        );
        let risk_factors = explain_risk_factors(
            critical_events,
            error_events,
            warning_events,
            open_alerts,
            distinct_services,
        );
        Ok(HostRisk {
            host: row.get(0)?,
            score,
            band: risk_band(score).to_string(),
            total_events,
            critical_events,
            error_events,
            warning_events,
            open_alerts,
            distinct_services,
            risk_factors,
        })
    })?;
    let mut hosts = rows.collect::<Result<Vec<_>, _>>()?;
    hosts.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| right.total_events.cmp(&left.total_events))
            .then_with(|| left.host.cmp(&right.host))
    });
    let host_count = hosts.len();
    let high_risk_hosts = hosts.iter().filter(|host| host.score >= 60).count();
    hosts.truncate(limit);
    Ok(HostRiskReport {
        since_utc: since,
        host_count,
        high_risk_hosts,
        hosts,
    })
}

fn calculate_host_risk(
    critical: i64,
    errors: i64,
    warnings: i64,
    open_alerts: i64,
    distinct_services: i64,
) -> u8 {
    let score = critical
        .max(0)
        .saturating_mul(18)
        .saturating_add(errors.max(0).saturating_mul(12))
        .saturating_add(warnings.max(0).saturating_mul(5))
        .saturating_add(open_alerts.max(0).saturating_mul(20))
        .saturating_add(distinct_services.saturating_sub(1).max(0).saturating_mul(3));
    score.clamp(0, 100) as u8
}

fn explain_risk_factors(
    critical: i64,
    errors: i64,
    warnings: i64,
    open_alerts: i64,
    distinct_services: i64,
) -> Vec<String> {
    let mut factors = Vec::new();
    if critical > 0 {
        factors.push(format!(
            "{critical} critical events (+{})",
            critical.saturating_mul(18)
        ));
    }
    if errors > 0 {
        factors.push(format!(
            "{errors} error events (+{})",
            errors.saturating_mul(12)
        ));
    }
    if warnings > 0 {
        factors.push(format!(
            "{warnings} warning events (+{})",
            warnings.saturating_mul(5)
        ));
    }
    if open_alerts > 0 {
        factors.push(format!(
            "{open_alerts} unresolved alerts (+{})",
            open_alerts.saturating_mul(20)
        ));
    }
    if distinct_services > 1 {
        let extra = distinct_services - 1;
        factors.push(format!(
            "activity across {distinct_services} services (+{})",
            extra.saturating_mul(3)
        ));
    }
    factors
}

fn risk_band(score: u8) -> &'static str {
    match score {
        0..=19 => "low",
        20..=49 => "moderate",
        50..=79 => "high",
        _ => "critical",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn risk_scores_are_bounded_and_explainable() {
        assert_eq!(calculate_host_risk(0, 0, 0, 0, 1), 0);
        assert_eq!(calculate_host_risk(1, 0, 0, 0, 1), 18);
        assert_eq!(calculate_host_risk(20, 20, 20, 20, 8), 100);
        assert_eq!(risk_band(0), "low");
        assert_eq!(risk_band(50), "high");
        assert_eq!(risk_band(100), "critical");
    }
}
