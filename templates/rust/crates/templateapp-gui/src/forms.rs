use app_core::service::{Link, MovePlan};
use app_core::theme::{self, Tokens};
use egui::{Button, ComboBox, Context, Id, Key, Modal, RichText, ScrollArea, TextEdit, Ui};

use crate::skin::{color, strong};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    RemoveRecord(i64),
    RemoveDemo,
    LoadDemo,
    InstallNotifier,
    UninstallNotifier,
    MoveDatabase(MovePlan),
    Import { path: String, format: String },
    OpenLink(Link),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Submit {
    AddRecord,
    EditRecord { id: i64, revision: i64 },
    Columns,
    Import,
    Export,
    Rule(String),
    MoveDatabase,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Open,
    Accepted,
    Cancelled,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Field {
    pub key: String,
    pub label: String,
    pub value: String,
    pub help: String,
    pub options: Vec<(String, String)>,
    pub multiline: bool,
    pub limit: usize,
}

impl Field {
    pub fn text(key: &str, label: &str, value: &str, help: &str, limit: usize) -> Field {
        Field {
            key: key.to_string(),
            label: label.to_string(),
            value: value.to_string(),
            help: help.to_string(),
            limit,
            ..Field::default()
        }
    }

    pub fn area(key: &str, label: &str, value: &str, help: &str, limit: usize) -> Field {
        Field { multiline: true, ..Field::text(key, label, value, help, limit) }
    }

    pub fn choice(key: &str, label: &str, value: &str, help: &str, options: &[(&str, &str)]) -> Field {
        Field {
            options: options.iter().map(|(value, label)| (value.to_string(), label.to_string())).collect(),
            ..Field::text(key, label, value, help, 0)
        }
    }

    pub fn yes_no(key: &str, label: &str, value: bool, help: &str) -> Field {
        Field::choice(key, label, if value { "true" } else { "false" }, help, &[("true", "Yes"), ("false", "No")])
    }

    pub fn shown(&self) -> &str {
        self.options.iter().find(|(value, _)| *value == self.value).map(|(_, label)| label.as_str()).unwrap_or("")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Form {
    pub title: String,
    pub accept: String,
    pub fields: Vec<Field>,
    pub problem: String,
    pub submit: Submit,
    pub fresh: bool,
}

impl Form {
    pub fn new(title: impl Into<String>, accept: &str, fields: Vec<Field>, submit: Submit) -> Form {
        Form { title: title.into(), accept: accept.to_string(), fields, problem: String::new(), submit, fresh: true }
    }

    pub fn value(&self, key: &str) -> String {
        self.fields.iter().find(|field| field.key == key).map(|field| field.value.clone()).unwrap_or_default()
    }

    pub fn set(&mut self, key: &str, value: &str) {
        if let Some(field) = self.fields.iter_mut().find(|field| field.key == key) {
            field.value = value.to_string();
        }
    }

    pub fn show(&mut self, ctx: &Context, tokens: &Tokens) -> Verdict {
        let mut verdict = Verdict::Open;
        let shown = Modal::new(Id::unique("form-dialog")).show(ctx, |ui| {
            ui.set_width(460.0_f32.min(ui.ctx().content_rect().width() - 48.0));
            ui.heading(&self.title);
            ui.add_space(tokens.spacing.s);
            let fresh = std::mem::take(&mut self.fresh);
            ScrollArea::vertical().max_height(ui.ctx().content_rect().height() - 220.0).show(ui, |ui| {
                for (index, field) in self.fields.iter_mut().enumerate() {
                    let name = ui.label(strong(&field.label));
                    if !field.options.is_empty() {
                        let chosen = field.shown().to_string();
                        let picker = ComboBox::from_id_salt(("form-field", index))
                            .selected_text(chosen)
                            .width(ui.available_width().min(420.0))
                            .show_ui(ui, |ui| {
                                for (value, label) in &field.options {
                                    ui.selectable_value(&mut field.value, value.clone(), label.as_str());
                                }
                            });
                        picker.response.labelled_by(name.id);
                    } else {
                        let editor = if field.multiline {
                            TextEdit::multiline(&mut field.value).desired_rows(4)
                        } else {
                            TextEdit::singleline(&mut field.value)
                        };
                        let limit = if field.limit == 0 { usize::MAX } else { field.limit };
                        let answer = ui.add(editor.char_limit(limit).desired_width(f32::INFINITY)).labelled_by(name.id);
                        if fresh && index == 0 {
                            answer.request_focus();
                        }
                        if !field.multiline && answer.lost_focus() && ui.input(|input| input.key_pressed(Key::Enter)) {
                            verdict = Verdict::Accepted;
                        }
                    }
                    if !field.help.is_empty() {
                        ui.label(RichText::new(&field.help).small().weak());
                    }
                    ui.add_space(tokens.spacing.xs);
                }
            });
            if !self.problem.is_empty() {
                ui.colored_label(color(tokens.semantic.error), format!("Not saved: {}", self.problem));
            }
            ui.add_space(tokens.spacing.s);
            ui.horizontal(|ui| {
                if ui.add(primary(&self.accept, tokens)).clicked() {
                    verdict = Verdict::Accepted;
                }
                if ui.button("Cancel").clicked() {
                    verdict = Verdict::Cancelled;
                }
            });
        });
        if shown.should_close() && verdict == Verdict::Open {
            verdict = Verdict::Cancelled;
        }
        verdict
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Confirm {
    pub title: String,
    pub statement: String,
    pub accept: String,
    pub cancel: String,
    pub danger: bool,
    pub action: Option<Action>,
}

impl Confirm {
    pub fn new(title: &str, statement: impl Into<String>, accept: &str, action: Option<Action>) -> Confirm {
        Confirm {
            title: title.to_string(),
            statement: statement.into(),
            accept: accept.to_string(),
            cancel: "Cancel".to_string(),
            danger: false,
            action,
        }
    }

    pub fn show(&self, ctx: &Context, tokens: &Tokens) -> Verdict {
        let mut verdict = Verdict::Open;
        let shown = Modal::new(Id::unique("confirm-dialog")).show(ctx, |ui| {
            ui.set_width(480.0_f32.min(ui.ctx().content_rect().width() - 48.0));
            ui.heading(&self.title);
            ui.add_space(tokens.spacing.s);
            ScrollArea::vertical().max_height(ui.ctx().content_rect().height() - 220.0).show(ui, |ui| {
                ui.label(&self.statement);
            });
            ui.add_space(tokens.spacing.m);
            ui.horizontal(|ui| {
                if self.action.is_some() {
                    let accept = if self.danger { danger(&self.accept, tokens) } else { primary(&self.accept, tokens) };
                    if ui.add(accept).clicked() {
                        verdict = Verdict::Accepted;
                    }
                }
                if ui.button(&self.cancel).clicked() {
                    verdict = Verdict::Cancelled;
                }
            });
        });
        if shown.should_close() && verdict == Verdict::Open {
            verdict = Verdict::Cancelled;
        }
        verdict
    }
}

pub fn primary<'a>(label: &str, tokens: &Tokens) -> Button<'a> {
    Button::new(RichText::new(label).color(color(tokens.on_accent))).fill(color(tokens.palette.accent))
}

pub fn danger<'a>(label: &str, tokens: &Tokens) -> Button<'a> {
    let fill = tokens.semantic.error;
    Button::new(RichText::new(label).color(color(theme::best_on(fill)))).fill(color(fill))
}

pub fn muted(ui: &mut Ui, text: impl Into<String>) {
    ui.label(RichText::new(text).weak());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fields_keep_values_labels_and_limits() {
        let mut form = Form::new(
            "Example",
            "Save",
            vec![
                Field::text("name", "Name", "first", "Help.", 20),
                Field::area("note", "Note", "", "", 100),
                Field::choice("mode", "Mode", "b", "", &[("a", "Alpha"), ("b", "Beta")]),
                Field::yes_no("enabled", "Enabled", true, ""),
            ],
            Submit::AddRecord,
        );
        assert_eq!(form.value("name"), "first");
        assert_eq!(form.fields[2].shown(), "Beta");
        assert_eq!(form.fields[3].shown(), "Yes");
        assert!(form.fields[1].multiline && !form.fields[0].multiline);
        form.set("mode", "a");
        form.set("missing", "ignored");
        assert_eq!(form.value("mode"), "a");
        assert_eq!(form.value("missing"), "");
        assert!(form.fresh && form.problem.is_empty());
        let confirm = Confirm::new("Title", "Statement", "Go", None);
        assert_eq!((confirm.cancel.as_str(), confirm.danger), ("Cancel", false));
    }
}
