use app_core::records::{self, Record, Status};
use app_core::theme::Severity;
use egui::{Align, Button, ComboBox, Frame, Label, Layout, Margin, Panel, RichText, ScrollArea, TextEdit, Ui};

use crate::forms::{danger, muted};
use crate::model::{Dashboard, SEARCH_LIMIT, enabled_label, event_label};
use crate::skin::{color, severity_icon, status_icon, subheading};
use crate::table::{self, Cell, Spec, fixed, flexible};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RecordAct {
    Add,
    Import,
    Export,
    Columns,
    Select(i64),
    Close,
    Edit,
    Toggle,
    Copy,
    Remove,
    Clear,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum InboxAct {
    Tab(bool),
    Unread(bool),
    Select(i64),
    MarkRead,
    MarkAll,
    OpenRecord,
    Rule(String),
    EditNamed(String),
    EditRule,
    ResetRule,
}

fn column_spec(column: &str) -> Spec {
    match column {
        "id" => fixed(56.0),
        "name" => flexible(3.0, 140.0),
        "status" => fixed(124.0),
        "severity" => fixed(112.0),
        "updated" => flexible(1.0, 150.0),
        _ => fixed(96.0),
    }
}

pub fn sort_label(key: &str) -> String {
    match key {
        "updated" => "Sort by update".to_string(),
        other => format!("Sort by {other}"),
    }
}

impl Dashboard<'_> {
    fn record_cells(&self, record: &Record, columns: &[String]) -> Vec<Cell> {
        columns
            .iter()
            .map(|column| match column.as_str() {
                "id" => Cell::Muted(record.id.to_string()),
                "name" => Cell::Text(record.name.clone()),
                "status" => Cell::Icon(
                    status_icon(record.status),
                    color(self.tokens.palette.text_muted),
                    record.status.label().to_string(),
                ),
                "severity" => Cell::Icon(
                    severity_icon(record.severity),
                    color(self.tokens.semantic.severity(record.severity)),
                    record.severity.label().to_string(),
                ),
                "updated" => Cell::Muted(self.app.format_time(record.updated_at)),
                _ => Cell::Text(enabled_label(record.enabled).to_string()),
            })
            .collect()
    }

    fn record_toolbar(&self, ui: &mut Ui, query: &mut records::Query, act: &mut Option<RecordAct>) {
        ui.horizontal_wrapped(|ui| {
            ui.add(
                TextEdit::singleline(&mut query.search)
                    .hint_text("Search names and notes")
                    .char_limit(SEARCH_LIMIT)
                    .desired_width(220.0),
            );
            let status = query.status.map(Status::label).unwrap_or("All statuses");
            ComboBox::from_id_salt("record-status").selected_text(status).show_ui(ui, |ui| {
                ui.selectable_value(&mut query.status, None, "All statuses");
                for option in Status::ALL {
                    ui.selectable_value(&mut query.status, Some(option), option.label());
                }
            });
            let severity = query.severity.map(Severity::label).unwrap_or("All severities");
            ComboBox::from_id_salt("record-severity").selected_text(severity).show_ui(ui, |ui| {
                ui.selectable_value(&mut query.severity, None, "All severities");
                for option in Severity::ALL {
                    ui.selectable_value(&mut query.severity, Some(option), option.label());
                }
            });
            ComboBox::from_id_salt("record-sort").selected_text(sort_label(&query.sort)).show_ui(ui, |ui| {
                for key in records::SORT_KEYS {
                    ui.selectable_value(&mut query.sort, key.to_string(), sort_label(key));
                }
            });
            let direction = if query.descending { "Descending" } else { "Ascending" };
            if ui.button(direction).on_hover_text("Reverse the sort order").clicked() {
                query.descending = !query.descending;
            }
            ui.checkbox(&mut query.include_disabled, "Show disabled");
        });
        ui.add_space(self.tokens.spacing.xs);
        ui.horizontal_wrapped(|ui| {
            let plus = self.icons.image(ui.ctx(), "plus", self.tokens.type_scale.body, color(self.tokens.on_accent));
            let add = Button::image_and_text(plus, RichText::new("Add record").color(color(self.tokens.on_accent)))
                .fill(color(self.tokens.palette.accent));
            if ui.add(add).clicked() {
                *act = Some(RecordAct::Add);
            }
            let entries = [
                ("upload", "Import", RecordAct::Import),
                ("download", "Export", RecordAct::Export),
                ("columns", "Columns", RecordAct::Columns),
            ];
            for (icon, label, wanted) in entries {
                let image = self.icon(ui.ctx(), icon, self.tokens.palette.text);
                if ui.add(Button::image_and_text(image, label)).clicked() {
                    *act = Some(wanted);
                }
            }
            if self.filtered() && ui.button("Clear filters").clicked() {
                *act = Some(RecordAct::Clear);
            }
        });
        ui.add_space(self.tokens.spacing.s);
    }

    fn record_detail(&self, ui: &mut Ui, record: &Record, act: &mut Option<RecordAct>) {
        ui.horizontal(|ui| {
            ui.add(Label::new(RichText::new(&record.name).text_style(subheading()).strong()).wrap());
        });
        ui.add_space(self.tokens.spacing.xs);
        ui.label(self.record_details(record));
        ui.add_space(self.tokens.spacing.s);
        if record.note.is_empty() {
            muted(ui, "No note.");
        } else {
            ScrollArea::vertical().id_salt("record-note").max_height(160.0).show(ui, |ui| {
                ui.add(Label::new(&record.note).wrap());
            });
        }
        if record.demo {
            ui.add_space(self.tokens.spacing.xs);
            muted(ui, "Demo record");
        }
        ui.add_space(self.tokens.spacing.m);
        ui.horizontal_wrapped(|ui| {
            let entries = [
                ("pencil", "Edit", RecordAct::Edit),
                ("circle-minus", if record.enabled { "Disable" } else { "Enable" }, RecordAct::Toggle),
                ("copy", "Copy", RecordAct::Copy),
            ];
            for (icon, label, wanted) in entries {
                let image = self.icon(ui.ctx(), icon, self.tokens.palette.text);
                if ui.add(Button::image_and_text(image, label)).clicked() {
                    *act = Some(wanted);
                }
            }
            if ui.add(danger("Remove", &self.tokens)).clicked() {
                *act = Some(RecordAct::Remove);
            }
            if ui.button("Close").clicked() {
                *act = Some(RecordAct::Close);
            }
        });
    }

    pub(crate) fn records_page(&mut self, ui: &mut Ui) {
        let mut query = self.rec.query.clone();
        let mut act = None;
        let board = &*self;
        board.record_toolbar(ui, &mut query, &mut act);
        if let Some(record) = board.selected_record() {
            let frame = Frame::new()
                .fill(color(board.tokens.palette.surface))
                .stroke((1.0, color(board.tokens.palette.divider)))
                .corner_radius(board.tokens.radius)
                .inner_margin(Margin::same(board.tokens.spacing.m.round() as i8));
            Panel::right("record-detail").frame(frame).resizable(false).exact_size(320.0).show(ui, |ui| {
                board.record_detail(ui, record, &mut act);
            });
        }
        if board.rec.list.is_empty() {
            ui.add(Label::new(board.empty_records_text()).wrap());
        } else {
            let gap = board.tokens.spacing.s;
            let columns = &board.rec.columns;
            let specs: Vec<Spec> = columns.iter().map(|column| column_spec(column)).collect();
            let titles: Vec<&str> = columns.iter().map(|column| records::column_label(column)).collect();
            ui.with_layout(Layout::bottom_up(Align::LEFT), |ui| {
                muted(ui, format!("{} records shown", board.rec.list.len()));
                ui.with_layout(Layout::top_down(Align::LEFT), |ui| {
                    let slots = table::layout(&specs, ui.available_width() - gap * 2.0, gap);
                    table::header(ui, &board.tokens, &slots, &titles);
                    let height = board.tokens.spacing.row_height;
                    ScrollArea::vertical().id_salt("record-rows").auto_shrink(false).show_rows(
                        ui,
                        height,
                        board.rec.list.len(),
                        |ui, range| {
                            for record in &board.rec.list[range] {
                                let cells = board.record_cells(record, columns);
                                let selected = board.rec.selected == Some(record.id);
                                if table::row(ui, &board.icons, &board.tokens, &slots, cells, selected).clicked() {
                                    act = Some(if selected { RecordAct::Close } else { RecordAct::Select(record.id) });
                                }
                            }
                        },
                    );
                });
            });
        }
        if query.sort != self.rec.query.sort || query.descending != self.rec.query.descending {
            let (key, descending) = (query.sort.clone(), query.descending);
            self.rec.query = query;
            self.sort_by(&key, descending);
        } else if query != self.rec.query {
            self.rec.query = query;
            self.load_records();
        }
        match act {
            Some(RecordAct::Add) => self.add_record(),
            Some(RecordAct::Import) => self.import_records(),
            Some(RecordAct::Export) => self.export_records(),
            Some(RecordAct::Columns) => self.choose_columns(),
            Some(RecordAct::Select(id)) => self.rec.selected = Some(id),
            Some(RecordAct::Close) => self.rec.selected = None,
            Some(RecordAct::Edit) => self.edit_record(),
            Some(RecordAct::Toggle) => self.toggle_record(),
            Some(RecordAct::Copy) => self.copy_record(),
            Some(RecordAct::Remove) => self.remove_record(),
            Some(RecordAct::Clear) => self.clear_filters(),
            None => {}
        }
    }

    fn inbox_list(&self, ui: &mut Ui, act: &mut Option<InboxAct>) {
        ui.horizontal_wrapped(|ui| {
            let mut unread = self.inbox.unread_only;
            if ui.checkbox(&mut unread, "Unread only").changed() {
                *act = Some(InboxAct::Unread(unread));
            }
            let entries = [
                ("check", "Mark as read", InboxAct::MarkRead),
                ("check", "Mark all as read", InboxAct::MarkAll),
                ("arrow-right", "Open record", InboxAct::OpenRecord),
            ];
            for (icon, label, wanted) in entries {
                let image = self.icon(ui.ctx(), icon, self.tokens.palette.text);
                if ui.add(Button::image_and_text(image, label)).clicked() {
                    *act = Some(wanted);
                }
            }
        });
        ui.add_space(self.tokens.spacing.s);
        if self.inbox.list.is_empty() {
            ui.add(Label::new(self.empty_inbox_text()).wrap());
            return;
        }
        let gap = self.tokens.spacing.s;
        ui.with_layout(Layout::bottom_up(Align::LEFT), |ui| {
            muted(ui, self.inbox_summary());
            ui.with_layout(Layout::top_down(Align::LEFT), |ui| {
                let specs = [fixed(112.0), flexible(3.0, 160.0), flexible(1.0, 130.0), flexible(1.0, 150.0), fixed(64.0)];
                let slots = table::layout(&specs, ui.available_width() - gap * 2.0, gap);
                table::header(ui, &self.tokens, &slots, &["Severity", "Notification", "Event", "Raised", "State"]);
                let height = self.tokens.spacing.row_height;
                ScrollArea::vertical().id_salt("inbox-rows").auto_shrink(false).show_rows(
                    ui,
                    height,
                    self.inbox.list.len(),
                    |ui, range| {
                        for item in &self.inbox.list[range] {
                            let cells = vec![
                                Cell::Icon(
                                    severity_icon(item.severity),
                                    color(self.tokens.semantic.severity(item.severity)),
                                    item.severity.label().to_string(),
                                ),
                                Cell::Text(item.title.clone()),
                                Cell::Muted(event_label(&item.event_type).to_string()),
                                Cell::Muted(self.app.format_time(item.created_at)),
                                if item.unread() { Cell::Text("Unread".to_string()) } else { Cell::Muted("Read".to_string()) },
                            ];
                            let selected = self.inbox.selected == Some(item.id);
                            let line = table::row(ui, &self.icons, &self.tokens, &slots, cells, selected);
                            if line.double_clicked() {
                                *act = Some(InboxAct::OpenRecord);
                            } else if line.clicked() {
                                *act = Some(InboxAct::Select(item.id));
                            }
                        }
                    },
                );
            });
        });
    }

    fn inbox_rules(&self, ui: &mut Ui, act: &mut Option<InboxAct>) {
        ui.horizontal_wrapped(|ui| {
            let image = self.icon(ui.ctx(), "pencil", self.tokens.palette.text);
            if ui.add(Button::image_and_text(image, "Edit rule")).clicked() {
                *act = Some(InboxAct::EditRule);
            }
            if ui.button("Reset to defaults").clicked() {
                *act = Some(InboxAct::ResetRule);
            }
        });
        muted(ui, "Select an event type to change its rule.");
        ui.add_space(self.tokens.spacing.s);
        let gap = self.tokens.spacing.s;
        let specs = [flexible(2.0, 180.0), fixed(80.0), fixed(130.0), fixed(130.0), fixed(120.0), fixed(90.0)];
        let slots = table::layout(&specs, ui.available_width() - gap * 2.0, gap);
        let titles = ["Event type", "Enabled", "Minimum severity", "Quiet hours", "De-duplication", "Source"];
        table::header(ui, &self.tokens, &slots, &titles);
        ScrollArea::vertical().id_salt("rule-rows").auto_shrink(false).show(ui, |ui| {
            for rule in &self.inbox.rule_list {
                let quiet = if rule.quiet_start.is_empty() && rule.quiet_end.is_empty() {
                    "None".to_string()
                } else {
                    format!("{} to {}", rule.quiet_start, rule.quiet_end)
                };
                let dedup = if rule.dedup_minutes == 0 { "Off".to_string() } else { format!("{} min", rule.dedup_minutes) };
                let cells = vec![
                    Cell::Text(event_label(&rule.event_type).to_string()),
                    Cell::Text(if rule.enabled { "Yes" } else { "No" }.to_string()),
                    Cell::Icon(
                        severity_icon(rule.min_severity),
                        color(self.tokens.semantic.severity(rule.min_severity)),
                        rule.min_severity.label().to_string(),
                    ),
                    Cell::Muted(quiet),
                    Cell::Muted(dedup),
                    Cell::Muted(if rule.customized { "Saved" } else { "Default" }.to_string()),
                ];
                let selected = self.inbox.rule.as_deref() == Some(rule.event_type.as_str());
                let line = table::row(ui, &self.icons, &self.tokens, &slots, cells, selected);
                if line.double_clicked() {
                    *act = Some(InboxAct::EditNamed(rule.event_type.clone()));
                } else if line.clicked() {
                    *act = Some(InboxAct::Rule(rule.event_type.clone()));
                }
            }
        });
    }

    pub(crate) fn inbox_page(&mut self, ui: &mut Ui) {
        let mut act = None;
        let board = &*self;
        ui.horizontal(|ui| {
            for (rules, label) in [(false, "Inbox"), (true, "Rules")] {
                if ui.selectable_label(board.inbox.rules == rules, label).clicked() {
                    act = Some(InboxAct::Tab(rules));
                }
            }
        });
        ui.separator();
        if board.inbox.rules {
            board.inbox_rules(ui, &mut act);
        } else {
            board.inbox_list(ui, &mut act);
        }
        match act {
            Some(InboxAct::Tab(rules)) => self.inbox.rules = rules,
            Some(InboxAct::Unread(only)) => {
                self.inbox.unread_only = only;
                self.load_inbox();
            }
            Some(InboxAct::Select(id)) => self.inbox.selected = Some(id),
            Some(InboxAct::MarkRead) => self.mark_read(),
            Some(InboxAct::MarkAll) => self.mark_all_read(),
            Some(InboxAct::OpenRecord) => self.open_notification_record(),
            Some(InboxAct::Rule(event)) => self.inbox.rule = Some(event),
            Some(InboxAct::EditNamed(event)) => {
                self.inbox.rule = Some(event);
                self.edit_rule();
            }
            Some(InboxAct::EditRule) => self.edit_rule(),
            Some(InboxAct::ResetRule) => self.reset_rule(),
            None => {}
        }
    }
}
