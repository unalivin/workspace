use crate::table::TableState;
use crate::GuiTab;
use eframe::egui;
use logforge_core::{event_query::EventFilter, SiemService};

pub fn render(ui: &mut egui::Ui, service: &SiemService, tab: GuiTab, table: &mut TableState) {
    let title = tab.title();
    ui.heading(title);
    if table.active_screen != Some(title) {
        table.active_screen = Some(title);
        table.loading_frames = 1;
    }
    if ui.button("Refresh from SQLite").clicked() {
        table.loading_frames = 1;
    }
    ui.label("Records are read from the shared local service.");
    if table.loading_frames > 0 {
        table.loading_frames -= 1;
        ui.label("Loading records…");
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(30));
        return;
    }
    if tab == GuiTab::Retention {
        match logforge_core::retention::get_default_policy(service.db()) {
            Ok(policy) => match logforge_core::retention::plan_retention_with_reasons(
                service.db(),
                policy.max_days,
                policy.max_events,
                policy.max_bytes,
                &chrono::Utc::now().to_rfc3339(),
            ) {
                Ok(plan) => {
                    ui.label(format!(
                        "Policy {} · max age {} days · max {} events · max {} bytes",
                        policy.name, policy.max_days, policy.max_events, policy.max_bytes
                    ));
                    ui.label(format!(
                        "Dry run: {} events, {} estimated bytes · expired={} · count-cap={} · storage-cap={}",
                        plan.plan.count, plan.plan.bytes_to_free, plan.age_expired,
                        plan.event_limit_exceeded, plan.byte_limit_exceeded
                    ));
                }
                Err(error) => {
                    ui.colored_label(egui::Color32::from_rgb(179, 38, 30), format!("Unable to plan retention: {error}"));
                }
            },
            Err(error) => {
                ui.colored_label(egui::Color32::from_rgb(179, 38, 30), format!("Unable to load retention policy: {error}"));
            }
        }
    }
    let loaded = load(service, tab, &table.filter);
    match loaded {
        Ok((columns, rows)) => {
            if rows.is_empty() {
                ui.label("Empty state: there are no records yet. Load demo data or ingest a configured file.");
            } else {
                ui.label(format!(
                    "Success: loaded {} records from the service.",
                    rows.len()
                ));
            }
            table.render(ui, title, &columns, &rows);
            if tab == GuiTab::SavedQueries {
                if let Some(record) = table.selected_record(&rows) {
                    ui.horizontal(|ui| {
                        ui.label(format!("Selected query: {}", record.get(1).map(String::as_str).unwrap_or("")));
                        if ui.button("Copy query text").clicked() {
                            ui.ctx().copy_text(record.get(2).cloned().unwrap_or_default());
                        }
                    });
                }
            }
            if tab == GuiTab::Cases {
                if let Some(record) = table.selected_record(&rows) {
                    if let Some(case_id) = record.first().and_then(|value| value.parse::<i64>().ok()) {
                        let manager = logforge_core::cases::CaseManager::new(service.db());
                        if let Ok(Some(case)) = manager.get(case_id) {
                            ui.collapsing(format!("Investigation details · case #{}", case.id), |ui| {
                                ui.label(&case.description);
                                ui.label(format!("Assignee: {}", if case.assignee.is_empty() { "unassigned" } else { &case.assignee }));
                                if let Ok(notes) = manager.notes(case_id) {
                                    for note in notes {
                                        ui.label(format!("{} · {} · {}", note.created_at, note.author, note.body));
                                    }
                                }
                                if let Ok(alerts) = manager.alerts(case_id) {
                                    for alert in alerts {
                                        ui.label(format!("Linked alert #{} · {} · {}", alert.id, alert.severity, alert.title));
                                    }
                                }
                                let next_status = match case.status.as_str() {
                                    "open" => "in_progress",
                                    "in_progress" => "closed",
                                    _ => "open",
                                };
                                if ui.button(format!("Move case to {next_status}")).clicked() {
                                    match manager.set_status(case_id, next_status) {
                                        Ok(true) => {
                                            table.loading_frames = 1;
                                            ui.ctx().request_repaint_after(std::time::Duration::from_millis(30));
                                        }
                                        Ok(false) => {
                                            ui.colored_label(egui::Color32::from_rgb(179, 38, 30), "The selected case no longer exists.");
                                        }
                                        Err(error) => {
                                            ui.colored_label(egui::Color32::from_rgb(179, 38, 30), format!("Unable to update case: {error}"));
                                        }
                                    }
                                }
                            });
                        }
                    }
                }
            }
            if tab == GuiTab::Alerts {
                if let Some(record) = table.selected_record(&rows) {
                    if let (Some(id), Some(status)) = (
                        record.first().and_then(|value| value.parse::<i64>().ok()),
                        record.get(2).map(String::as_str),
                    ) {
                        let next_status = match status {
                            "open" => "acknowledged",
                            "acknowledged" => "resolved",
                            _ => "open",
                        };
                        if ui.button(format!("Mark selected alert {next_status}")).clicked() {
                            match service.update_alert_status(id, next_status) {
                                Ok(()) => {
                                    table.loading_frames = 1;
                                    ui.ctx().request_repaint_after(std::time::Duration::from_millis(30));
                                }
                                Err(error) => {
                                    ui.colored_label(egui::Color32::from_rgb(179, 38, 30), format!("Unable to update alert: {error}"));
                                }
                            }
                        }
                    }
                }
            }
            if tab == GuiTab::Detections {
                if let Some(record) = table.selected_record(&rows) {
                    let kind = record.first().map(String::as_str).unwrap_or("");
                    let id = record.get(1).and_then(|value| value.parse::<i64>().ok());
                    let enabled = record.get(5).and_then(|value| value.parse::<bool>().ok());
                    if let (Some(id), Some(enabled)) = (id, enabled) {
                        if ui.button(if enabled { "Disable selected detection" } else { "Enable selected detection" }).clicked() {
                            let result = if kind == "Pattern" {
                                service.set_rule_enabled(id, !enabled)
                            } else {
                                logforge_core::correlation_admin::set_enabled(service.db(), id, !enabled)
                            };
                            match result {
                                Ok(true) => {
                                    table.loading_frames = 1;
                                    ui.ctx().request_repaint_after(std::time::Duration::from_millis(30));
                                }
                                Ok(false) => {
                                    ui.colored_label(egui::Color32::from_rgb(179, 38, 30), "The selected detection no longer exists.");
                                }
                                Err(error) => {
                                    ui.colored_label(egui::Color32::from_rgb(179, 38, 30), format!("Unable to update detection: {error}"));
                                }
                            };
                        }
                    }
                }
            }
            if tab == GuiTab::Sources {
                if let Some(record) = table.selected_record(&rows) {
                    if let (Some(id), Some(enabled)) = (
                        record.first().and_then(|value| value.parse::<i64>().ok()),
                        record.get(4).and_then(|value| value.parse::<bool>().ok()),
                    ) {
                        if ui.button(if enabled { "Disable selected source" } else { "Enable selected source" }).clicked() {
                            match service.set_source_enabled(id, !enabled) {
                                Ok(true) => {
                                    table.loading_frames = 1;
                                    ui.ctx().request_repaint_after(std::time::Duration::from_millis(30));
                                }
                                Ok(false) => {
                                    ui.colored_label(egui::Color32::from_rgb(179, 38, 30), "The selected source no longer exists.");
                                }
                                Err(error) => {
                                    ui.colored_label(egui::Color32::from_rgb(179, 38, 30), format!("Unable to update source: {error}"));
                                }
                            };
                        }
                    }
                }
            }
        }
        Err(error) => {
            ui.colored_label(
                egui::Color32::from_rgb(179, 38, 30),
                format!("Error loading {title}: {error}"),
            );
            if ui.button("Retry").clicked() {
                table.loading_frames = 1;
                ui.ctx()
                    .request_repaint_after(std::time::Duration::from_millis(30));
            }
        }
    }
}

fn load(
    service: &SiemService,
    tab: GuiTab,
    filter_text: &str,
) -> Result<(Vec<&'static str>, Vec<Vec<String>>), String> {
    match tab {
        GuiTab::Search => {
            let filter = EventFilter {
                query: (!filter_text.trim().is_empty())
                    .then(|| filter_text.trim().to_string()),
                limit: 500,
                ..EventFilter::default()
            };
            let records = service
                .search_events_filtered(&filter)
                .map_err(|error| error.to_string())?;
            Ok((
                vec![
                    "ID",
                    "Timestamp UTC",
                    "Severity",
                    "Host",
                    "Service",
                    "Message",
                    "Redacted",
                    "Source",
                ],
                records
                    .into_iter()
                    .map(|r| {
                        vec![
                            r.id.to_string(),
                            r.timestamp_utc,
                            r.severity,
                            r.host,
                            r.service,
                            r.message,
                            r.redacted.to_string(),
                            r.source_id.to_string(),
                        ]
                    })
                    .collect(),
            ))
        }
        GuiTab::SavedQueries => Ok((
            vec!["ID", "Name", "Query", "Created UTC"],
            service
                .list_saved_queries()
                .map_err(|e| e.to_string())?
                .into_iter()
                .map(|r| vec![r.id.to_string(), r.name, r.query, r.created_at])
                .collect(),
        )),
        GuiTab::Sources => Ok((
            vec!["ID", "Name", "Type", "Location", "Enabled", "Created UTC"],
            service
                .list_sources()
                .map_err(|e| e.to_string())?
                .into_iter()
                .map(|r| {
                    vec![
                        r.id.to_string(),
                        r.name,
                        r.kind,
                        r.location,
                        r.enabled.to_string(),
                        r.created_at,
                    ]
                })
                .collect(),
        )),
        GuiTab::Schema => Ok((
            vec!["Column", "SQLite type", "Required", "Primary key"],
            service
                .event_schema()
                .map_err(|error| error.to_string())?
                .into_iter()
                .map(|field| {
                    vec![
                        field.name,
                        field.data_type,
                        field.required.to_string(),
                        field.primary_key.to_string(),
                    ]
                })
                .collect(),
        )),
        GuiTab::Detections => {
            let mut rows = service
                .list_rules()
                .map_err(|e| e.to_string())?
                .into_iter()
                .map(|r| {
                    vec![
                        "Pattern".into(),
                        r.id.to_string(),
                        r.name,
                        r.severity,
                        r.pattern,
                        r.enabled.to_string(),
                    ]
                })
                .collect::<Vec<_>>();
            rows.extend(
                logforge_core::correlation_admin::list_rules(service.db(), false)
                    .map_err(|e| e.to_string())?
                    .into_iter()
                    .map(|r| {
                        vec![
                            "Correlation".into(),
                            r.id.to_string(),
                            r.name,
                            r.severity,
                            format!(
                                "{}:{}:{}s:{}",
                                r.kind, r.entity_field, r.window_seconds, r.threshold
                            ),
                            r.enabled.to_string(),
                        ]
                    }),
            );
            Ok((
                vec!["Kind", "ID", "Name", "Severity", "Condition", "Enabled"],
                rows,
            ))
        }
        GuiTab::Alerts => Ok((
            vec![
                "ID",
                "Severity",
                "Status",
                "Title",
                "Rule",
                "Event",
                "Updated UTC",
            ],
            service
                .list_alerts(None)
                .map_err(|e| e.to_string())?
                .into_iter()
                .map(|r| {
                    vec![
                        r.id.to_string(),
                        r.severity,
                        r.status,
                        r.title,
                        r.rule_id.to_string(),
                        r.event_id.to_string(),
                        r.updated_at,
                    ]
                })
                .collect(),
        )),
        GuiTab::Cases => {
            let manager = logforge_core::cases::CaseManager::new(service.db());
            Ok((
                vec![
                    "ID",
                    "Status",
                    "Priority",
                    "Title",
                    "Assignee",
                    "Alerts",
                    "Updated UTC",
                ],
                manager
                    .list(None)
                    .map_err(|e| e.to_string())?
                    .into_iter()
                    .map(|r| {
                        vec![
                            r.id.to_string(),
                            r.status,
                            r.priority,
                            r.title,
                            if r.assignee.is_empty() {
                                "unassigned".into()
                            } else {
                                r.assignee
                            },
                            r.alert_count.to_string(),
                            r.updated_at,
                        ]
                    })
                    .collect(),
            ))
        }
        GuiTab::Audit => {
            let filter = logforge_core::audit_verify::AuditFilter {
                limit: 500,
                ..Default::default()
            };
            Ok((
                vec![
                    "ID",
                    "Created UTC",
                    "Actor",
                    "Action",
                    "Target",
                    "Previous hash",
                    "Entry hash",
                ],
                logforge_core::audit_verify::query_audit_log(service.db(), &filter)
                    .map_err(|e| e.to_string())?
                    .into_iter()
                    .map(|r| {
                        vec![
                            r.id.to_string(),
                            r.created_at,
                            r.actor,
                            r.action,
                            r.target,
                            r.previous_hash,
                            r.entry_hash,
                        ]
                    })
                    .collect(),
            ))
        }
        GuiTab::Health => Ok((
            vec![
                "Source",
                "Enabled",
                "Events",
                "Critical",
                "Errors",
                "Last event UTC",
                "Last received UTC",
            ],
            logforge_core::stats::source_health(service.db())
                .map_err(|e| e.to_string())?
                .into_iter()
                .map(|r| {
                    vec![
                        r.name,
                        r.enabled.to_string(),
                        r.total_events.to_string(),
                        r.critical_events.to_string(),
                        r.error_events.to_string(),
                        r.last_event_at.unwrap_or_else(|| "never".into()),
                        r.last_received_at.unwrap_or_else(|| "never".into()),
                    ]
                })
                .collect(),
        )),
        GuiTab::Retention => {
            let policy = logforge_core::retention::get_default_policy(service.db())
                .map_err(|e| e.to_string())?;
            let plan = logforge_core::retention::plan_retention_with_reasons(
                service.db(),
                policy.max_days,
                policy.max_events,
                policy.max_bytes,
                &chrono::Utc::now().to_rfc3339(),
            )
            .map_err(|e| e.to_string())?;
            let rows = plan
                .plan
                .event_ids
                .into_iter()
                .map(|id| {
                    vec![
                        id.to_string(),
                        "candidate".into(),
                        policy.name.clone(),
                        "dry run; no deletion".into(),
                    ]
                })
                .collect();
            Ok((vec!["Event ID", "Status", "Policy", "Action"], rows))
        }
        _ => Ok((
            vec!["Setting", "Value", "Source"],
            vec![vec![
                "Screen".into(),
                tab.title().into(),
                "LogForge-SIEM".into(),
            ]],
        )),
    }
}

impl GuiTab {
    fn title(self) -> &'static str {
        match self {
            Self::Overview => "Overview",
            Self::Search => "Search",
            Self::SavedQueries => "Saved Queries",
            Self::Sources => "Sources",
            Self::Schema => "Schema",
            Self::Detections => "Detections",
            Self::Alerts => "Alerts",
            Self::Cases => "Cases",
            Self::Retention => "Retention",
            Self::Audit => "Audit",
            Self::Health => "Health",
            Self::About => "About",
            Self::Settings => "Settings",
        }
    }
}
