use std::collections::{BTreeMap, BTreeSet};

use app_core::guide;
use app_core::meta;
use app_core::service::{self, SHORTCUTS};
use app_core::settings::{self, Kind, Value};
use app_core::theme::{self, Color};
use app_core::userconfig::{FRONT_END_GUI, FRONT_END_TUI};
use egui::{Button, ComboBox, Key, Label, RichText, ScrollArea, Sense, TextEdit, Ui, vec2};

use crate::forms::{danger, muted};
use crate::model::{ABOUT_TABS, AboutTab, Dashboard, NOTIFIER_MISSING, NOTIFIER_OFF, NotifierState, front_end_label, saved_value};
use crate::skin::{color, subheading};
use crate::table::{self, Cell, fixed, flexible};

const CONTROL_WIDTH: f32 = 240.0;
const LABEL_WIDTH: f32 = 300.0;

#[derive(Debug, Clone, PartialEq, Eq)]
enum SettingAct {
    Save(String, String),
    Reset(String),
    Database,
    Demo,
    Notifier,
    Remember(String),
}

struct Edits {
    drafts: BTreeMap<String, String>,
    dirty: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum AboutAct {
    Tab(AboutTab),
    Source,
    Advisory,
    Bug,
    Filter,
    Section(Option<usize>),
    CopyDiagnostics,
    Check,
}

pub fn option_label<'a>(key: &str, option: &'a str) -> &'a str {
    if key == settings::KEY_THEME {
        let label = theme::label(option);
        if label.is_empty() { option } else { label }
    } else {
        option
    }
}

impl Dashboard<'_> {
    fn setting_row(&self, ui: &mut Ui, title: &str, help: &str, control: impl FnOnce(&mut Ui)) {
        ui.horizontal_top(|ui| {
            ui.allocate_ui(vec2(LABEL_WIDTH.min(ui.available_width() * 0.45), 0.0), |ui| {
                ui.vertical(|ui| {
                    ui.label(RichText::new(title).strong());
                    if !help.is_empty() {
                        ui.add(Label::new(RichText::new(help).small().weak()).wrap());
                    }
                });
            });
            ui.vertical(control);
        });
        ui.add_space(self.tokens.spacing.s);
    }

    fn setting_control(&self, ui: &mut Ui, value: &Value, edits: &mut Edits, act: &mut Option<SettingAct>) {
        let key = value.key.as_str();
        let saved = saved_value(value);
        ui.horizontal(|ui| {
            match value.definition.kind {
                Kind::Bool => {
                    let mut on = saved == "true";
                    if ui.checkbox(&mut on, if on { "On" } else { "Off" }).changed() {
                        *act = Some(SettingAct::Save(key.to_string(), on.to_string()));
                    }
                }
                Kind::Enum => {
                    let mut chosen = saved.to_string();
                    ComboBox::from_id_salt(("setting", key))
                        .selected_text(option_label(key, saved).to_string())
                        .width(CONTROL_WIDTH)
                        .show_ui(ui, |ui| {
                            for option in value.definition.options {
                                ui.selectable_value(&mut chosen, option.to_string(), option_label(key, option));
                            }
                        });
                    if chosen != saved {
                        *act = Some(SettingAct::Save(key.to_string(), chosen));
                    }
                }
                _ => {
                    let draft = edits.drafts.entry(key.to_string()).or_insert_with(|| saved.to_string());
                    if value.definition.kind == Kind::Color {
                        let shown = Color::parse(draft).unwrap_or(self.tokens.palette.accent);
                        let (swatch, _) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::hover());
                        ui.painter().rect_filled(swatch, self.tokens.radius, color(shown));
                        ui.painter().rect_stroke(
                            swatch,
                            self.tokens.radius,
                            (1.0, color(self.tokens.palette.border)),
                            egui::StrokeKind::Inside,
                        );
                    }
                    let hint = if value.definition.kind == Kind::Color { "#RRGGBB, empty for the theme accent" } else { "" };
                    let edit = ui.add(TextEdit::singleline(draft).hint_text(hint).char_limit(200).desired_width(CONTROL_WIDTH - 60.0));
                    let entered = edit.lost_focus() && ui.input(|input| input.key_pressed(Key::Enter));
                    let dirty = draft.as_str() != saved;
                    if edit.changed() {
                        if dirty {
                            edits.dirty.insert(key.to_string());
                        } else {
                            edits.dirty.remove(key);
                        }
                    }
                    if (ui.add_enabled(dirty, Button::new("Save")).clicked() || entered) && dirty {
                        *act = Some(SettingAct::Save(key.to_string(), draft.clone()));
                    }
                }
            }
            if ui.add_enabled(value.stored.is_some(), Button::new("Reset")).on_hover_text("Back to the default").clicked() {
                *act = Some(SettingAct::Reset(key.to_string()));
            }
        });
        let mut source = value.source.label().to_string();
        let note = self.setting_note(value);
        if !note.is_empty() {
            source = format!("{source}, {note}");
        }
        if value.definition.kind == Kind::Int {
            source = format!("{source}. Whole number from {} to {}", value.definition.min, value.definition.max);
        }
        ui.label(RichText::new(source).small().weak());
        if let Some(problem) = self.set.problems.get(key) {
            ui.colored_label(color(self.tokens.semantic.error), format!("Not saved: {problem}"));
        }
    }

    fn special_rows(&self, ui: &mut Ui, group: &str, act: &mut Option<SettingAct>) {
        match group {
            "Data" => {
                self.setting_row(ui, "Database location", &self.database_note(), |ui| {
                    ui.add(Label::new(RichText::new(self.database_path()).monospace()).wrap());
                    if ui.add_enabled(!self.database_locked(), Button::new("Change")).clicked() {
                        *act = Some(SettingAct::Database);
                    }
                });
                let help = "Removes the sample records whose names start with \"Demo:\" after confirmation. Your own records are not touched.";
                self.setting_row(ui, "Remove demo data", help, |ui| {
                    ui.label(format!("{} demo records", self.overview.demo));
                    if ui.add_enabled(self.overview.demo > 0, danger("Remove demo data", &self.tokens)).clicked() {
                        *act = Some(SettingAct::Demo);
                    }
                });
            }
            "Notifications" => {
                let (state, button) = match &self.set.notifier {
                    NotifierState::Missing => (NOTIFIER_MISSING.to_string(), None),
                    NotifierState::Unknown(problem) => (problem.clone(), None),
                    NotifierState::On(path) => (format!("On. Entry: {path}"), Some("Turn off")),
                    NotifierState::Off => (NOTIFIER_OFF.to_string(), Some("Turn on")),
                };
                let help = "The notifier shows system notifications while the application is closed.";
                self.setting_row(ui, "Start at sign-in", help, |ui| {
                    ui.add(Label::new(state).wrap());
                    if let Some(label) = button
                        && ui.button(label).clicked()
                    {
                        *act = Some(SettingAct::Notifier);
                    }
                });
            }
            "Launcher" => {
                let help = "Which front end opens when the application starts without a command.";
                self.setting_row(ui, "Open with", help, |ui| {
                    let current = self.front_end().to_string();
                    let mut chosen = current.clone();
                    ComboBox::from_id_salt("open-with")
                        .selected_text(front_end_label(&current))
                        .width(CONTROL_WIDTH)
                        .show_ui(ui, |ui| {
                            for mode in ["", FRONT_END_GUI, FRONT_END_TUI] {
                                ui.selectable_value(&mut chosen, mode.to_string(), front_end_label(mode));
                            }
                        });
                    if chosen != current {
                        *act = Some(SettingAct::Remember(chosen));
                    }
                    if let Some(note) = self.launcher_override() {
                        ui.label(RichText::new(note).small().weak());
                    }
                });
            }
            _ => {}
        }
    }

    pub(crate) fn settings_page(&mut self, ui: &mut Ui) {
        let mut act = None;
        let mut edits =
            Edits { drafts: std::mem::take(&mut self.set.drafts), dirty: std::mem::take(&mut self.set.dirty) };
        let board = &*self;
        ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            for group in &board.set.groups {
                board.section(ui, group);
                board.card(ui, |ui| {
                    ui.set_width(ui.available_width());
                    for value in board.set.list.iter().filter(|value| value.definition.group == group) {
                        let title = if value.key == settings::KEY_ACCENT { "Custom accent" } else { value.definition.label };
                        let mut help = value.definition.help.to_string();
                        if value.key == settings::KEY_ACCENT {
                            help.push_str(" The color is adjusted when needed to stay readable.");
                        }
                        board.setting_row(ui, title, &help, |ui| board.setting_control(ui, value, &mut edits, &mut act));
                    }
                    board.special_rows(ui, group, &mut act);
                });
            }
        });
        self.set.drafts = edits.drafts;
        self.set.dirty = edits.dirty;
        match act {
            Some(SettingAct::Save(key, raw)) => self.save_setting(&key, &raw),
            Some(SettingAct::Reset(key)) => self.reset_setting(&key),
            Some(SettingAct::Database) => self.change_database(),
            Some(SettingAct::Demo) => self.remove_demo(),
            Some(SettingAct::Notifier) => self.toggle_notifier(),
            Some(SettingAct::Remember(mode)) => self.remember(&mode),
            None => {}
        }
    }

    fn about_overview(&self, ui: &mut Ui, act: &mut Option<AboutAct>) {
        ui.horizontal(|ui| {
            ui.add(self.icons.logo(ui.ctx(), 64.0));
            ui.vertical(|ui| {
                ui.heading(format!("{} {}", meta::APP_NAME, meta::VERSION));
                ui.add(Label::new(meta::TAGLINE).wrap());
            });
        });
        ui.add_space(self.tokens.spacing.m);
        let source = service::source_link();
        if ui.button(source.label.as_str()).on_hover_text(source.url.as_str()).clicked() {
            *act = Some(AboutAct::Source);
        }
        if ui.button("Found a bug? Please report it on GitHub.").clicked() {
            *act = Some(AboutAct::Bug);
        }
        muted(ui, "The exact text of the report is shown before your browser opens.");
        let advisory = service::advisory_link();
        if ui.button("Report a vulnerability privately").on_hover_text(advisory.url.as_str()).clicked() {
            *act = Some(AboutAct::Advisory);
        }
        ui.add_space(self.tokens.spacing.m);
        ui.add(
            Label::new(
                RichText::new(
                    "The application makes no network requests. Links open in your browser only when you choose them.",
                )
                .weak(),
            )
            .wrap(),
        );
    }

    fn about_guide(&self, ui: &mut Ui, search: &mut String, act: &mut Option<AboutAct>) {
        let edited = ui.add(TextEdit::singleline(search).hint_text("Search the guide").char_limit(100).desired_width(320.0));
        if edited.changed() {
            *act = Some(AboutAct::Filter);
        }
        ui.add_space(self.tokens.spacing.s);
        let page = &self.about;
        if page.hits.is_empty() {
            ui.add(Label::new("No section mentions these words. Clear the search box to see the contents.").wrap());
            return;
        }
        ui.horizontal_top(|ui| {
            ui.allocate_ui(vec2(260.0_f32.min(ui.available_width() * 0.4), ui.available_height()), |ui| {
                ScrollArea::vertical().id_salt("guide-contents").auto_shrink(false).show(ui, |ui| {
                    for index in &page.hits {
                        let section = &page.usage.sections[*index];
                        let indent = "   ".repeat(section.level.saturating_sub(1).min(3));
                        let chosen = page.section == Some(*index);
                        if ui.selectable_label(chosen, format!("{indent}{}", section.title)).clicked() {
                            *act = Some(AboutAct::Section(Some(*index)));
                        }
                    }
                });
            });
            ui.separator();
            ScrollArea::vertical().id_salt("guide-section").auto_shrink(false).show(ui, |ui| {
                let shown = page.section.or_else(|| page.hits.first().copied());
                if let Some(section) = shown.and_then(|index| page.usage.sections.get(index)) {
                    ui.label(RichText::new(&section.title).text_style(subheading()).strong());
                    ui.add_space(self.tokens.spacing.xs);
                    ui.add(Label::new(guide::plain(&section.body)).wrap());
                }
            });
        });
    }

    fn about_shortcuts(&self, ui: &mut Ui) {
        let gap = self.tokens.spacing.s;
        let width = ui.available_width().min(720.0);
        let slots = table::layout(&[fixed(180.0), flexible(1.0, 200.0)], width - gap * 2.0, gap);
        table::header(ui, &self.tokens, &slots, &["Keys", "Action"]);
        ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            for shortcut in SHORTCUTS {
                let cells = vec![Cell::Text(shortcut.keys.to_string()), Cell::Muted(shortcut.action.to_string())];
                table::row(ui, &self.icons, &self.tokens, &slots, cells, false);
            }
        });
    }

    fn about_diagnostics(&self, ui: &mut Ui, act: &mut Option<AboutAct>) {
        ui.horizontal_wrapped(|ui| {
            let image = self.icon(ui.ctx(), "copy", self.tokens.palette.text);
            if ui.add(Button::image_and_text(image, "Copy diagnostics")).clicked() {
                *act = Some(AboutAct::CopyDiagnostics);
            }
            if ui.button("Check the database").clicked() {
                *act = Some(AboutAct::Check);
            }
        });
        muted(ui, "Diagnostics stay on this computer. A bug report includes only the lines shown in its preview.");
        ui.add_space(self.tokens.spacing.s);
        ScrollArea::both().auto_shrink(false).show(ui, |ui| {
            ui.add(Label::new(RichText::new(self.diagnostics_text()).monospace()).extend());
        });
    }

    pub(crate) fn about_page(&mut self, ui: &mut Ui) {
        let mut act = None;
        let mut search = std::mem::take(&mut self.about.search);
        let board = &*self;
        ui.horizontal_wrapped(|ui| {
            for (tab, label) in ABOUT_TABS {
                if ui.selectable_label(board.about.tab == *tab, *label).clicked() {
                    act = Some(AboutAct::Tab(*tab));
                }
            }
        });
        ui.separator();
        match board.about.tab {
            AboutTab::Overview => board.about_overview(ui, &mut act),
            AboutTab::Guide => board.about_guide(ui, &mut search, &mut act),
            AboutTab::Shortcuts => board.about_shortcuts(ui),
            AboutTab::Diagnostics => board.about_diagnostics(ui, &mut act),
            AboutTab::Licenses => {
                ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
                    ui.add(Label::new(RichText::new(meta::LICENSE_TEXT).monospace()).wrap());
                    ui.separator();
                    ui.add(Label::new(RichText::new(meta::NOTICES_TEXT).monospace()).wrap());
                });
            }
        }
        self.about.search = search;
        match act {
            Some(AboutAct::Tab(tab)) => self.about.tab = tab,
            Some(AboutAct::Source) => self.follow(&service::source_link()),
            Some(AboutAct::Advisory) => self.follow(&service::advisory_link()),
            Some(AboutAct::Bug) => self.report_bug(),
            Some(AboutAct::Filter) => self.filter_guide(),
            Some(AboutAct::Section(section)) => self.about.section = section,
            Some(AboutAct::CopyDiagnostics) => self.copy_diagnostics(),
            Some(AboutAct::Check) => self.check_database(),
            None => {}
        }
    }
}
