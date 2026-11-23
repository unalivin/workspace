use eframe::egui;
use logforge_core::SiemService;

mod identity;
mod overview;
mod screens;
mod table;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuiTab {
    Overview,
    Search,
    SavedQueries,
    Sources,
    Schema,
    Detections,
    Alerts,
    Cases,
    Retention,
    Audit,
    Health,
    About,
    Settings,
}

pub struct LogForgeApp {
    service: SiemService,
    active_tab: GuiTab,
    search_query: String,
    search_host: String,
    last_data_version: i64,
    external_update_notice: bool,
    table: table::TableState,
    theme: String,
    accent: String,
    density: String,
    text_scale: u8,
    settings_error: Option<String>,
}

impl LogForgeApp {
    pub fn new(service: SiemService) -> Self {
        let last_data_version = service.db().data_version().unwrap_or(0);
        let theme = service
            .db()
            .get_setting("theme")
            .ok()
            .flatten()
            .unwrap_or_else(|| "dark".into());
        let accent = service
            .db()
            .get_setting("accent")
            .ok()
            .flatten()
            .unwrap_or_else(|| "amber".into());
        let density = service
            .db()
            .get_setting("density")
            .ok()
            .flatten()
            .unwrap_or_else(|| "compact".into());
        let text_scale = service
            .db()
            .get_setting("text_scale")
            .ok()
            .flatten()
            .and_then(|value| value.parse().ok())
            .filter(|value| logforge_core::config::validate_text_scale(*value))
            .unwrap_or(100);
        Self {
            service,
            active_tab: GuiTab::Overview,
            search_query: String::new(),
            search_host: String::new(),
            last_data_version,
            external_update_notice: false,
            table: table::TableState::default(),
            theme,
            accent,
            density,
            text_scale,
            settings_error: None,
        }
    }

    fn save_setting(&mut self, key: &str, value: &str) {
        self.settings_error = self
            .service
            .db()
            .set_setting(key, value, &chrono::Utc::now().to_rfc3339())
            .err()
            .map(|error| error.to_string());
    }
}

impl eframe::App for LogForgeApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        identity::apply_theme(ctx, &self.theme, &self.accent);
        ctx.set_zoom_factor(self.text_scale as f32 / 100.0);
        if let Ok(ver) = self.service.db().data_version() {
            if ver != self.last_data_version {
                self.last_data_version = ver;
                self.external_update_notice = true;
            }
        }

        egui::SidePanel::left("sidebar")
            .default_width(180.0)
            .show(ctx, |ui| {
                identity::draw_identity(ui, identity::accent_color(&self.accent));
                ui.heading("LogForge-SIEM");
                ui.separator();
                ui.selectable_value(&mut self.active_tab, GuiTab::Overview, "Overview");
                ui.selectable_value(&mut self.active_tab, GuiTab::Search, "Search");
                ui.selectable_value(&mut self.active_tab, GuiTab::SavedQueries, "Saved Queries");
                ui.selectable_value(&mut self.active_tab, GuiTab::Sources, "Sources");
                ui.selectable_value(&mut self.active_tab, GuiTab::Schema, "Schema");
                ui.selectable_value(&mut self.active_tab, GuiTab::Detections, "Detections");
                ui.selectable_value(&mut self.active_tab, GuiTab::Alerts, "Alerts");
                ui.selectable_value(&mut self.active_tab, GuiTab::Cases, "Cases");
                ui.selectable_value(&mut self.active_tab, GuiTab::Retention, "Retention");
                ui.selectable_value(&mut self.active_tab, GuiTab::Audit, "Audit");
                ui.selectable_value(&mut self.active_tab, GuiTab::Health, "Health");
                ui.separator();
                ui.selectable_value(&mut self.active_tab, GuiTab::About, "About");
                ui.selectable_value(&mut self.active_tab, GuiTab::Settings, "Settings");
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            if self.external_update_notice {
                ui.horizontal(|ui| {
                    ui.label("Updated by another window.");
                    if ui.button("Dismiss").clicked() {
                        self.external_update_notice = false;
                    }
                });
                ui.separator();
            }

            if matches!(
                self.active_tab,
                GuiTab::Search
                    | GuiTab::SavedQueries
                    | GuiTab::Sources
                    | GuiTab::Schema
                    | GuiTab::Detections
                    | GuiTab::Alerts
                    | GuiTab::Cases
                    | GuiTab::Retention
                    | GuiTab::Audit
                    | GuiTab::Health
            ) {
                screens::render(ui, &self.service, self.active_tab, &mut self.table);
                return;
            }

            match self.active_tab {
                GuiTab::Overview => {
                    ui.heading("Overview");
                    match self.service.overview_counts() {
                        Ok(c) => {
                            ui.horizontal_wrapped(|ui| {
                                overview::metric_card(ui, "Events", c.total_events.to_string());
                                overview::metric_card(ui, "Active sources", c.active_sources.to_string());
                                overview::metric_card(ui, "Open alerts", c.open_alerts.to_string());
                            });
                            ui.label(format!(
                                "Ingestion: received={} deduplicated={} dropped={} malformed={}",
                                c.stat_received, c.stat_deduplicated, c.stat_dropped, c.stat_malformed
                            ));
                            ui.separator();
                            overview::render_event_rate_chart(ui, &self.service);
                        }
                        Err(error) => {
                            ui.colored_label(egui::Color32::from_rgb(179, 38, 30), format!("Error loading overview: {error}"));
                        }
                    }
                }
                GuiTab::Search => {
                    ui.heading("Search (UTC)");
                    ui.horizontal(|ui| {
                        ui.label("Query:");
                        ui.text_edit_singleline(&mut self.search_query);
                        ui.label("Exact host:");
                        ui.text_edit_singleline(&mut self.search_host);
                    });
                    ui.separator();
                    let filter = logforge_core::event_query::EventFilter {
                        query: (!self.search_query.trim().is_empty()).then(|| self.search_query.clone()),
                        host: (!self.search_host.trim().is_empty()).then(|| self.search_host.clone()),
                        limit: 100,
                        ..logforge_core::event_query::EventFilter::default()
                    };
                    if let Ok(events) = self.service.search_events_filtered(&filter) {
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            for ev in events {
                                let badge = if ev.redacted { " [REDACTED]" } else { "" };
                                ui.label(format!(
                                    "{} [{}] {}: {}{}",
                                    ev.timestamp_utc, ev.severity, ev.host, ev.message, badge
                                ));
                            }
                        });
                    }
                }
                GuiTab::SavedQueries => {
                    ui.heading("Saved Queries");
                    if let Ok(queries) = self.service.list_saved_queries() {
                        for q in queries {
                            ui.label(format!("{}: {}", q.name, q.query));
                        }
                    }
                }
                GuiTab::Schema => {
                    ui.heading("Schema");
                    ui.label("Normalized fields: timestamp_utc, severity, host, service, message, dedup_hash, redacted");
                }
                GuiTab::Detections => {
                    ui.heading("Detection Rules");
                    if let Ok(rules) = self.service.list_rules() {
                        for r in rules {
                            ui.label(format!("{} [{}] pattern={}", r.name, r.severity, r.pattern));
                        }
                    }
                    ui.separator();
                    ui.heading("Persistent Correlation Rules");
                    for rule in logforge_core::correlation_admin::list_rules(self.service.db(), false).unwrap_or_default() {
                        ui.horizontal(|ui| {
                            let mut enabled = rule.enabled;
                            if ui.checkbox(&mut enabled, format!("#{} {}", rule.id, rule.name)).changed() {
                                let _ = logforge_core::correlation_admin::set_enabled(self.service.db(), rule.id, enabled);
                            }
                            ui.label(format!("[{}] {} on {} within {}s threshold={}", rule.severity, rule.kind, rule.entity_field, rule.window_seconds, rule.threshold));
                        });
                    }
                }
                GuiTab::Cases => {
                    ui.heading("Investigation Cases");
                    for case in logforge_core::cases::CaseManager::new(self.service.db()).list(None).unwrap_or_default() {
                        ui.group(|ui| {
                            ui.horizontal(|ui| {
                                ui.strong(format!("#{} {}", case.id, case.title));
                                ui.label(format!("{} · {} · {} alerts", case.status, case.priority, case.alert_count));
                            });
                            ui.label(format!("Assignee: {}", if case.assignee.is_empty() { "unassigned" } else { &case.assignee }));
                            ui.label(&case.description);
                            if let Ok(notes) = logforge_core::cases::CaseManager::new(self.service.db()).notes(case.id) {
                                for note in notes { ui.label(format!("{}: {}", note.author, note.body)); }
                            }
                        });
                    }
                }
                GuiTab::Retention => {
                    ui.heading("Retention");
                    if let Ok(p) = logforge_core::retention::get_default_policy(self.service.db()) {
                        ui.label(format!("Policy: {} ({} days, max {} events, {} bytes)", p.name, p.max_days, p.max_events, p.max_bytes));
                        if let Ok(breakdown) = logforge_core::retention::plan_retention_with_reasons(self.service.db(), p.max_days, p.max_events, p.max_bytes, &chrono::Utc::now().to_rfc3339()) {
                            ui.label(format!("Dry run: {} events and {} bytes would be removed", breakdown.plan.count, breakdown.plan.bytes_to_free));
                            ui.label(format!("Reasons: age={} · event limit={} · storage limit={}", breakdown.age_expired, breakdown.event_limit_exceeded, breakdown.byte_limit_exceeded));
                            egui::ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
                                for id in breakdown.plan.event_ids { ui.label(format!("Candidate event #{id}")); }
                            });
                        }
                    }
                    if let Ok(m) = logforge_core::storage_metrics::collect_storage_metrics(self.service.db()) {
                        ui.label(format!("Storage: {} bytes ({} events, {} alerts)", m.total_bytes, m.events_count, m.alerts_count));
                    }
                }
                GuiTab::Audit => {
                    ui.heading("Audit Log");
                    if let Ok(res) = logforge_core::audit_verify::verify_audit_chain(self.service.db()) {
                        ui.label(format!("Audit entries: {} | Chain valid: {}", res.total_entries, res.valid));
                    }
                    ui.separator();
                    ui.heading("Recent audit activity");
                    let filter = logforge_core::audit_verify::AuditFilter {
                        limit: 50,
                        ..logforge_core::audit_verify::AuditFilter::default()
                    };
                    if let Ok(entries) = logforge_core::audit_verify::query_audit_log(self.service.db(), &filter) {
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            for entry in entries {
                                ui.label(format!("{} · {} · {} · {}", entry.created_at, entry.actor, entry.action, entry.target));
                            }
                        });
                    }
                }
                GuiTab::Alerts => {
                    ui.heading("Alerts");
                    if let Ok(alerts) = self.service.list_alerts(None) {
                        for a in alerts {
                            ui.label(format!("#{} [{}] {} ({})", a.id, a.severity, a.title, a.status));
                        }
                    }
                }
                GuiTab::Sources => {
                    ui.heading("Sources");
                    if let Ok(sources) = self.service.list_sources() {
                        for s in sources {
                            ui.label(format!("{} ({}) - {}", s.name, s.kind, s.location));
                        }
                    }
                }
                GuiTab::Health => {
                    ui.heading("Operational Health");
                    ui.label("Source activity and ingestion quality");
                    if let Ok(liveness) = logforge_core::stats::source_liveness(
                        self.service.db(),
                        &chrono::Utc::now().to_rfc3339(),
                        300,
                    ) {
                        ui.label(format!(
                            "Freshness (<5m): {} active · {} stale · {} never seen · {} disabled",
                            liveness.active, liveness.stale, liveness.never_seen, liveness.disabled
                        ));
                    }
                    if let Ok(metrics) = logforge_core::stats::ingestion_stats(self.service.db()) {
                        ui.label(format!(
                            "Received {} · accepted {} · duplicate {:.1}% · malformed {:.1}% · dropped {}",
                            metrics.received, metrics.accepted, metrics.duplicate_rate_percent,
                            metrics.malformed_rate_percent, metrics.dropped
                        ));
                    }
                    if let Ok(queue) = logforge_core::cases::case_queue_metrics(
                        self.service.db(),
                        &chrono::Utc::now().to_rfc3339(),
                        24,
                    ) {
                        ui.label(format!(
                            "Cases: {} open · {} in progress · {} closed · {} overdue · {} unassigned",
                            queue.open_cases, queue.in_progress_cases, queue.closed_cases,
                            queue.overdue_active, queue.unassigned_active
                        ));
                    }
                    ui.separator();
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        if let Ok(sources) = logforge_core::stats::source_health(self.service.db()) {
                            for source in sources {
                                ui.group(|ui| {
                                    ui.horizontal(|ui| {
                                        ui.strong(&source.name);
                                        ui.label(if source.enabled { "enabled" } else { "disabled" });
                                        ui.label(format!("{} events", source.total_events));
                                    });
                                    ui.label(format!(
                                        "last event: {} · critical: {} · errors: {}",
                                        source.last_event_at.as_deref().unwrap_or("never"),
                                        source.critical_events, source.error_events
                                    ));
                                    ui.label(format!("last received: {}", source.last_received_at.as_deref().unwrap_or("never")));
                                });
                            }
                        }
                        ui.separator();
                        if let Ok(risk) = logforge_core::risk_scoring::score_host_risk(
                        self.service.db(),
                        "1970-01-01T00:00:00Z",
                        20,
                    ) {
                        ui.heading("Host risk (all retained events)");
                        for host in risk.hosts {
                            ui.label(format!("{} · {} ({}) · {} events · {} alerts", host.host, host.score, host.band, host.total_events, host.open_alerts));
                        }
                    }
                    if let Ok(alerts) = logforge_core::query_report::alert_counts_by_rule(self.service.db()) {
                            ui.heading("Alert volume by rule");
                            for rule in alerts {
                                ui.label(format!("{}: {} total, {} open", rule.rule_name, rule.total_alerts, rule.open_alerts));
                            }
                        }
                    });
                }
                GuiTab::About => {
                    ui.heading("About LogForge-SIEM");
                    ui.label("Version: 0.1.0-dev");
                    ui.label("Publisher: Sonix Devs");
                    ui.label("Repository: https://github.com/sonix-devs/logforge-siem");
                }
                GuiTab::Settings => {
                    ui.heading("Settings");
                    ui.label("Appearance is stored in this database and applies to the next frame.");
                    ui.horizontal(|ui| {
                        ui.label("Theme");
                        let previous = self.theme.clone();
                        egui::ComboBox::from_id_source("theme-mode")
                            .selected_text(&self.theme)
                            .show_ui(ui, |ui| {
                                for value in ["dark", "light", "system", "high-contrast"] {
                                    ui.selectable_value(&mut self.theme, value.to_string(), value);
                                }
                            });
                        if self.theme != previous {
                            self.save_setting("theme", &self.theme);
                            identity::apply_theme(ctx, &self.theme, &self.accent);
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label("Brand highlight");
                        let previous = self.accent.clone();
                        egui::ComboBox::from_id_source("brand-accent")
                            .selected_text(&self.accent)
                            .show_ui(ui, |ui| {
                                for value in ["amber", "blue", "teal"] {
                                    ui.selectable_value(&mut self.accent, value.to_string(), value);
                                }
                            });
                        if self.accent != previous {
                            self.save_setting("accent", &self.accent);
                            identity::apply_theme(ctx, &self.theme, &self.accent);
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label("Density");
                        let previous = self.density.clone();
                        egui::ComboBox::from_id_source("density")
                            .selected_text(&self.density)
                            .show_ui(ui, |ui| {
                                for value in ["compact", "comfortable"] {
                                    ui.selectable_value(&mut self.density, value.to_string(), value);
                                }
                            });
                        ui.label(if self.density == "compact" { "default" } else { "override" });
                        if self.density != previous && logforge_core::config::validate_density(&self.density) {
                            self.save_setting("density", &self.density);
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label("Text scale");
                        let previous = self.text_scale;
                        ui.add(egui::Slider::new(&mut self.text_scale, 80..=150).suffix("%"));
                        ui.label(if self.text_scale == 100 { "default" } else { "override" });
                        if self.text_scale != previous && logforge_core::config::validate_text_scale(self.text_scale) {
                            self.save_setting("text_scale", &self.text_scale.to_string());
                        }
                    });
                    ui.group(|ui| {
                        ui.heading("Network");
                        ui.label("Disabled by default. No remote destination is configured.");
                    });
                    ui.group(|ui| {
                        ui.heading("Notifications");
                        ui.label("Outbox delivery is local-only until a delivery adapter is explicitly configured.");
                    });
                    if self.accent != "amber" && ui.button("Reset to brand colours").clicked() {
                        self.accent = "amber".into();
                        self.save_setting("accent", &self.accent);
                        identity::apply_theme(ctx, &self.theme, &self.accent);
                    }
                    if let Some(error) = &self.settings_error {
                        ui.colored_label(egui::Color32::from_rgb(179, 38, 30), error);
                    }
                }
            }
        });
    }
}

pub fn run_gui(service: SiemService) -> eframe::Result<()> {
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "LogForge-SIEM",
        options,
        Box::new(|_cc| Ok(Box::new(LogForgeApp::new(service)))),
    )
}
