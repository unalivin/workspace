use app_core::theme::Severity;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Look {
    Plain,
    Muted,
    Accent,
    Heading,
    Selected,
    Border,
    Status,
    Success,
    Warning,
    Failure,
    Level(Severity),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    pub text: String,
    pub look: Look,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Line(pub Vec<Span>);

pub fn width(text: &str) -> usize {
    UnicodeWidthStr::width(text)
}

pub fn clean(text: &str) -> String {
    text.chars().map(|letter| if letter.is_control() { ' ' } else { letter }).collect()
}

pub fn clip(text: &str, limit: usize) -> String {
    let cleaned = clean(text);
    if width(&cleaned) <= limit {
        return cleaned;
    }
    if limit == 0 {
        return String::new();
    }
    let mut out = String::new();
    let mut used = 0;
    for letter in cleaned.chars() {
        let size = UnicodeWidthChar::width(letter).unwrap_or(0);
        if used + size > limit - 1 {
            break;
        }
        out.push(letter);
        used += size;
    }
    out.push('…');
    out
}

pub fn pad(text: &str, size: usize) -> String {
    let mut out = clip(text, size);
    let used = width(&out);
    out.extend(std::iter::repeat_n(' ', size.saturating_sub(used)));
    out
}

pub fn wrap(text: &str, limit: usize) -> Vec<String> {
    let limit = limit.max(8);
    let mut lines = Vec::new();
    for paragraph in text.split('\n') {
        let paragraph = clean(paragraph);
        let mut current = String::new();
        for word in paragraph.split(' ') {
            let mut word = word.to_string();
            while width(&word) > limit {
                if !current.is_empty() {
                    lines.push(std::mem::take(&mut current));
                }
                let mut head = String::new();
                let mut used = 0;
                let mut rest = String::new();
                for letter in word.chars() {
                    let size = UnicodeWidthChar::width(letter).unwrap_or(0);
                    if rest.is_empty() && used + size <= limit {
                        head.push(letter);
                        used += size;
                    } else {
                        rest.push(letter);
                    }
                }
                lines.push(head);
                word = rest;
            }
            if current.is_empty() {
                current = word;
            } else if width(&current) + 1 + width(&word) <= limit {
                current.push(' ');
                current.push_str(&word);
            } else {
                lines.push(std::mem::replace(&mut current, word));
            }
        }
        lines.push(current);
    }
    lines
}

pub fn window(total: usize, cursor: usize, height: usize) -> (usize, usize) {
    if height == 0 || total == 0 {
        return (0, 0);
    }
    if total <= height {
        return (0, total);
    }
    let cursor = cursor.min(total - 1);
    let start = cursor.saturating_sub(height / 2).min(total - height);
    (start, start + height)
}

impl Line {
    pub fn new() -> Line {
        Line(Vec::new())
    }

    pub fn plain(text: impl Into<String>) -> Line {
        Line::styled(text, Look::Plain)
    }

    pub fn styled(text: impl Into<String>, look: Look) -> Line {
        Line(vec![Span { text: text.into(), look }])
    }

    pub fn with(mut self, text: impl Into<String>, look: Look) -> Line {
        self.0.push(Span { text: text.into(), look });
        self
    }

    pub fn width(&self) -> usize {
        self.0.iter().map(|span| width(&span.text)).sum()
    }

    pub fn text(&self) -> String {
        self.0.iter().map(|span| span.text.as_str()).collect()
    }

    pub fn recolor(mut self, look: Look) -> Line {
        for span in &mut self.0 {
            span.look = look;
        }
        self
    }

    pub fn fit(self, size: usize) -> Line {
        let mut out = Vec::new();
        let mut used = 0;
        for span in self.0 {
            if used >= size {
                break;
            }
            let room = size - used;
            let text = if width(&span.text) > room { clip(&span.text, room) } else { clean(&span.text) };
            used += width(&text);
            out.push(Span { text, look: span.look });
        }
        if used < size {
            let look = out.last().map(|span| span.look).filter(|look| *look == Look::Selected).unwrap_or(Look::Plain);
            out.push(Span { text: " ".repeat(size - used), look });
        }
        Line(out)
    }
}

pub fn wrapped(text: &str, limit: usize, look: Look) -> Vec<Line> {
    wrap(text, limit).into_iter().map(|line| Line::styled(line, look)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_is_clipped_padded_and_wrapped_by_display_width() {
        assert_eq!(clip("hello", 5), "hello");
        assert_eq!(clip("hello world", 6), "hello…");
        assert_eq!(clip("日本語のテキスト", 7), "日本語…");
        assert_eq!(clip("a\tb\nc", 9), "a b c");
        assert_eq!(clip("abc", 0), "");
        assert_eq!(pad("ab", 4), "ab  ");
        assert_eq!(width(&pad("日本語のテキスト", 9)), 9);
        assert_eq!(wrap("one two three four", 9), vec!["one two", "three", "four"]);
        assert_eq!(wrap("abcdefghijklmnop", 8), vec!["abcdefgh", "ijklmnop"]);
        assert_eq!(wrap("first\n\nsecond", 20), vec!["first", "", "second"]);
        assert!(wrap(&"x".repeat(500), 10).iter().all(|line| width(line) <= 10));
    }

    #[test]
    fn the_scroll_window_always_contains_the_cursor() {
        assert_eq!(window(0, 0, 5), (0, 0));
        assert_eq!(window(3, 2, 5), (0, 3));
        assert_eq!(window(20, 0, 5), (0, 5));
        assert_eq!(window(20, 19, 5), (15, 20));
        for cursor in 0..20 {
            let (start, end) = window(20, cursor, 5);
            assert!(start <= cursor && cursor < end && end - start == 5, "{cursor}: {start}..{end}");
        }
    }

    #[test]
    fn lines_fit_their_column_exactly() {
        let line = Line::plain("name").with(" · ", Look::Muted).with("a long value", Look::Accent);
        assert_eq!(line.width(), 19);
        assert_eq!(line.clone().fit(10).text(), "name · a …");
        assert_eq!(line.clone().fit(25).text(), "name · a long value      ");
        assert_eq!(Line::styled("row", Look::Selected).fit(6).0.last().unwrap().look, Look::Selected);
        assert_eq!(line.recolor(Look::Selected).0[1].look, Look::Selected);
    }
}
