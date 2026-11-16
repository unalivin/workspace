use app_core::meta;
use app_core::records::DEMO_RECORDS;
use app_core::service::{NAV_ABOUT, NAV_NOTIFICATIONS, NAV_OVERVIEW, NAV_RECORDS, NAV_SETTINGS};
use egui::{
    Align, Button, CentralPanel, Context, Frame, Image, Key, Label, Layout, Margin, MenuBar, Modifiers, Panel, RichText,
    ScrollArea, TextEdit, Ui, vec2,
};

use crate::forms::{Verdict, muted, primary};
use crate::model::{AboutTab, Dashboard, SEARCH_LIMIT, Shortcut, Tone};
use crate::skin::{self, ICON_SCALE, alpha, color, severity_icon, status_icon, subheading};
use crate::table::{self, Cell, fixed, flexible};

const PAGE_KEYS: [Key; 5] = [Key::Num1, Key::Num2, Key::Num3, Key::Num4, Key::Num5];
const SIDEBAR_WIDTH: f32 = 196.0;
const FLASH_SECONDS: f32 = 0.6;
const SEARCH_HINT: &str = "Search records and the usage guide, then press Enter";
const DEMO_NOTE: &str =
    "Sample records are loaded. Remove demo data below deletes only the records whose names start with \"Demo:\".";

pub fn nav_icon(id: &str) -> &'static str {
    match id {
        NAV_OVERVIEW => "layout-dashboard",
        NAV_RECORDS => "list",
        NAV_NOTIFICATIONS => "bell",
        NAV_SETTINGS => "settings",
        NAV_ABOUT => "info",
        _ => "info",
    }
}

fn command() -> &'static str {
    if cfg!(target_os = "macos") { "Cmd" } else { "Ctrl" }
}

impl Dashboard<'_> {
    pub(crate) fn icon(&self, ctx: &Context, name: &str, tint: app_core::theme::Color) -> Image<'static> {
        self.icons.image(ctx, name, self.tokens.type_scale.body * ICON_SCALE, color(tint))
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        if std::mem::take(&mut self.restyled) {
            skin::apply(&ctx, &self.tokens);
            *ui.style_mut() = skin::style(&self.tokens);
        }
        self.keys(&ctx);
        if let Some(text) = self.clipboard.take() {
            ctx.copy_text(text);
        }
        self.menu_bar(ui);
        self.top_bar(ui);
        self.notice_bar(ui);
        self.sidebar(ui);
        let margin = Margin::same(self.tokens.spacing.l.round() as i8);
        let frame = Frame::new().fill(color(self.tokens.palette.background)).inner_margin(margin);
        CentralPanel::default().frame(frame).show(ui, |ui| match self.page_id() {
            NAV_RECORDS => self.records_page(ui),
            NAV_NOTIFICATIONS => self.inbox_page(ui),
            NAV_SETTINGS => self.settings_page(ui),
            NAV_ABOUT => self.about_page(ui),
            _ => self.overview_page(ui),
        });
        self.dialogs(&ctx);
    }

    fn keys(&mut self, ctx: &Context) {
        let mut pressed = Vec::new();
        let typing = ctx.text_edit_focused();
        let modal = self.form.is_some() || self.confirm.is_some();
        ctx.input_mut(|input| {
            for (index, key) in PAGE_KEYS.into_iter().enumerate() {
                if input.consume_key(Modifiers::COMMAND, key) {
                    pressed.push(Shortcut::Page(index));
                }
            }
            let bound = [
                (Key::F, Shortcut::Search),
                (Key::Comma, Shortcut::Settings),
                (Key::N, Shortcut::Add),
                (Key::R, Shortcut::Refresh),
                (Key::B, Shortcut::Sidebar),
                (Key::Q, Shortcut::Quit),
            ];
            for (key, shortcut) in bound {
                if input.consume_key(Modifiers::COMMAND, key) {
                    pressed.push(shortcut);
                }
            }
            if input.consume_key(Modifiers::NONE, Key::F1) {
                pressed.push(Shortcut::Guide);
            }
            if !typing && !modal {
                if input.key_pressed(Key::Delete) {
                    pressed.push(Shortcut::Remove);
                }
                if input.key_pressed(Key::Escape) {
                    pressed.push(Shortcut::Escape);
                }
            }
        });
        for shortcut in pressed {
            self.shortcut(shortcut);
        }
    }

    fn menu_bar(&mut self, ui: &mut Ui) {
        Panel::top("menu-bar").show(ui, |ui| {
            MenuBar::new().ui(ui, |ui| {
                ui.menu_button("Menu", |ui| {
                    if ui.button("Open terminal UI").clicked() {
                        self.open_terminal();
                        ui.close();
                    }
                    ui.separator();
                    if ui.add(Button::new("Quit").shortcut_text(format!("{}+Q", command()))).clicked() {
                        self.close("");
                        ui.close();
                    }
                });
                ui.menu_button("View", |ui| {
                    let nav = self.nav;
                    for (index, item) in nav.iter().enumerate() {
                        let entry = Button::new(item.label).shortcut_text(format!("{}+{}", command(), index + 1));
                        if ui.add(entry).clicked() {
                            self.page = index;
                            ui.close();
                        }
                    }
                    ui.separator();
                    let sidebar = Button::new("Collapse or expand the sidebar").shortcut_text(format!("{}+B", command()));
                    if ui.add(sidebar).clicked() {
                        self.toggle_sidebar();
                        ui.close();
                    }
                    if ui.add(Button::new("Refresh now").shortcut_text(format!("{}+R", command()))).clicked() {
                        self.refresh();
                        ui.close();
                    }
                });
                ui.menu_button("Help", |ui| {
                    let entries = [
                        ("Usage guide".to_string(), AboutTab::Guide),
                        ("Keyboard shortcuts".to_string(), AboutTab::Shortcuts),
                        ("Diagnostics".to_string(), AboutTab::Diagnostics),
                        (format!("About {}", meta::APP_NAME), AboutTab::Overview),
                    ];
                    for (label, tab) in entries {
                        if ui.button(label).clicked() {
                            self.show_about(tab);
                            ui.close();
                        }
                    }
                });
            });
        });
    }

    fn top_bar(&mut self, ui: &mut Ui) {
        let gaps = self.tokens.spacing;
        let frame = Frame::new()
            .fill(color(self.tokens.palette.surface))
            .inner_margin(Margin::symmetric(gaps.l.round() as i8, gaps.s.round() as i8));
        Panel::top("top-bar").frame(frame).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.heading(self.nav[self.page].label);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let help = Button::image_and_text(self.icon(ui.ctx(), "info", self.tokens.palette.text), "Help");
                    if ui.add(help).on_hover_text("Usage guide (F1)").clicked() {
                        self.show_about(AboutTab::Guide);
                    }
                    let unread = self.overview.unread_notifications;
                    let bell = self.icon(ui.ctx(), "bell", self.tokens.palette.text);
                    let bell = if unread > 0 {
                        Button::image_and_text(bell, unread.to_string())
                    } else {
                        Button::image(bell)
                    };
                    if ui.add(bell).on_hover_text(self.bell_label()).clicked() {
                        self.show(NAV_NOTIFICATIONS);
                    }
                    if self.overview.demo > 0 {
                        let chip =
                            Button::image_and_text(self.icon(ui.ctx(), "info", self.tokens.semantic.info), "Demo data");
                        if ui.add(chip).on_hover_text(DEMO_NOTE).clicked() {
                            self.show(NAV_SETTINGS);
                            self.say(Tone::Info, DEMO_NOTE);
                        }
                    }
                    let width = (ui.available_width() - gaps.l).clamp(120.0, 420.0);
                    let search = ui.add(
                        TextEdit::singleline(&mut self.search)
                            .hint_text(SEARCH_HINT)
                            .char_limit(SEARCH_LIMIT)
                            .desired_width(width),
                    );
                    if std::mem::take(&mut self.focus_search) {
                        search.request_focus();
                    }
                    if search.lost_focus() && ui.input(|input| input.key_pressed(Key::Enter)) {
                        self.search_everywhere();
                    }
                });
            });
        });
    }

    fn notice_bar(&mut self, ui: &mut Ui) {
        let Some((tone, text)) = self.notice.clone() else {
            return;
        };
        let gaps = self.tokens.spacing;
        let level = tone.severity();
        let tint = if tone == Tone::Success { self.tokens.semantic.success } else { self.tokens.semantic.severity(level) };
        let now = ui.input(|input| input.time);
        if self.flash != self.flash_seen {
            self.flash_seen = self.flash;
            self.flash_time = now;
        }
        let wash = self.notice_wash(now);
        if wash > 0 {
            ui.ctx().request_repaint();
        }
        let frame = Frame::new()
            .fill(color(self.tokens.palette.surface))
            .inner_margin(Margin::symmetric(gaps.l.round() as i8, gaps.s.round() as i8));
        Panel::bottom("notice-bar").frame(frame).show(ui, |ui| {
            if wash > 0 {
                ui.painter().rect_filled(ui.max_rect().expand(gaps.l), 0.0, alpha(self.tokens.palette.accent, wash));
            }
            ui.horizontal(|ui| {
                let name = if tone == Tone::Success { "check" } else { severity_icon(level) };
                ui.add(self.icon(ui.ctx(), name, tint));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let dismiss = Button::image(self.icon(ui.ctx(), "x", self.tokens.palette.text)).frame(false);
                    if ui.add(dismiss).on_hover_text("Dismiss this message").clicked() {
                        self.notice = None;
                    }
                    ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                        ui.add(Label::new(text).wrap());
                    });
                });
            });
        });
    }

    pub(crate) fn notice_wash(&self, now: f64) -> u8 {
        let age = (now - self.flash_time) as f32;
        if self.tokens.motion && (0.0..FLASH_SECONDS).contains(&age) {
            ((1.0 - age / FLASH_SECONDS) * 80.0) as u8
        } else {
            0
        }
    }

    fn nav_button(&mut self, ui: &mut Ui, index: usize) {
        let nav = self.nav;
        let item = &nav[index];
        let current = index == self.page;
        let tint = if current { self.tokens.palette.accent } else { self.tokens.palette.text };
        let image = self.icon(ui.ctx(), nav_icon(item.id), tint);
        let label = self.nav_label(item);
        let button = if self.collapsed {
            Button::image(image)
        } else {
            Button::image_and_text(image, label.clone())
        };
        let width = ui.available_width();
        let button = button.selected(current).min_size(vec2(width, self.tokens.spacing.row_height));
        let hint = if self.collapsed { format!("{label}: {}", item.hint) } else { item.hint.to_string() };
        if ui.add(button).on_hover_text(hint).clicked() {
            self.page = index;
        }
    }

    fn sidebar(&mut self, ui: &mut Ui) {
        let gaps = self.tokens.spacing;
        let width = if self.collapsed { self.tokens.type_scale.body * ICON_SCALE + gaps.m * 2.0 + gaps.s * 2.0 } else { SIDEBAR_WIDTH };
        let frame = Frame::new().fill(color(self.tokens.palette.surface)).inner_margin(Margin::same(gaps.s.round() as i8));
        Panel::left("sidebar").frame(frame).resizable(false).exact_size(width).show(ui, |ui| {
            let nav = self.nav;
            ui.with_layout(Layout::top_down_justified(Align::LEFT), |ui| {
                for (index, item) in nav.iter().enumerate() {
                    if !item.bottom {
                        self.nav_button(ui, index);
                    }
                }
            });
            ui.with_layout(Layout::bottom_up(Align::LEFT).with_cross_justify(true), |ui| {
                let image = self.icon(ui.ctx(), "sidebar", self.tokens.palette.text_muted);
                let hint = if self.collapsed { "Expand the sidebar" } else { "Collapse the sidebar to icons" };
                let toggle = if self.collapsed { Button::image(image) } else { Button::image_and_text(image, "Collapse") };
                if ui.add(toggle.frame(false)).on_hover_text(hint).clicked() {
                    self.toggle_sidebar();
                }
                ui.separator();
                for (index, item) in nav.iter().enumerate().rev() {
                    if item.bottom {
                        self.nav_button(ui, index);
                    }
                }
            });
        });
    }

    fn dialogs(&mut self, ctx: &Context) {
        if let Some(mut form) = self.form.take() {
            let verdict = form.show(ctx, &self.tokens);
            self.form = Some(form);
            match verdict {
                Verdict::Accepted => self.submit(),
                Verdict::Cancelled => self.cancel(),
                Verdict::Open => {}
            }
            return;
        }
        if let Some(confirm) = &self.confirm {
            match confirm.show(ctx, &self.tokens) {
                Verdict::Accepted => self.answer(true),
                Verdict::Cancelled => self.answer(false),
                Verdict::Open => {}
            }
        }
    }

    pub(crate) fn card(&self, ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
        Frame::new()
            .fill(color(self.tokens.palette.surface))
            .stroke((1.0, color(self.tokens.palette.divider)))
            .corner_radius(self.tokens.radius)
            .inner_margin(Margin::same(self.tokens.spacing.m.round() as i8))
            .show(ui, add);
    }

    pub(crate) fn section(&self, ui: &mut Ui, title: &str) {
        ui.add_space(self.tokens.spacing.s);
        ui.label(RichText::new(title).text_style(subheading()).strong());
        ui.add_space(self.tokens.spacing.xs);
    }

    fn overview_page(&mut self, ui: &mut Ui) {
        let mut load = false;
        ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            self.section(ui, "What needs attention");
            let overview = &self.overview;
            let cards = [
                ("Open records", overview.open, "not done and enabled"),
                ("Urgent records", overview.urgent, "open with high or critical severity"),
                ("Unread notifications", overview.unread_notifications, "see Notifications"),
                ("Disabled records", overview.disabled, "kept but not counted"),
            ];
            ui.columns(cards.len(), |columns| {
                for (column, (title, number, note)) in columns.iter_mut().zip(cards) {
                    self.card(column, |ui| {
                        ui.set_width(ui.available_width());
                        ui.label(title);
                        ui.label(RichText::new(number.to_string()).heading().strong());
                        muted(ui, note);
                    });
                }
            });
            self.section(ui, "Enabled records by status and severity");
            if overview.total() == 0 {
                ui.add(
                    Label::new(
                        "No records yet. Open Records and choose Add record, or load labeled demo data to look around.",
                    )
                    .wrap(),
                );
            } else {
                let gap = self.tokens.spacing.s;
                let width = ui.available_width().min(640.0);
                let slots = table::layout(&[flexible(1.0, 120.0), flexible(1.0, 120.0), fixed(80.0)], width - gap * 2.0, gap);
                table::header(ui, &self.tokens, &slots, &["Status", "Severity", "Records"]);
                for line in &overview.breakdown {
                    let cells = vec![
                        Cell::Icon(
                            status_icon(line.status),
                            color(self.tokens.palette.text_muted),
                            line.status.label().to_string(),
                        ),
                        Cell::Icon(
                            severity_icon(line.severity),
                            color(self.tokens.semantic.severity(line.severity)),
                            line.severity.label().to_string(),
                        ),
                        Cell::Text(line.total.to_string()),
                    ];
                    table::row(ui, &self.icons, &self.tokens, &slots, cells, false);
                }
            }
            ui.add_space(self.tokens.spacing.m);
            if overview.demo > 0 {
                let note = format!(
                    "Demo data is loaded: {} sample records whose names start with \"Demo:\". Remove them in Settings, Data, Remove demo data.",
                    overview.demo
                );
                ui.add(Label::new(RichText::new(note).weak()).wrap());
            } else {
                let image = self.icon(ui.ctx(), "download", self.tokens.palette.text);
                let label = format!("Load demo data ({} sample records)", DEMO_RECORDS.len());
                if ui.add(Button::image_and_text(image, label)).clicked() {
                    load = true;
                }
            }
        });
        if load {
            self.load_demo();
        }
    }

    pub(crate) fn primary_button(&self, ui: &mut Ui, label: &str) -> egui::Response {
        ui.add(primary(label, &self.tokens))
    }
}
