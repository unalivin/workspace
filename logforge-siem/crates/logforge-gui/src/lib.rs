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
                            ui.separator();
                            overview::render_overview_table(ui, &self.service, &mut self.table);
                        }
                        Err(error) => {
                            ui.colored_label(egui::Color32::from_rgb(179, 38, 30), format!("Error loading overview: {error}"));
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
                    ui.label("Appearance is stored in this database and applies to the next frame.");
                    ui.horizontal(|ui| {
                        ui.label("Theme");
                        let previous = self.theme.clone();
                        egui::ComboBox::from_id_salt("theme-mode")
                            .selected_text(&self.theme)
                            .show_ui(ui, |ui| {
                                for value in ["dark", "light", "system", "high-contrast"] {
                                    ui.selectable_value(&mut self.theme, value.to_string(), value);
                                }
                            });
                        if self.theme != previous {
                            let value = self.theme.clone();
                            self.save_setting("theme", &value);
                            identity::apply_theme(ctx, &self.theme, &self.accent);
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label("Brand highlight");
                        let previous = self.accent.clone();
                        egui::ComboBox::from_id_salt("brand-accent")
                            .selected_text(&self.accent)
                            .show_ui(ui, |ui| {
                                for value in ["amber", "blue", "teal"] {
                                    ui.selectable_value(&mut self.accent, value.to_string(), value);
                                }
                            });
                        if self.accent != previous {
                            let value = self.accent.clone();
                            self.save_setting("accent", &value);
                            identity::apply_theme(ctx, &self.theme, &self.accent);
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label("Density");
                        let previous = self.density.clone();
                        egui::ComboBox::from_id_salt("density")
                            .selected_text(&self.density)
                            .show_ui(ui, |ui| {
                                for value in ["compact", "comfortable"] {
                                    ui.selectable_value(&mut self.density, value.to_string(), value);
                                }
                            });
                        ui.label(if self.density == "compact" { "default" } else { "override" });
                        if self.density != previous && logforge_core::config::validate_density(&self.density) {
                            let value = self.density.clone();
                            self.save_setting("density", &value);
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
                        let value = self.accent.clone();
                        self.save_setting("accent", &value);
                        identity::apply_theme(ctx, &self.theme, &self.accent);
                    }
                    if let Some(error) = &self.settings_error {
                        ui.colored_label(egui::Color32::from_rgb(179, 38, 30), error);
                    }
                }
                _ => {}
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
