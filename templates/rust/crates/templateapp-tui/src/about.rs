use app_core::guide::{self, Guide};
use app_core::meta;
use app_core::service::{self, Diagnostics, Link};

use crate::model::{Action, Model, Notice, Preview, move_cursor};
use crate::text::{Line, Look, pad, window, wrap, wrapped};
use crate::widgets::{Input, Key, clip_start, row, selectable};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AboutTab {
    Overview,
    Guide,
    Shortcuts,
    Diagnostics,
    Licenses,
}

const TABS: &[(AboutTab, &str)] = &[
    (AboutTab::Overview, "Overview"),
    (AboutTab::Guide, "Usage guide"),
    (AboutTab::Shortcuts, "Shortcuts"),
    (AboutTab::Diagnostics, "Diagnostics"),
    (AboutTab::Licenses, "Licenses"),
];

const EXTRA_SHORTCUTS: &[(&str, &str)] = &[
    ("Tab, Shift+Tab", "Next or previous category"),
    (": or F10", "Open the menu"),
    ("Left, Right", "Switch sections inside a category"),
    ("e", "Records: edit the selected record"),
    ("x", "Records: enable or disable the selected record"),
    ("f, v", "Records: next status filter, next severity filter"),
    ("s, S", "Records: next sort column, reverse the order"),
    ("c", "Records: choose the table columns"),
    ("a", "Records: show or hide disabled records"),
    ("i, o", "Records: import, export"),
    ("l", "Overview: load demo data"),
    ("m, M", "Notifications: mark one as read, mark all as read"),
    ("u", "Notifications: unread only. Rules and Settings: reset"),
];

const SEARCH_LIMIT: usize = 100;

pub(crate) struct AboutPage {
    pub tab: AboutTab,
    pub cursor: usize,
    pub scroll: usize,
    pub usage: Guide,
    pub hits: Vec<usize>,
    pub section: usize,
    pub reading: bool,
    pub searching: bool,
    pub search: Input,
    pub diagnostics: Option<Diagnostics>,
}

impl AboutPage {
    pub fn new() -> AboutPage {
        let usage = service::usage_guide();
        let hits = (0..usage.sections.len()).collect();
        AboutPage {
            tab: AboutTab::Overview,
            cursor: 0,
            scroll: 0,
            usage,
            hits,
            section: 0,
            reading: false,
            searching: false,
            search: Input::new("", SEARCH_LIMIT),
            diagnostics: None,
        }
    }

    fn filter(&mut self) {
        let query = self.search.value();
        self.hits = if query.trim().is_empty() {
            (0..self.usage.sections.len()).collect()
        } else {
            let mut found: Vec<usize> = Vec::new();
            for hit in self.usage.search(query.trim()) {
                if !found.contains(&hit.section) {
                    found.push(hit.section);
                }
            }
            found
        };
        self.cursor = 0;
    }

    fn links(&self) -> Vec<(&'static str, Link, bool)> {
        let issue = self.diagnostics.as_ref().map(|report| report.issue_report().url).unwrap_or_default();
        vec![
            ("Source Code", service::source_link(), false),
            ("Found a bug? Please report it on GitHub.", service::external_link("Report a bug", &issue), true),
            ("Report a vulnerability privately", service::advisory_link(), false),
        ]
    }

    fn turn(&mut self, delta: usize) {
        let at = TABS.iter().position(|(tab, _)| *tab == self.tab).unwrap_or(0);
        self.tab = TABS[(at + delta) % TABS.len()].0;
        self.cursor = 0;
        self.scroll = 0;
    }

    fn scroll_key(&mut self, key: Key) {
        match key {
            Key::Up | Key::Char('k') => self.scroll = self.scroll.saturating_sub(1),
            Key::Down | Key::Char('j') => self.scroll += 1,
            Key::PageUp => self.scroll = self.scroll.saturating_sub(10),
            Key::PageDown | Key::Char(' ') => self.scroll += 10,
            _ => {}
        }
    }
}

impl Model<'_> {
    pub(crate) fn load_about(&mut self) {
        self.about.diagnostics = Some(self.app.diagnostics(self.options.has_dashboard));
    }

    pub(crate) fn about_key(&mut self, key: Key) {
        let page = &mut self.about;
        if page.searching {
            match key {
                Key::Enter => page.searching = false,
                Key::Esc => {
                    page.searching = false;
                    page.search = Input::new("", SEARCH_LIMIT);
                    page.filter();
                }
                _ => {
                    if page.search.handle(key) {
                        page.filter();
                    }
                }
            }
            return;
        }
        match key {
            Key::Right | Key::Char(']') if !page.reading => return page.turn(1),
            Key::Left | Key::Char('[') if !page.reading => return page.turn(TABS.len() - 1),
            Key::Right | Key::Left | Key::Char('[' | ']') => return,
            _ => {}
        }
        match page.tab {
            AboutTab::Overview => {
                let links = page.links();
                if move_cursor(&mut page.cursor, key, links.len()) || key != Key::Enter {
                    return;
                }
                let (_, link, issue) = links[page.cursor.min(links.len() - 1)].clone();
                if !issue {
                    let opened = self.open_link(&link);
                    if let Some(done) = self.fail(opened) {
                        self.say(Notice::Success, done);
                    }
                    return;
                }
                let Some(report) = self.about.diagnostics.as_ref().map(Diagnostics::issue_report) else {
                    return;
                };
                let mut lines = vec![
                    "The report form opens in your browser, prefilled with exactly this text and nothing else. You can edit it before you submit it.".to_string(),
                    String::new(),
                ];
                lines.extend(report.body.split('\n').map(str::to_string));
                lines.push(String::new());
                lines.push(format!("Link: {}", report.url));
                self.preview = Some(Preview {
                    title: "Report a bug".to_string(),
                    lines,
                    offset: 0,
                    accept: "open the report form in the browser".to_string(),
                    action: Some(Action::OpenLink(link)),
                });
            }
            AboutTab::Guide if page.reading => match key {
                Key::Esc | Key::Backspace | Key::Enter => {
                    page.reading = false;
                    page.scroll = 0;
                }
                _ => page.scroll_key(key),
            },
            AboutTab::Guide => {
                let count = page.hits.len();
                if move_cursor(&mut page.cursor, key, count) {
                    return;
                }
                match key {
                    Key::Char('/') => page.searching = true,
                    Key::Esc => {
                        page.search = Input::new("", SEARCH_LIMIT);
                        page.filter();
                    }
                    Key::Enter => {
                        if let Some(section) = page.hits.get(page.cursor) {
                            page.section = *section;
                            page.reading = true;
                            page.scroll = 0;
                        }
                    }
                    _ => {}
                }
            }
            _ => page.scroll_key(key),
        }
    }

    pub(crate) fn about_view(&mut self, size: usize, height: usize) -> Vec<Line> {
        let page = &self.about;
        let mut tabs = Line::new();
        for (index, (tab, title)) in TABS.iter().enumerate() {
            if index > 0 {
                tabs = tabs.with(" ", Look::Plain);
            }
            tabs = if *tab == page.tab {
                tabs.with(format!("▌{title} "), Look::Selected)
            } else {
                tabs.with(format!(" {title} "), Look::Plain)
            };
        }
        let mut lines = vec![tabs, Line::new()];
        let room = height.saturating_sub(3).max(3);
        let body: Vec<Line> = match page.tab {
            AboutTab::Overview => {
                lines.push(Line::styled(format!("{} {}", meta::APP_NAME, meta::VERSION), Look::Heading));
                lines.push(Line::new());
                lines.extend(wrapped(meta::TAGLINE, size, Look::Plain));
                lines.push(Line::new());
                lines.push(Line::plain(format!("Publisher: {}", meta::PUBLISHER)));
                lines.push(Line::plain(format!("License: {}", meta::LICENSE)));
                lines.push(Line::new());
                for (index, (label, link, _)) in page.links().iter().enumerate() {
                    let address = clip_start(&link.url, size.saturating_sub(46));
                    let line = if index == page.cursor {
                        Line::styled(pad(&format!("▌ {label}"), 42), Look::Selected).with(" ", Look::Plain)
                    } else {
                        Line::plain(pad(&format!("  {label}"), 43))
                    };
                    lines.push(line.with(address, Look::Muted));
                }
                lines.push(Line::new());
                lines.extend(wrapped(
                    "Enter opens the selected link in your browser. Nothing is sent by the application.",
                    size,
                    Look::Muted,
                ));
                return lines;
            }
            AboutTab::Guide if page.reading => {
                let Some(section) = page.usage.sections.get(page.section) else {
                    return lines;
                };
                let mut body = vec![Line::styled(section.title.clone(), Look::Heading), Line::new()];
                body.extend(wrapped(&guide::plain(&section.body), size, Look::Plain));
                body.push(Line::new());
                body.push(Line::styled("Esc back to the contents", Look::Muted));
                body
            }
            AboutTab::Guide => {
                let search = if page.searching {
                    page.search.view(30, true).trim_end().to_string()
                } else if page.search.value().is_empty() {
                    "press / to search".to_string()
                } else {
                    page.search.value()
                };
                lines.push(Line::styled(format!("Search: {search}"), Look::Muted));
                lines.push(Line::new());
                if page.hits.is_empty() {
                    lines.extend(wrapped(
                        "No section mentions these words. Press Esc to show the contents.",
                        size,
                        Look::Plain,
                    ));
                    return lines;
                }
                let (start, end) = window(page.hits.len(), page.cursor, room.saturating_sub(2).max(1));
                for (index, hit) in page.hits.iter().enumerate().take(end).skip(start) {
                    let section = &page.usage.sections[*hit];
                    let title = format!("{}{}", "  ".repeat(section.level.saturating_sub(1)), section.title);
                    lines.push(selectable(&title, index == page.cursor, size));
                }
                return lines;
            }
            AboutTab::Shortcuts => {
                let widths = [22, size.saturating_sub(24)];
                let mut body = vec![Line::styled(row(&["KEY", "ACTION"], &widths), Look::Muted)];
                body.extend(
                    service::SHORTCUTS
                        .iter()
                        .map(|shortcut| Line::plain(row(&[shortcut.terminal, shortcut.action], &widths))),
                );
                body.extend(EXTRA_SHORTCUTS.iter().map(|(keys, action)| Line::plain(row(&[*keys, *action], &widths))));
                body
            }
            AboutTab::Diagnostics => {
                let mut body: Vec<Line> = page
                    .diagnostics
                    .iter()
                    .flat_map(Diagnostics::lines)
                    .map(|(label, value)| Line::plain(format!("{} {value}", pad(label, 30))))
                    .collect();
                body.push(Line::new());
                body.extend(wrapped(
                    "Diagnostics stay on this computer. A bug report includes only the lines shown in its preview.",
                    size,
                    Look::Muted,
                ));
                body
            }
            AboutTab::Licenses => wrap(&format!("{}\n\n{}", meta::LICENSE_TEXT, meta::NOTICES_TEXT), size)
                .into_iter()
                .map(Line::plain)
                .collect(),
        };
        let scroll = page.scroll.min(body.len().saturating_sub(room));
        lines.extend(body.into_iter().skip(scroll).take(room));
        self.about.scroll = scroll;
        lines
    }
}
