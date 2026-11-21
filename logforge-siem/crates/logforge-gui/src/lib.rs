use eframe::egui;
use logforge-core::SiemService;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuiTab {
    Overview,
    Search,
    SavedQueries,
    Sources,
    Schema,
    Detections,
    Alerts,
    Retention,
    Audit,
    About,
    Settings,
}

pub struct LogForgeApp {
    service: SiemService,
    active_tab: GuiTab,
    search_query: String,
    last_data_version: i64,
    external_update_notice: bool,
}

impl LogForgeApp {
    pub fn new(service: SiemService) -> Self {
        let last_data_version = service.db().data_version().unwrap_or(0);
        Self {
            service,
            active_tab: GuiTab::Overview,
            search_query: String::new(),
            last_data_version,
            external_update_notice: false,
        }
    }
}

impl eframe::App for LogForgeApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if let Ok(ver) = self.service.db().data_version() {
            if ver != self.last_data_version {
                self.last_data_version = ver;
                self.external_update_notice = true;
            }
        }

        egui::SidePanel::left("sidebar")
            .default_width(180.0)
            .show(ctx, |ui| {
                ui.heading("LogForge-SIEM");
                ui.separator();
                ui.selectable_value(&mut self.active_tab, GuiTab::Overview, "Overview");
                ui.selectable_value(&mut self.active_tab, GuiTab::Search, "Search");
                ui.selectable_value(&mut self.active_tab, GuiTab::SavedQueries, "Saved Queries");
                ui.selectable_value(&mut self.active_tab, GuiTab::Sources, "Sources");
                ui.selectable_value(&mut self.active_tab, GuiTab::Schema, "Schema");
                ui.selectable_value(&mut self.active_tab, GuiTab::Detections, "Detections");
                ui.selectable_value(&mut self.active_tab, GuiTab::Alerts, "Alerts");
                ui.selectable_value(&mut self.active_tab, GuiTab::Retention, "Retention");
                ui.selectable_value(&mut self.active_tab, GuiTab::Audit, "Audit");
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

            match self.active_tab {
                GuiTab::Overview => {
                    ui.heading("Overview");
                    if let Ok(c) = self.service.overview_counts() {
                        ui.label(format!("Total events: {}", c.total_events));
                        ui.label(format!("Active sources: {}", c.active_sources));
                        ui.label(format!("Open alerts: {}", c.open_alerts));
                        ui.label(format!(
                            "Ingestion: received={} deduplicated={} dropped={} malformed={}",
                            c.stat_received, c.stat_deduplicated, c.stat_dropped, c.stat_malformed
                        ));
                    }
                }
                GuiTab::Search => {
                    ui.heading("Search (UTC)");
                    ui.horizontal(|ui| {
                        ui.label("Query:");
                        ui.text_edit_singleline(&mut self.search_query);
                    });
                    ui.separator();
                    if let Ok(events) = self.service.search_events(&self.search_query, 100) {
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
                }
                GuiTab::Retention => {
                    ui.heading("Retention");
                    if let Ok(p) = logforge_core::retention::get_default_policy(self.service.db()) {
                        ui.label(format!("Policy: {} ({} days, max {} events)", p.name, p.max_days, p.max_events));
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
                GuiTab::About => {
                    ui.heading("About LogForge-SIEM");
                    ui.label("Version: 0.1.0-dev");
                    ui.label("Publisher: Sonix Devs");
                    ui.label("Repository: https://github.com/sonix-devs/logforge-siem");
                }
                GuiTab::Settings => {
                    ui.heading("Settings");
                    ui.label("Theme: Dark | Accent: Amber | Density: Compact");
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
