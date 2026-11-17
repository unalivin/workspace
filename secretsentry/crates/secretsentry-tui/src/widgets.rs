use crate::text::{Line, Look, clip, pad, width, window, wrapped};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Char(char),
    Ctrl(char),
    F(u8),
    Enter,
    Esc,
    Tab,
    BackTab,
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,
    Backspace,
    Delete,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Input {
    letters: Vec<char>,
    cursor: usize,
    limit: usize,
}

impl Input {
    pub fn new(value: &str, limit: usize) -> Input {
        let letters: Vec<char> = value.chars().collect();
        Input { cursor: letters.len(), letters, limit }
    }

    pub fn value(&self) -> String {
        self.letters.iter().collect()
    }

    pub fn handle(&mut self, key: Key) -> bool {
        match key {
            Key::Left => self.cursor = self.cursor.saturating_sub(1),
            Key::Right => self.cursor = (self.cursor + 1).min(self.letters.len()),
            Key::Home | Key::Ctrl('a') => self.cursor = 0,
            Key::End | Key::Ctrl('e') => self.cursor = self.letters.len(),
            Key::Backspace => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                    self.letters.remove(self.cursor);
                }
            }
            Key::Delete => {
                if self.cursor < self.letters.len() {
                    self.letters.remove(self.cursor);
                }
            }
            Key::Ctrl('u') => {
                self.letters.drain(..self.cursor);
                self.cursor = 0;
            }
            Key::Char(letter) => {
                if !letter.is_control() && (self.limit == 0 || self.letters.len() < self.limit) {
                    self.letters.insert(self.cursor, letter);
                    self.cursor += 1;
                }
            }
            _ => return false,
        }
        true
    }

    pub fn view(&self, size: usize, focused: bool) -> String {
        if !focused {
            return pad(&self.value(), size);
        }
        let after: String = self.letters[self.cursor..].iter().collect();
        let mut start = 0;
        loop {
            let before: String = self.letters[start..self.cursor].iter().collect();
            let shown = format!("{before}▏{after}");
            if width(&shown) <= size || start >= self.cursor {
                return pad(&shown, size);
            }
            start += 1;
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Control {
    Text(Input),
    Choice { options: Vec<(String, String)>, chosen: usize },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    pub key: String,
    pub label: String,
    pub help: String,
    pub control: Control,
}

impl Field {
    pub fn text(key: &str, label: &str, value: &str, help: &str, limit: usize) -> Field {
        Field {
            key: key.to_string(),
            label: label.to_string(),
            help: help.to_string(),
            control: Control::Text(Input::new(value, limit)),
        }
    }

    pub fn choice(key: &str, label: &str, value: &str, help: &str, options: &[(&str, &str)]) -> Field {
        let chosen = options.iter().position(|(option, _)| *option == value).unwrap_or(0);
        Field {
            key: key.to_string(),
            label: label.to_string(),
            help: help.to_string(),
            control: Control::Choice {
                options: options.iter().map(|(option, label)| (option.to_string(), label.to_string())).collect(),
                chosen,
            },
        }
    }

    pub fn yes_no(key: &str, label: &str, value: bool, help: &str) -> Field {
        Field::choice(key, label, if value { "true" } else { "false" }, help, &[("true", "Yes"), ("false", "No")])
    }

    pub fn value(&self) -> String {
        match &self.control {
            Control::Text(input) => input.value(),
            Control::Choice { options, chosen } => options[*chosen].0.clone(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormEvent {
    Open,
    Submit,
    Cancel,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Form {
    pub title: String,
    pub fields: Vec<Field>,
    pub focus: usize,
    pub problem: String,
}

pub const FORM_HINT: &str = "Tab next field · Left/Right change a choice · Enter or Ctrl+S save · Esc cancel";

impl Form {
    pub fn new(title: impl Into<String>, fields: Vec<Field>) -> Form {
        Form { title: title.into(), fields, focus: 0, problem: String::new() }
    }

    pub fn value(&self, key: &str) -> String {
        self.fields.iter().find(|field| field.key == key).map(Field::value).unwrap_or_default()
    }

    pub fn handle(&mut self, key: Key) -> FormEvent {
        let count = self.fields.len();
        if count == 0 {
            return FormEvent::Cancel;
        }
        match key {
            Key::Esc => return FormEvent::Cancel,
            Key::Tab | Key::Down => self.focus = (self.focus + 1) % count,
            Key::BackTab | Key::Up => self.focus = (self.focus + count - 1) % count,
            Key::Enter if self.focus + 1 < count => self.focus += 1,
            Key::Enter | Key::Ctrl('s') => return FormEvent::Submit,
            _ => match &mut self.fields[self.focus].control {
                Control::Choice { options, chosen } => match key {
                    Key::Left => *chosen = (*chosen + options.len() - 1) % options.len(),
                    Key::Right | Key::Char(' ') => *chosen = (*chosen + 1) % options.len(),
                    _ => {}
                },
                Control::Text(input) => {
                    input.handle(key);
                }
            },
        }
        FormEvent::Open
    }

    pub fn view(&self, size: usize) -> Vec<Line> {
        let mut lines = vec![Line::styled(self.title.clone(), Look::Heading), Line::new()];
        let label_width = self.fields.iter().map(|field| width(&field.label)).max().unwrap_or(0);
        let value_width = size.saturating_sub(label_width + 6).max(10);
        for (index, field) in self.fields.iter().enumerate() {
            let focused = index == self.focus;
            let value = match &field.control {
                Control::Choice { options, chosen } => format!("‹ {} ›", options[*chosen].1),
                Control::Text(input) => input.view(value_width, focused),
            };
            let look = if focused { Look::Accent } else { Look::Plain };
            lines.push(
                Line::styled(if focused { "▌ " } else { "  " }, Look::Accent)
                    .with(pad(&field.label, label_width), look)
                    .with(format!("  {value}"), Look::Plain),
            );
            if focused && !field.help.is_empty() {
                for help in wrapped(&field.help, size.saturating_sub(4), Look::Muted) {
                    lines.push(Line::plain("    ").with(help.text(), Look::Muted));
                }
            }
        }
        lines.push(Line::new());
        if !self.problem.is_empty() {
            let text = format!("Not saved: {}", self.problem);
            for (index, line) in wrapped(&text, size.saturating_sub(2), Look::Failure).into_iter().enumerate() {
                let lead = if index == 0 { "✖ " } else { "  " };
                lines.push(Line::styled(format!("{lead}{}", line.text()), Look::Failure));
            }
            lines.push(Line::new());
        }
        lines.extend(wrapped(FORM_HINT, size, Look::Muted));
        lines
    }
}

const MIN_FLEXIBLE: usize = 8;

pub fn layout(columns: &[(usize, bool)], size: usize) -> Vec<usize> {
    let fixed: usize = columns.iter().map(|(column, flex)| if *flex { 2 } else { column + 2 }).sum();
    let flexible = columns.iter().filter(|(_, flex)| *flex).count();
    let share = size.saturating_sub(fixed).checked_div(flexible).unwrap_or(0).max(MIN_FLEXIBLE);
    columns.iter().map(|(column, flex)| if *flex { share } else { *column }).collect()
}

pub fn row<T: AsRef<str>>(cells: &[T], widths: &[usize]) -> String {
    let mut out = String::new();
    for (cell, size) in cells.iter().zip(widths) {
        out.push_str(&pad(cell.as_ref(), *size));
        out.push_str("  ");
    }
    out
}

pub fn table(
    columns: &[(&str, usize, bool)],
    rows: &[Vec<String>],
    cursor: usize,
    room: usize,
    size: usize,
) -> Vec<Line> {
    let shape: Vec<(usize, bool)> = columns.iter().map(|(_, column, flex)| (*column, *flex)).collect();
    let widths = layout(&shape, size.saturating_sub(2));
    let titles: Vec<&str> = columns.iter().map(|(title, _, _)| *title).collect();
    let mut lines = vec![Line::styled(format!("  {}", row(&titles, &widths)), Look::Muted)];
    let (start, end) = window(rows.len(), cursor, room.max(3));
    for (index, cells) in rows.iter().enumerate().take(end).skip(start) {
        lines.push(selectable(&row(cells, &widths), index == cursor, size));
    }
    let at = cursor.min(rows.len().saturating_sub(1)) + 1;
    lines.push(Line::styled(format!("Row {at} of {}", rows.len()), Look::Muted));
    lines
}

pub fn tail(text: &str, size: usize) -> String {
    let letters: Vec<char> = text.chars().collect();
    if letters.len() <= size || size == 0 {
        return clip(text, size);
    }
    let kept: String = letters[letters.len() - (size - 1)..].iter().collect();
    format!("…{kept}")
}

pub fn clip_start(text: &str, size: usize) -> String {
    if size < 8 { String::new() } else { clip(text, size) }
}

pub fn selectable(text: &str, selected: bool, size: usize) -> Line {
    if selected {
        Line::styled(pad(&format!("▌ {text}"), size), Look::Selected)
    } else {
        Line::plain(format!("  {text}"))
    }
}

pub fn step(current: usize, delta: isize, count: usize) -> usize {
    if count == 0 {
        return 0;
    }
    current.saturating_add_signed(delta).min(count - 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn typed(input: &mut Input, text: &str) {
        for letter in text.chars() {
            input.handle(Key::Char(letter));
        }
    }

    #[test]
    fn the_text_input_edits_at_the_cursor_and_respects_its_limit() {
        let mut input = Input::new("abc", 6);
        typed(&mut input, "de");
        assert_eq!(input.value(), "abcde");
        input.handle(Key::Home);
        typed(&mut input, "Xyz");
        assert_eq!(input.value(), "Xabcde");
        input.handle(Key::Delete);
        assert_eq!(input.value(), "Xbcde");
        input.handle(Key::End);
        input.handle(Key::Backspace);
        input.handle(Key::Left);
        input.handle(Key::Left);
        input.handle(Key::Ctrl('u'));
        assert_eq!(input.value(), "cd");
        input.handle(Key::Right);
        input.handle(Key::Char('\u{7}'));
        assert_eq!(input.value(), "cd");
        assert!(!input.handle(Key::F(5)));
        assert_eq!(input.view(6, true), "c▏d   ");
        assert_eq!(input.view(6, false), "cd    ");
        let long = Input::new("abcdefghijklmnop", 0);
        assert_eq!(long.view(8, true), "jklmnop▏");
    }

    #[test]
    fn a_form_moves_between_fields_changes_choices_and_submits_on_the_last_field() {
        let mut form = Form::new(
            "Demo",
            vec![
                Field::text("name", "Name", "", "Required.", 20),
                Field::choice("level", "Level", "b", "", &[("a", "First"), ("b", "Second"), ("c", "Third")]),
                Field::yes_no("enabled", "Enabled", true, ""),
            ],
        );
        typed_form(&mut form, "Ada");
        assert_eq!(form.handle(Key::Enter), FormEvent::Open);
        assert_eq!(form.focus, 1);
        form.handle(Key::Right);
        assert_eq!(form.value("level"), "c");
        form.handle(Key::Right);
        assert_eq!(form.value("level"), "a");
        form.handle(Key::Left);
        assert_eq!(form.value("level"), "c");
        form.handle(Key::Tab);
        form.handle(Key::Char(' '));
        assert_eq!(form.value("enabled"), "false");
        assert_eq!(form.handle(Key::Enter), FormEvent::Submit);
        form.handle(Key::BackTab);
        assert_eq!(form.focus, 1);
        assert_eq!(form.handle(Key::Ctrl('s')), FormEvent::Submit);
        assert_eq!(form.handle(Key::Esc), FormEvent::Cancel);
        assert_eq!((form.value("name"), form.value("missing")), ("Ada".to_string(), String::new()));
        form.problem = "name: the name is empty".to_string();
        let shown: Vec<String> = form.view(100).iter().map(Line::text).collect();
        assert!(shown.iter().any(|line| line.contains("‹ Third ›")), "{shown:?}");
        assert!(shown.iter().any(|line| line.starts_with("✖ Not saved: name")), "{shown:?}");
        assert_eq!(shown.last().unwrap(), FORM_HINT);
    }

    fn typed_form(form: &mut Form, text: &str) {
        for letter in text.chars() {
            form.handle(Key::Char(letter));
        }
    }

    #[test]
    fn columns_share_spare_room_and_rows_keep_their_widths() {
        assert_eq!(layout(&[(5, false), (16, true), (8, false)], 55), vec![5, 36, 8]);
        assert_eq!(layout(&[(5, false), (16, true)], 10), vec![5, 8]);
        assert_eq!(layout(&[(5, false), (7, false)], 40), vec![5, 7]);
        assert_eq!(row(&["7", "A long name"], &[3, 6]), "7    A lon…  ");
        assert_eq!(tail("/home/someone/data/app.db", 10), "…ta/app.db");
        assert_eq!(tail("short", 10), "short");
        assert_eq!(
            (clip_start("https://example.org/a/b", 5), clip_start("https://example.org/a/b", 9)),
            (String::new(), "https://…".to_string())
        );
        assert_eq!(selectable("row", true, 8).text(), "▌ row   ");
        assert_eq!(selectable("row", false, 8).text(), "  row");
        assert_eq!((step(0, -1, 4), step(3, 1, 4), step(1, 10, 4), step(2, 1, 0)), (0, 3, 3, 0));
    }
}
