use crate::db::{Database, DbError};
use crate::ingest::IngestError;
use crate::model::{Alert, DetectionRule, LogEvent, OverviewCounts, SavedQuery, Source};
use chrono::Utc;
use rusqlite::{params, params_from_iter, OptionalExtension};
use sha2::{Digest, Sha256};
use std::io::BufRead;

pub const MAX_BATCH_SIZE: usize = 5_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventSchemaField {
    pub name: String,
    pub data_type: String,
    pub required: bool,
    pub primary_key: bool,
}

pub struct SiemService {
    db: Database,
}

impl SiemService {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub fn db(&self) -> &Database {
        &self.db
    }

    pub fn default_retention_policy(&self) -> Result<crate::retention::RetentionPolicy, DbError> {
        crate::retention::get_default_policy(&self.db)
    }

    pub fn plan_retention_with_reasons(
        &self,
        max_days: i64,
        max_events: i64,
        max_bytes: i64,
        now_utc: &str,
    ) -> Result<crate::retention::RetentionBreakdown, DbError> {
        crate::retention::plan_retention_with_reasons(
            &self.db, max_days, max_events, max_bytes, now_utc,
        )
    }

    pub fn event_schema(&self) -> Result<Vec<EventSchemaField>, DbError> {
        let mut statement = self.db.conn().prepare("PRAGMA table_info(events)")?;
        let fields = statement.query_map([], |row| {
            Ok(EventSchemaField {
                name: row.get(1)?,
                data_type: row.get(2)?,
                required: row.get::<_, i64>(3)? != 0,
                primary_key: row.get::<_, i64>(5)? != 0,
            })
        })?;
        fields.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
    }

    pub fn add_source(&self, name: &str, kind: &str, location: &str) -> Result<i64, DbError> {
        let now = Utc::now().to_rfc3339();
        self.db.conn().execute(
            "INSERT INTO sources (name, kind, location, enabled, created_at)
             VALUES (?1, ?2, ?3, 1, ?4)",
            params![name, kind, location, now],
        )?;
        let id = self.db.conn().last_insert_rowid();
        self.record_audit("local", "add_source", name)?;
        Ok(id)
    }

    pub fn ingestion_stats(&self) -> Result<crate::stats::IngestionStats, DbError> {
        crate::stats::ingestion_stats(&self.db)
    }

    pub fn source_health(&self) -> Result<Vec<crate::stats::SourceHealth>, DbError> {
        crate::stats::source_health(&self.db)
    }

    pub fn source_liveness(
        &self,
        checked_at_utc: &str,
        stale_after_seconds: i64,
    ) -> Result<crate::stats::SourceLivenessReport, DbError> {
        crate::stats::source_liveness(&self.db, checked_at_utc, stale_after_seconds)
    }

    pub fn list_sources(&self) -> Result<Vec<Source>, DbError> {
        let mut stmt = self.db.conn().prepare(
            "SELECT id, name, kind, location, enabled, created_at FROM sources ORDER BY name ASC",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(Source {
                id: r.get(0)?,
                name: r.get(1)?,
                kind: r.get(2)?,
                location: r.get(3)?,
                enabled: r.get::<_, i64>(4)? != 0,
                created_at: r.get(5)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
    }

    pub fn set_source_enabled(&self, source_id: i64, enabled: bool) -> Result<bool, DbError> {
        let changed = self.db.conn().execute(
            "UPDATE sources SET enabled = ?1 WHERE id = ?2",
            params![i64::from(enabled), source_id],
        )?;
        if changed != 0 {
            self.record_audit(
                "local",
                "set_source_enabled",
                &format!("source:{source_id}:{enabled}"),
            )?;
        }
        Ok(changed != 0)
    }

    pub fn remove_source(&self, source_id: i64) -> Result<bool, DbError> {
        let tx = self.db.conn().unchecked_transaction()?;
        tx.execute(
            "DELETE FROM alerts WHERE event_id IN (SELECT id FROM events WHERE source_id = ?1)",
            params![source_id],
        )?;
        let removed = tx.execute("DELETE FROM sources WHERE id = ?1", params![source_id])?;
        tx.commit()?;
        if removed != 0 {
            self.record_audit("local", "remove_source", &format!("source:{source_id}"))?;
        }
        Ok(removed != 0)
    }

    pub fn set_rule_enabled(&self, rule_id: i64, enabled: bool) -> Result<bool, DbError> {
        let changed = self.db.conn().execute(
            "UPDATE detection_rules SET enabled = ?1 WHERE id = ?2",
            params![i64::from(enabled), rule_id],
        )?;
        if changed != 0 {
            self.record_audit(
                "local",
                "set_rule_enabled",
                &format!("rule:{rule_id}:{enabled}"),
            )?;
        }
        Ok(changed != 0)
    }

    pub fn add_rule(&self, name: &str, severity: &str, pattern: &str) -> Result<i64, DbError> {
        crate::rule_engine::validate_pattern(pattern)
            .map_err(|_| DbError::Sqlite(rusqlite::Error::InvalidQuery))?;
        let now = Utc::now().to_rfc3339();
        self.db.conn().execute(
            "INSERT INTO detection_rules (name, severity, pattern, enabled, created_at)
             VALUES (?1, ?2, ?3, 1, ?4)",
            params![name, severity, pattern, now],
        )?;
        let id = self.db.conn().last_insert_rowid();
        self.record_audit("local", "add_rule", name)?;
        Ok(id)
    }

    pub fn list_cases(
        &self,
        status: Option<&str>,
    ) -> Result<Vec<crate::cases::CaseRecord>, DbError> {
        crate::cases::CaseManager::new(&self.db).list(status)
    }

    pub fn get_case(&self, case_id: i64) -> Result<Option<crate::cases::CaseRecord>, DbError> {
        crate::cases::CaseManager::new(&self.db).get(case_id)
    }

    pub fn case_notes(&self, case_id: i64) -> Result<Vec<crate::cases::CaseNote>, DbError> {
        crate::cases::CaseManager::new(&self.db).notes(case_id)
    }

    pub fn case_alerts(&self, case_id: i64) -> Result<Vec<Alert>, DbError> {
        crate::cases::CaseManager::new(&self.db).alerts(case_id)
    }

    pub fn set_case_status(
        &self,
        case_id: i64,
        status: &str,
    ) -> Result<bool, Box<dyn std::error::Error>> {
        crate::cases::CaseManager::new(&self.db).set_status(case_id, status)
    }

    pub fn list_correlation_rules(
        &self,
        enabled_only: bool,
    ) -> Result<Vec<crate::correlation_admin::CorrelationRule>, DbError> {
        crate::correlation_admin::list_rules(&self.db, enabled_only)
    }

    pub fn set_correlation_rule_enabled(&self, id: i64, enabled: bool) -> Result<bool, DbError> {
        crate::correlation_admin::set_enabled(&self.db, id, enabled)
    }

    pub fn list_rules(&self) -> Result<Vec<DetectionRule>, DbError> {
        let mut stmt = self.db.conn().prepare(
            "SELECT id, name, severity, pattern, enabled, created_at FROM detection_rules ORDER BY id ASC",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(DetectionRule {
                id: r.get(0)?,
                name: r.get(1)?,
                severity: r.get(2)?,
                pattern: r.get(3)?,
                enabled: r.get::<_, i64>(4)? != 0,
                created_at: r.get(5)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
    }

    pub fn ingest_lines(&self, source_id: i64, lines: &[&str]) -> Result<IngestSummary, DbError> {
        self.ingest_with_policy(
            source_id,
            lines,
            crate::intake_queue::FullPolicy::DropNewest,
        )
    }

    pub fn ingest_reader<R: BufRead>(
        &self,
        source_id: i64,
        reader: R,
    ) -> Result<IngestSummary, StreamIngestError> {
        self.ingest_reader_with_format(source_id, reader, crate::parser_suite::LogFormat::Auto)
    }

    pub fn ingest_reader_with_format<R: BufRead>(
        &self,
        source_id: i64,
        mut reader: R,
        format: crate::parser_suite::LogFormat,
    ) -> Result<IngestSummary, StreamIngestError> {
        let mut total = IngestSummary::default();
        let mut batch = Vec::with_capacity(MAX_BATCH_SIZE);
        loop {
            let mut bytes = Vec::with_capacity(512);
            let Some(oversized) =
                read_bounded_line(&mut reader, &mut bytes, crate::ingest::MAX_LINE_BYTES)?
            else {
                break;
            };
            let mut line = String::from_utf8_lossy(&bytes).into_owned();
            if oversized && line.len() <= crate::ingest::MAX_LINE_BYTES {
                line.push(' ');
            }
            batch.push(line);
            if batch.len() == MAX_BATCH_SIZE {
                add_summaries(
                    &mut total,
                    self.ingest_string_batch(source_id, &batch, format)?,
                );
                batch.clear();
            }
        }
        if !batch.is_empty() {
            add_summaries(
                &mut total,
                self.ingest_string_batch(source_id, &batch, format)?,
            );
        }
        Ok(total)
    }

    fn ingest_string_batch(
        &self,
        source_id: i64,
        batch: &[String],
        format: crate::parser_suite::LogFormat,
    ) -> Result<IngestSummary, DbError> {
        let lines: Vec<&str> = batch.iter().map(String::as_str).collect();
        self.ingest_with_format(
            source_id,
            &lines,
            format,
            crate::intake_queue::FullPolicy::DropNewest,
        )
    }

    pub fn ingest_with_policy(
        &self,
        source_id: i64,
        lines: &[&str],
        policy: crate::intake_queue::FullPolicy,
    ) -> Result<IngestSummary, DbError> {
        self.ingest_with_format(
            source_id,
            lines,
            crate::parser_suite::LogFormat::Auto,
            policy,
        )
    }

    pub fn ingest_with_format(
        &self,
        source_id: i64,
        lines: &[&str],
        format: crate::parser_suite::LogFormat,
        policy: crate::intake_queue::FullPolicy,
    ) -> Result<IngestSummary, DbError> {
        let mut summary = IngestSummary::default();
        match policy {
            crate::intake_queue::FullPolicy::DropNewest => {
                let queue = crate::intake_queue::BoundedIntake::new(MAX_BATCH_SIZE);
                for line in lines {
                    let _ = queue.push(*line, policy, std::time::Duration::ZERO);
                }
                let mut accepted = Vec::new();
                while let Some(line) = queue.pop(std::time::Duration::ZERO) {
                    accepted.push(line);
                }
                summary = self.ingest_batch(
                    source_id,
                    &accepted,
                    format,
                    queue.counters().dropped as usize,
                )?;
            }
            crate::intake_queue::FullPolicy::BlockReader => {
                for batch in lines.chunks(MAX_BATCH_SIZE) {
                    let result = self.ingest_batch(source_id, batch, format, 0)?;
                    summary.received += result.received;
                    summary.deduplicated += result.deduplicated;
                    summary.malformed += result.malformed;
                    summary.alerts_created += result.alerts_created;
                    summary.dropped += result.dropped;
                }
            }
        }
        Ok(summary)
    }

    fn ingest_batch(
        &self,
        source_id: i64,
        lines: &[&str],
        format: crate::parser_suite::LogFormat,
        dropped: usize,
    ) -> Result<IngestSummary, DbError> {
        let mut summary = IngestSummary::default();
        let enabled: Option<i64> = self
            .db
            .conn()
            .query_row(
                "SELECT enabled FROM sources WHERE id = ?1",
                params![source_id],
                |row| row.get(0),
            )
            .optional()?;
        if enabled != Some(1) {
            return Err(DbError::Sqlite(rusqlite::Error::InvalidQuery));
        }
        let now = Utc::now().to_rfc3339();
        let rules = self.list_rules()?;
        summary.dropped = dropped;

        for raw in lines {
            let parsed = match crate::parser_suite::parse_with_format(raw, format) {
                Ok(ev) => ev,
                Err(IngestError::Empty) => continue,
                Err(error) => {
                    summary.malformed += 1;
                    let (safe_line, _) = crate::ingest::redact_secrets(raw);
                    let (safe_reason, _) = crate::ingest::redact_secrets(&error.to_string());
                    self.db.conn().execute(
                        "INSERT INTO malformed_events (source_id, received_at, raw_line, reason) VALUES (?1, ?2, ?3, ?4)",
                        params![source_id, now, safe_line, safe_reason],
                    )?;
                    continue;
                }
            };

            let exists: i64 = self.db.conn().query_row(
                "SELECT COUNT(*) FROM events WHERE dedup_hash = ?1",
                params![parsed.dedup_hash],
                |r| r.get(0),
            )?;
            if exists > 0 {
                summary.deduplicated += 1;
                continue;
            }

            self.db.conn().execute(
                "INSERT INTO events (source_id, timestamp_utc, received_at, severity, host, service, message, raw_line, dedup_hash, redacted, fields_json)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                params![
                    source_id,
                    parsed.timestamp_utc,
                    now,
                    parsed.severity.as_str(),
                    parsed.host,
                    parsed.service,
                    parsed.message,
                    parsed.raw_line,
                    parsed.dedup_hash,
                    if parsed.redacted { 1 } else { 0 },
                    parsed.fields_json,
                ],
            )?;
            let event_id = self.db.conn().last_insert_rowid();
            summary.received += 1;

            for rule in crate::rule_engine::evaluate_rules(&rules, &parsed.message) {
                let title = format!("{}: {}", rule.name, parsed.host);
                self.db.conn().execute(
                    "INSERT INTO alerts (rule_id, event_id, severity, status, title, case_ref, created_at, updated_at)
                     VALUES (?1, ?2, ?3, 'open', ?4, '', ?5, ?5)",
                    params![rule.id, event_id, rule.severity, title, now],
                )?;
                self.queue_alert_notification(
                    self.db.conn().last_insert_rowid(),
                    event_id,
                    &rule.severity,
                    &title,
                    &now,
                )?;
                summary.alerts_created += 1;
            }
            summary.alerts_created += self.evaluate_correlations(
                event_id,
                &parsed.timestamp_utc,
                &parsed.host,
                &parsed.service,
                &parsed.message,
            )?;
        }

        self.db.conn().execute(
            "UPDATE ingestion_stats
             SET received = received + ?1,
                 deduplicated = deduplicated + ?2,
                 dropped = dropped + ?3,
                 malformed = malformed + ?4
             WHERE id = 1",
            params![
                summary.received as i64,
                summary.deduplicated as i64,
                summary.dropped as i64,
                summary.malformed as i64,
            ],
        )?;

        Ok(summary)
    }

    fn queue_alert_notification(
        &self,
        alert_id: i64,
        event_id: i64,
        severity: &str,
        title: &str,
        now_utc: &str,
    ) -> Result<(), DbError> {
        let payload = serde_json::json!({
            "alert_id": alert_id,
            "event_id": event_id,
            "severity": severity,
            "title": title,
        });
        let _ = crate::notifier::enqueue_alert_notification(
            &self.db,
            alert_id,
            "alert.created",
            severity,
            &payload,
            now_utc,
        )?;
        Ok(())
    }

    pub fn add_threshold_correlation(
        &self,
        detection_rule_id: i64,
        entity_field: &str,
        window_seconds: i64,
        threshold: usize,
    ) -> Result<i64, DbError> {
        if !matches!(entity_field, "host" | "service" | "global")
            || !(1..=2_592_000).contains(&window_seconds)
            || !(1..=100_000).contains(&threshold)
        {
            return Err(DbError::Sqlite(rusqlite::Error::InvalidQuery));
        }
        let pattern: String = self.db.conn().query_row(
            "SELECT pattern FROM detection_rules WHERE id = ?1",
            params![detection_rule_id],
            |row| row.get(0),
        )?;
        let now = Utc::now().to_rfc3339();
        self.db.conn().execute(
            "INSERT INTO correlation_rules (detection_rule_id, kind, entity_field, window_seconds, threshold, first_pattern, second_pattern, created_at)
             VALUES (?1, 'threshold', ?2, ?3, ?4, ?5, '', ?6)",
            params![detection_rule_id, entity_field, window_seconds, threshold as i64, pattern, now],
        )?;
        Ok(self.db.conn().last_insert_rowid())
    }

    pub fn add_sequence_correlation(
        &self,
        detection_rule_id: i64,
        entity_field: &str,
        window_seconds: i64,
        first_pattern: &str,
        second_pattern: &str,
    ) -> Result<i64, DbError> {
        if !matches!(entity_field, "host" | "service" | "global")
            || !(1..=2_592_000).contains(&window_seconds)
            || first_pattern.trim().is_empty()
            || second_pattern.trim().is_empty()
        {
            return Err(DbError::Sqlite(rusqlite::Error::InvalidQuery));
        }
        let now = Utc::now().to_rfc3339();
        self.db.conn().execute(
            "INSERT INTO correlation_rules (detection_rule_id, kind, entity_field, window_seconds, threshold, first_pattern, second_pattern, created_at)
             VALUES (?1, 'sequence', ?2, ?3, 1, ?4, ?5, ?6)",
            params![detection_rule_id, entity_field, window_seconds, first_pattern.trim(), second_pattern.trim(), now],
        )?;
        Ok(self.db.conn().last_insert_rowid())
    }

    fn evaluate_correlations(
        &self,
        event_id: i64,
        timestamp_utc: &str,
        host: &str,
        service: &str,
        message: &str,
    ) -> Result<usize, DbError> {
        let mut statement = self.db.conn().prepare(
            "SELECT cr.detection_rule_id, cr.kind, cr.entity_field, cr.window_seconds, cr.threshold, cr.first_pattern, cr.second_pattern, dr.name, dr.severity
             FROM correlation_rules cr JOIN detection_rules dr ON dr.id = cr.detection_rule_id
             WHERE cr.enabled = 1 AND dr.enabled = 1 ORDER BY cr.id",
        )?;
        let configs = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, String>(8)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        drop(statement);
        let mut created = 0;
        for (
            rule_id,
            kind,
            field,
            window,
            threshold,
            first_pattern,
            second_pattern,
            name,
            severity,
        ) in configs
        {
            let key = match field.as_str() {
                "host" => host,
                "service" => service,
                _ => "global",
            };
            let lower = message.to_lowercase();
            let action = if lower.contains(&first_pattern.to_lowercase()) {
                Some("first")
            } else if kind == "sequence" && lower.contains(&second_pattern.to_lowercase()) {
                Some("second")
            } else {
                None
            };
            let Some(action) = action else { continue };
            let incoming = [crate::correlation_state::PendingEvent {
                event_id,
                timestamp_utc: timestamp_utc.to_string(),
                action: action.to_string(),
            }];
            let now = Utc::now().to_rfc3339();
            let result = if kind == "threshold" {
                crate::correlation_state::evaluate_threshold(
                    &self.db,
                    &format!("rule:{rule_id}"),
                    key,
                    window,
                    threshold as usize,
                    &incoming,
                    &now,
                )?
            } else {
                crate::correlation_state::evaluate_sequence(
                    &self.db,
                    &format!("rule:{rule_id}"),
                    key,
                    crate::correlation_state::SequenceSpec {
                        first_action: "first",
                        second_action: "second",
                        window_seconds: window,
                    },
                    &incoming,
                    &now,
                )?
            };
            if result.emitted {
                let title = format!("Correlation: {name} on {key}");
                self.db.conn().execute(
                    "INSERT INTO alerts (rule_id, event_id, severity, status, title, case_ref, created_at, updated_at)
                     VALUES (?1, ?2, ?3, 'open', ?4, '', ?5, ?5)",
                    params![rule_id, event_id, severity, title, now],
                )?;
                self.queue_alert_notification(
                    self.db.conn().last_insert_rowid(),
                    event_id,
                    &severity,
                    &title,
                    &now,
                )?;
                created += 1;
            }
        }
        Ok(created)
    }

    pub fn search_events_filtered(
        &self,
        filter: &crate::event_query::EventFilter,
    ) -> Result<Vec<LogEvent>, DbError> {
        let plan = filter.search_plan()?;
        let mut statement = self.db.conn().prepare(&plan.sql)?;
        let rows = statement.query_map(params_from_iter(plan.parameters.iter()), map_event)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
    }

    pub fn count_events_filtered(
        &self,
        filter: &crate::event_query::EventFilter,
    ) -> Result<i64, DbError> {
        let plan = filter.count_plan()?;
        self.db
            .conn()
            .query_row(&plan.sql, params_from_iter(plan.parameters.iter()), |row| {
                row.get(0)
            })
            .map_err(DbError::from)
    }

    pub fn search_events(&self, query: &str, limit: usize) -> Result<Vec<LogEvent>, DbError> {
        let trimmed = query.trim();
        let limit_i64 = limit.min(500) as i64;
        if trimmed.is_empty() {
            let mut stmt = self.db.conn().prepare(
                "SELECT id, source_id, timestamp_utc, received_at, severity, host, service, message, raw_line, dedup_hash, redacted, fields_json
                 FROM events
                 ORDER BY timestamp_utc DESC
                 LIMIT ?1",
            )?;
            let rows = stmt.query_map(params![limit_i64], map_event)?;
            return rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from);
        }

        let compiled = crate::query_language::compile_query(trimmed)
            .map_err(|_| DbError::Sqlite(rusqlite::Error::InvalidQuery))?;
        let fts = (!compiled.fts_match.is_empty()).then_some(compiled.fts_match);
        let mut stmt = self.db.conn().prepare(
            "SELECT e.id, e.source_id, e.timestamp_utc, e.received_at, e.severity, e.host, e.service, e.message, e.raw_line, e.dedup_hash, e.redacted, e.fields_json
             FROM events e
             WHERE (?1 IS NULL OR e.id IN (SELECT rowid FROM events_fts WHERE events_fts MATCH ?1))
               AND (?2 IS NULL OR e.timestamp_utc >= ?2)
               AND (?3 IS NULL OR e.timestamp_utc <= ?3)
               AND (?4 IS NULL OR lower(e.severity) = ?4)
               AND (?5 IS NULL OR e.host = ?5)
             ORDER BY e.timestamp_utc DESC
             LIMIT ?6",
        )?;
        let rows = stmt.query_map(
            params![
                fts,
                compiled.lower_timestamp,
                compiled.upper_timestamp,
                compiled.severity,
                compiled.host,
                limit_i64
            ],
            map_event,
        )?;
        rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
    }

    pub fn event_rate_series(
        &self,
        start_utc: &str,
        end_utc: &str,
        bucket_size: crate::event_analytics::BucketSize,
    ) -> Result<Vec<crate::event_analytics::TimeBucket>, DbError> {
        crate::event_analytics::event_rate_series(&self.db, start_utc, end_utc, bucket_size)
    }

    pub fn count_events_in_range(&self, start_utc: &str, end_utc: &str) -> Result<i64, DbError> {
        crate::stats::count_events_in_range(&self.db, start_utc, end_utc)
    }

    pub fn overview_counts(&self) -> Result<OverviewCounts, DbError> {
        self.db
            .conn()
            .query_row(
                "SELECT total_events, active_sources, open_alerts, stat_received, stat_deduplicated, stat_dropped, stat_malformed
                 FROM v_overview_counts",
                [],
                |r| {
                    Ok(OverviewCounts {
                        total_events: r.get(0)?,
                        active_sources: r.get(1)?,
                        open_alerts: r.get(2)?,
                        stat_received: r.get(3)?,
                        stat_deduplicated: r.get(4)?,
                        stat_dropped: r.get(5)?,
                        stat_malformed: r.get(6)?,
                    })
                },
            )
            .map_err(DbError::from)
    }

    pub fn list_alerts(&self, status_filter: Option<&str>) -> Result<Vec<Alert>, DbError> {
        let mut stmt = match status_filter {
            Some(_) => self.db.conn().prepare(
                "SELECT id, rule_id, event_id, severity, status, title, case_ref, created_at, updated_at
                 FROM alerts WHERE status = ?1 ORDER BY id DESC",
            )?,
            None => self.db.conn().prepare(
                "SELECT id, rule_id, event_id, severity, status, title, case_ref, created_at, updated_at
                 FROM alerts ORDER BY id DESC",
            )?,
        };

        let mapper = |r: &rusqlite::Row<'_>| {
            Ok(Alert {
                id: r.get(0)?,
                rule_id: r.get(1)?,
                event_id: r.get(2)?,
                severity: r.get(3)?,
                status: r.get(4)?,
                title: r.get(5)?,
                case_ref: r.get(6)?,
                created_at: r.get(7)?,
                updated_at: r.get(8)?,
            })
        };

        let rows = match status_filter {
            Some(st) => stmt.query_map(params![st], mapper)?,
            None => stmt.query_map([], mapper)?,
        };
        rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
    }

    pub fn update_alert_status(&self, alert_id: i64, status: &str) -> Result<(), DbError> {
        let now = Utc::now().to_rfc3339();
        self.db.conn().execute(
            "UPDATE alerts SET status = ?1, updated_at = ?2 WHERE id = ?3",
            params![status, now, alert_id],
        )?;
        self.record_audit(
            "local",
            "update_alert",
            &format!("alert:{alert_id}:{status}"),
        )?;
        Ok(())
    }

    pub fn save_query(&self, name: &str, query: &str) -> Result<i64, DbError> {
        let now = Utc::now().to_rfc3339();
        self.db.conn().execute(
            "INSERT INTO saved_queries (name, query, created_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(name) DO UPDATE SET query = excluded.query",
            params![name, query, now],
        )?;
        Ok(self.db.conn().last_insert_rowid())
    }

    pub fn list_saved_queries(&self) -> Result<Vec<SavedQuery>, DbError> {
        let mut stmt = self
            .db
            .conn()
            .prepare("SELECT id, name, query, created_at FROM saved_queries ORDER BY name ASC")?;
        let rows = stmt.query_map([], |r| {
            Ok(SavedQuery {
                id: r.get(0)?,
                name: r.get(1)?,
                query: r.get(2)?,
                created_at: r.get(3)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
    }

    pub fn verify_audit_chain(&self) -> Result<crate::audit_verify::AuditVerifyResult, DbError> {
        crate::audit_verify::verify_audit_chain(&self.db)
    }

    pub fn query_audit_log(
        &self,
        filter: &crate::audit_verify::AuditFilter,
    ) -> Result<Vec<crate::audit_verify::AuditEntry>, DbError> {
        crate::audit_verify::query_audit_log(&self.db, filter)
    }

    pub fn record_audit(&self, actor: &str, action: &str, target: &str) -> Result<(), DbError> {
        let now = Utc::now().to_rfc3339();
        let prev_hash: String = self
            .db
            .conn()
            .query_row(
                "SELECT entry_hash FROM audit_log ORDER BY id DESC LIMIT 1",
                [],
                |r| r.get(0),
            )
            .unwrap_or_else(|_| "genesis".to_string());

        let mut hasher = Sha256::new();
        hasher.update(prev_hash.as_bytes());
        hasher.update(actor.as_bytes());
        hasher.update(action.as_bytes());
        hasher.update(target.as_bytes());
        hasher.update(now.as_bytes());
        let entry_hash = format!("{:x}", hasher.finalize());

        self.db.conn().execute(
            "INSERT INTO audit_log (actor, action, target, created_at, prev_hash, entry_hash)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![actor, action, target, now, prev_hash, entry_hash],
        )?;
        Ok(())
    }

    pub fn seed_demo_data(&self) -> Result<(), DbError> {
        if !self.list_sources()?.is_empty() {
            return Ok(());
        }
        let src_id = self.add_source("demo-auth", "synthetic", "demo://auth.log")?;
        self.add_rule("Failed SSH login", "warn", "Failed password")?;
        self.add_rule("Privilege escalation", "critical", "sudo")?;
        self.save_query("SSH failures", "Failed")?;

        let demo_lines = [
            "2026-11-21T08:00:00Z web-01 sshd: Failed password for invalid user admin",
            "2026-11-21T08:01:00Z db-01 sudo: pam_unix(sudo:session): session opened for user root",
            r#"{"timestamp":"2026-11-21T08:02:00Z","severity":"info","host":"api-01","service":"gateway","message":"Health check ok token=demo123"}"#,
        ];
        self.ingest_lines(src_id, &demo_lines)?;
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum StreamIngestError {
    #[error(transparent)]
    Database(#[from] DbError),
    #[error("read input: {0}")]
    Io(#[from] std::io::Error),
}

fn add_summaries(total: &mut IngestSummary, part: IngestSummary) {
    total.received += part.received;
    total.deduplicated += part.deduplicated;
    total.dropped += part.dropped;
    total.malformed += part.malformed;
    total.alerts_created += part.alerts_created;
}

fn read_bounded_line<R: BufRead>(
    reader: &mut R,
    output: &mut Vec<u8>,
    max_bytes: usize,
) -> Result<Option<bool>, std::io::Error> {
    let mut oversized = false;
    loop {
        let (consumed, content_len, has_newline, copied) = {
            let available = reader.fill_buf()?;
            if available.is_empty() {
                return Ok((!output.is_empty()).then_some(oversized || output.len() > max_bytes));
            }
            let newline = available.iter().position(|byte| *byte == b'\n');
            let consumed = newline.map_or(available.len(), |index| index + 1);
            let content_len = newline.unwrap_or(consumed);
            let room = max_bytes.saturating_add(1).saturating_sub(output.len());
            let copied = content_len.min(room);
            output.extend_from_slice(&available[..copied]);
            (consumed, content_len, newline.is_some(), copied)
        };
        oversized |= copied < content_len;
        reader.consume(consumed);
        if has_newline {
            return Ok(Some(oversized || output.len() > max_bytes));
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct IngestSummary {
    pub received: usize,
    pub deduplicated: usize,
    pub dropped: usize,
    pub malformed: usize,
    pub alerts_created: usize,
}

fn map_event(r: &rusqlite::Row<'_>) -> rusqlite::Result<LogEvent> {
    Ok(LogEvent {
        id: r.get(0)?,
        source_id: r.get(1)?,
        timestamp_utc: r.get(2)?,
        received_at: r.get(3)?,
        severity: r.get(4)?,
        host: r.get(5)?,
        service: r.get(6)?,
        message: r.get(7)?,
        raw_line: r.get(8)?,
        dedup_hash: r.get(9)?,
        redacted: r.get::<_, i64>(10)? != 0,
        fields_json: r.get(11)?,
    })
}
