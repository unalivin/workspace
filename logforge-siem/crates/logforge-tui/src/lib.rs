use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use logforge_core::SiemService;
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Terminal,
};
use std::io;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TuiTab {
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

impl TuiTab {
    pub fn all() -> &'static [TuiTab] {
        &[
            TuiTab::Overview,
            TuiTab::Search,
            TuiTab::SavedQueries,
            TuiTab::Sources,
            TuiTab::Schema,
            TuiTab::Detections,
            TuiTab::Alerts,
            TuiTab::Retention,
            TuiTab::Audit,
            TuiTab::About,
            TuiTab::Settings,
        ]
    }

    pub fn label(&self) -> &'static str {
        match self {
            TuiTab::Overview => "1 Overview",
            TuiTab::Search => "2 Search",
            TuiTab::SavedQueries => "3 Saved Queries",
            TuiTab::Sources => "4 Sources",
            TuiTab::Schema => "5 Schema",
            TuiTab::Detections => "6 Detections",
            TuiTab::Alerts => "7 Alerts",
            TuiTab::Retention => "8 Retention",
            TuiTab::Audit => "9 Audit",
            TuiTab::About => "0 About",
            TuiTab::Settings => "s Settings",
        }
    }
}

pub fn run_tui(service: &SiemService) -> io::Result<()> {
    let no_color = std::env::var_os("NO_COLOR").is_some();
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut active_tab = TuiTab::Overview;
    let accent = if no_color { Color::Reset } else { Color::Yellow };

    loop {
        let counts = service.overview_counts().unwrap_or(logforge_core::OverviewCounts {
            total_events: 0,
            active_sources: 0,
            open_alerts: 0,
            stat_received: 0,
            stat_deduplicated: 0,
            stat_dropped: 0,
            stat_malformed: 0,
        });
        let alerts = service.list_alerts(Some("open")).unwrap_or_default();
        let events = service.search_events("", 25).unwrap_or_default();

        terminal.draw(|frame| {
            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Length(22), Constraint::Min(30)])
                .split(frame.area());

            let items: Vec<ListItem> = TuiTab::all()
                .iter()
                .map(|t| {
                    let style = if *t == active_tab {
                        Style::default().fg(accent).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                    };
                    ListItem::new(t.label()).style(style)
                })
                .collect();

            let sidebar = List::new(items)
                .block(Block::default().borders(Borders::ALL).title("LogForge-SIEM"));
            frame.render_widget(sidebar, chunks[0]);

            let body = match active_tab {
                TuiTab::Overview => format!(
                    "Total events: {}\nActive sources: {}\nOpen alerts: {}\nReceived: {} | Deduplicated: {} | Dropped: {} | Malformed: {}\n\nPress 1-7 to switch tabs, q to quit.",
                    counts.total_events,
                    counts.active_sources,
                    counts.open_alerts,
                    counts.stat_received,
                    counts.stat_deduplicated,
                    counts.stat_dropped,
                    counts.stat_malformed
                ),
                TuiTab::Search => events
                    .iter()
                    .map(|e| format!("{} [{}] {}: {}", e.timestamp_utc, e.severity, e.host, e.message))
                    .collect::<Vec<_>>()
                    .join("\n"),
                TuiTab::SavedQueries => service
                    .list_saved_queries()
                    .unwrap_or_default()
                    .iter()
                    .map(|q| format!("{}: {}", q.name, q.query))
                    .collect::<Vec<_>>()
                    .join("\n"),
                TuiTab::Alerts => alerts
                    .iter()
                    .map(|a| format!("#{} [{}] {} ({})", a.id, a.severity, a.title, a.status))
                    .collect::<Vec<_>>()
                    .join("\n"),
                TuiTab::Sources => service
                    .list_sources()
                    .unwrap_or_default()
                    .iter()
                    .map(|s| format!("{} ({}) - {}", s.name, s.kind, s.location))
                    .collect::<Vec<_>>()
                    .join("\n"),
                TuiTab::Schema => "Normalized fields: timestamp_utc, severity, host, service, message, dedup_hash, redacted".to_string(),
                TuiTab::Detections => service
                    .list_rules()
                    .unwrap_or_default()
                    .iter()
                    .map(|r| format!("{} [{}] pattern={}", r.name, r.severity, r.pattern))
                    .collect::<Vec<_>>()
                    .join("\n"),
                TuiTab::Retention => {
                    let pol = logforge_core::retention::get_default_policy(service.db());
                    let metrics = logforge_core::storage_metrics::collect_storage_metrics(service.db());
                    match (pol, metrics) {
                        (Ok(p), Ok(m)) => format!(
                            "Policy: {} ({} days, max {} events)\nStorage: {} bytes ({} events, {} alerts)",
                            p.name, p.max_days, p.max_events, m.total_bytes, m.events_count, m.alerts_count
                        ),
                        _ => "Retention policy unavailable.".to_string(),
                    }
                }
                TuiTab::Audit => match logforge_core::audit_verify::verify_audit_chain(service.db()) {
                    Ok(res) => format!("Audit entries: {} | Chain valid: {}", res.total_entries, res.valid),
                    Err(_) => "Audit verification unavailable.".to_string(),
                },
                TuiTab::About => "LogForge-SIEM v0.1.0-dev\nCopyright (c) 2026 Sonix Devs\nRepository: https://github.com/sonix-devs/logforge-siem".to_string(),
                TuiTab::Settings => "Theme: dark | Accent: amber | Density: compact\nUse `logforge-siem config theme set <theme>` to change.".to_string(),
            };

            let panel = Paragraph::new(body).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(active_tab.label()),
            );
            frame.render_widget(panel, chunks[1]);
        })?;

        if event::poll(Duration::from_millis(250))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => break,
                    KeyCode::Char('1') => active_tab = TuiTab::Overview,
                    KeyCode::Char('2') => active_tab = TuiTab::Search,
                    KeyCode::Char('3') => active_tab = TuiTab::SavedQueries,
                    KeyCode::Char('4') => active_tab = TuiTab::Sources,
                    KeyCode::Char('5') => active_tab = TuiTab::Schema,
                    KeyCode::Char('6') => active_tab = TuiTab::Detections,
                    KeyCode::Char('7') => active_tab = TuiTab::Alerts,
                    KeyCode::Char('8') => active_tab = TuiTab::Retention,
                    KeyCode::Char('9') => active_tab = TuiTab::Audit,
                    KeyCode::Char('0') => active_tab = TuiTab::About,
                    KeyCode::Char('s') => active_tab = TuiTab::Settings,
                    _ => {}
                }
            }
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}
