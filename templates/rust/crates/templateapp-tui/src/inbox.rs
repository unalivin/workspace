use app_core::error::{AppError, Result};
use app_core::notify::{self, Filter, Notification, Rule};
use app_core::records::Query;
use app_core::service::NAV_RECORDS;
use app_core::theme::Severity;

use crate::model::{LIST_LIMIT, Model, Notice, Submit, move_cursor};
use crate::records::{borrowed, severity_options};
use crate::text::{Line, Look, window, wrapped};
use crate::widgets::{Field, Form, Key, layout, row, selectable};

#[derive(Default)]
pub(crate) struct InboxPage {
    pub rules: bool,
    pub unread_only: bool,
    pub list: Vec<Notification>,
    pub cursor: usize,
    pub rule_list: Vec<Rule>,
    pub rule_cursor: usize,
}

fn event_label(id: &str) -> &str {
    notify::event_type(id).map(|event| event.label).unwrap_or(id)
}

fn yes_no(value: bool) -> &'static str {
    if value { "Yes" } else { "No" }
}

impl Model<'_> {
    pub(crate) fn load_inbox(&mut self) {
        let previous = self.inbox.list.get(self.inbox.cursor).map(|item| item.id);
        let filter = Filter { unread_only: self.inbox.unread_only, limit: LIST_LIMIT, ..Filter::default() };
        let list = self.app.notifications().list(&filter);
        let Some(list) = self.fail(list) else {
            return;
        };
        let rules = self.app.notifications().rules();
        let Some(rules) = self.fail(rules) else {
            return;
        };
        let page = &mut self.inbox;
        page.list = list;
        page.rule_list = rules;
        if let Some(index) = page.list.iter().position(|item| Some(item.id) == previous) {
            page.cursor = index;
        }
        page.cursor = page.cursor.min(page.list.len().saturating_sub(1));
        page.rule_cursor = page.rule_cursor.min(page.rule_list.len().saturating_sub(1));
    }

    pub(crate) fn inbox_key(&mut self, key: Key) {
        match key {
            Key::Right | Key::Char(']') => self.inbox.rules = true,
            Key::Left | Key::Char('[') => self.inbox.rules = false,
            _ if self.inbox.rules => self.rule_key(key),
            _ => self.notification_key(key),
        }
    }

    fn notification_key(&mut self, key: Key) {
        let count = self.inbox.list.len();
        if move_cursor(&mut self.inbox.cursor, key, count) {
            return;
        }
        match key {
            Key::Char('u') => {
                self.inbox.unread_only = !self.inbox.unread_only;
                self.load_inbox();
            }
            Key::Char('M') => {
                let changed = self.app.notifications().mark_all_read();
                if let Some(changed) = self.fail(changed) {
                    self.changed(format!("Marked {changed} notifications as read"));
                }
            }
            Key::Char('m') | Key::Enter => {
                let Some(item) = self.inbox.list.get(self.inbox.cursor).cloned() else {
                    return;
                };
                if item.unread() {
                    let marked = self.app.notifications().mark_read(item.id);
                    if self.fail(marked).is_none() {
                        return;
                    }
                    self.changed(format!("Marked notification {} as read", item.id));
                }
                if key != Key::Enter {
                    return;
                }
                if item.record_id == 0 {
                    self.say(Notice::Info, "The record of this notification was removed");
                    return;
                }
                self.rec.query =
                    Query { sort: "updated".to_string(), descending: true, include_disabled: true, ..Query::default() };
                self.load_records();
                self.rec.select(item.record_id);
                self.rec.detail = true;
                self.open(NAV_RECORDS);
            }
            _ => {}
        }
    }

    fn rule_key(&mut self, key: Key) {
        let count = self.inbox.rule_list.len();
        if move_cursor(&mut self.inbox.rule_cursor, key, count) {
            return;
        }
        let Some(rule) = self.inbox.rule_list.get(self.inbox.rule_cursor).cloned() else {
            return;
        };
        match key {
            Key::Char('u') => {
                let reset = self.app.notifications().reset_rule(&rule.event_type);
                if self.fail(reset).is_some() {
                    self.changed(format!("Rule for {} is back to its defaults", rule.event_type));
                }
            }
            Key::Char('e') | Key::Enter => {
                let severities = severity_options();
                let fields = vec![
                    Field::yes_no("enabled", "Enabled", rule.enabled, "Off stops notifications of this event type."),
                    Field::choice(
                        "severity",
                        "Minimum severity",
                        rule.min_severity.as_str(),
                        "Events below this severity raise no notification.",
                        &borrowed(&severities),
                    ),
                    Field::text(
                        "start",
                        "Quiet hours start",
                        &rule.quiet_start,
                        "Time of day as HH:MM. Leave both times empty for no quiet hours.",
                        5,
                    ),
                    Field::text(
                        "end",
                        "Quiet hours end",
                        &rule.quiet_end,
                        "Time of day as HH:MM. May be earlier than the start to span midnight.",
                        5,
                    ),
                    Field::text(
                        "dedup",
                        "De-duplication",
                        &rule.dedup_minutes.to_string(),
                        "Minutes in which the same event for the same record is not raised again. 0 turns it off.",
                        5,
                    ),
                ];
                let title = format!("Rule for {}", event_label(&rule.event_type));
                self.form = Some((Form::new(title, fields), Submit::Rule(rule.event_type)));
            }
            _ => {}
        }
    }

    pub(crate) fn submit_rule(&mut self, form: &Form, event: &str) -> Result<()> {
        let minutes: i64 = form.value("dedup").trim().parse().map_err(|_| {
            AppError::Usage("de-duplication needs a whole number of minutes, for example 60".to_string())
        })?;
        let changed = Rule {
            event_type: event.to_string(),
            enabled: form.value("enabled") == "true",
            min_severity: Severity::parse(&form.value("severity")).unwrap_or(Severity::Info),
            quiet_start: form.value("start").trim().to_string(),
            quiet_end: form.value("end").trim().to_string(),
            dedup_minutes: minutes,
            customized: true,
        };
        self.app.notifications().set_rule(&changed)?;
        self.changed(format!("Saved the rule for {event}"));
        Ok(())
    }

    pub(crate) fn inbox_view(&self, size: usize, height: usize) -> Vec<Line> {
        let page = &self.inbox;
        let tab = |title: &str, active: bool| {
            if active {
                Line::styled(format!("▌ {title} "), Look::Selected)
            } else {
                Line::plain(format!("  {title}  "))
            }
        };
        let mut tabs = tab("Inbox", !page.rules).with(" ", Look::Plain);
        tabs.0.extend(tab("Rules", page.rules).0);
        let mut lines = vec![tabs, Line::new()];
        if page.rules {
            let widths = layout(&[(22, true), (7, false), (12, false), (11, false), (7, false), (7, false)], size.saturating_sub(2));
            let titles = ["EVENT", "ENABLED", "MIN SEVERITY", "QUIET HOURS", "DEDUP", "SOURCE"];
            lines.push(Line::styled(format!("  {}", row(&titles, &widths)), Look::Muted));
            for (index, rule) in page.rule_list.iter().enumerate() {
                let quiet = if rule.quiet_start.is_empty() {
                    "none".to_string()
                } else {
                    format!("{}-{}", rule.quiet_start, rule.quiet_end)
                };
                let cells = [
                    event_label(&rule.event_type).to_string(),
                    yes_no(rule.enabled).to_string(),
                    format!("{} {}", rule.min_severity.glyph(), rule.min_severity.label()),
                    quiet,
                    format!("{} min", rule.dedup_minutes),
                    if rule.customized { "changed" } else { "default" }.to_string(),
                ];
                lines.push(selectable(&row(&cells, &widths), index == page.rule_cursor, size));
            }
            if let Some(event) = page.rule_list.get(page.rule_cursor).and_then(|rule| notify::event_type(&rule.event_type)) {
                lines.push(Line::new());
                lines.extend(wrapped(event.description, size, Look::Plain));
            }
            return lines;
        }
        let filter = if page.unread_only { "unread only" } else { "all notifications" };
        lines.push(Line::styled(
            format!("Showing {filter} · {} unread", self.overview.unread_notifications),
            Look::Muted,
        ));
        lines.push(Line::new());
        if page.list.is_empty() {
            let message = if page.unread_only {
                "No unread notifications. Press u to show all of them."
            } else {
                "No notifications. They appear when a record needs attention or is completed."
            };
            lines.extend(wrapped(message, size, Look::Plain));
            return lines;
        }
        let widths = layout(&[(4, false), (16, false), (12, false), (6, false), (20, true), (10, false)], size.saturating_sub(2));
        let titles = ["ID", "WHEN", "SEVERITY", "STATE", "TITLE", "LINK"];
        lines.push(Line::styled(format!("  {}", row(&titles, &widths)), Look::Muted));
        let rows = height.saturating_sub(lines.len() + 1).max(3);
        let (start, end) = window(page.list.len(), page.cursor, rows);
        for (index, item) in page.list.iter().enumerate().take(end).skip(start) {
            let link = if item.record_id == 0 {
                "-".to_string()
            } else {
                format!("{} {}", item.record_kind, item.record_id)
            };
            let cells = [
                item.id.to_string(),
                self.app.format_time(item.created_at),
                format!("{} {}", item.severity.glyph(), item.severity.label()),
                if item.unread() { "Unread" } else { "Read" }.to_string(),
                item.title.clone(),
                link,
            ];
            let text = row(&cells, &widths);
            if index != page.cursor && item.unread() {
                lines.push(Line::plain(format!("● {text}")));
            } else {
                lines.push(selectable(&text, index == page.cursor, size));
            }
        }
        lines
    }
}
