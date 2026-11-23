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
    widgets::{
        Block, Borders, Cell, List, ListItem, Paragraph, Row, Table, TableState as WidgetTableState,
    },
    Terminal,
};
use std::io;
use std::time::Duration;

mod table;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TuiTab {
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
            TuiTab::Cases,
            TuiTab::Retention,
            TuiTab::Audit,
            TuiTab::Health,
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
            TuiTab::Cases => "c Cases",
            TuiTab::Retention => "8 Retention",
            TuiTab::Audit => "9 Audit",
            TuiTab::Health => "h Health",
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
    let mut filter = String::new();
    let mut filter_active = false;
    let mut reverse_sort = false;
    let mut page = 0usize;
    let mut selected_row = 0usize;
    let mut detail_open = true;
    let mut columns_open = false;
    let mut visible_columns = [true; 6];
    let mut export_notice = String::new();
    let mut export_requested = false;
    let accent = if no_color {
        Color::Reset
    } else {
        Color::Yellow
    };

    loop {
        let counts = service
            .overview_counts()
            .unwrap_or(logforge_core::OverviewCounts {
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
        let analytics = logforge_core::event_analytics::analytics_snapshot(service.db(), 5).ok();

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
                    "Total events: {}\nActive sources: {}\nOpen alerts: {}\nReceived: {} | Deduplicated: {} | Dropped: {} | Malformed: {}\nIngestion latency average/p95: {}/{} ms\nTop hosts: {}\n\nPress 1-9 or c to switch tabs, q to quit.",
                    counts.total_events,
                    counts.active_sources,
                    counts.open_alerts,
                    counts.stat_received,
                    counts.stat_deduplicated,
                    counts.stat_dropped,
                    counts.stat_malformed,
                    analytics.as_ref().map_or(0, |snapshot| snapshot.ingestion_latency.average_millis),
                    analytics.as_ref().map_or(0, |snapshot| snapshot.ingestion_latency.p95_millis),
                    analytics.as_ref().map(|snapshot| snapshot.top_hosts.iter().map(|host| format!("{} ({})", host.value, host.event_count)).collect::<Vec<_>>().join(", ")).unwrap_or_default()
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
                TuiTab::Detections => {
                    let mut lines: Vec<String> = service.list_rules().unwrap_or_default().iter()
                        .map(|rule| format!("{} [{}] pattern={}", rule.name, rule.severity, rule.pattern))
                        .collect();
                    lines.extend(logforge_core::correlation_admin::list_rules(service.db(), false).unwrap_or_default().iter()
                        .map(|rule| format!("Correlation #{}: {} [{}] {} on {} window={}s threshold={} enabled={}", rule.id, rule.name, rule.severity, rule.kind, rule.entity_field, rule.window_seconds, rule.threshold, rule.enabled)));
                    lines.join("\n")
                },
                TuiTab::Cases => logforge_core::cases::CaseManager::new(service.db())
                    .list(None)
                    .unwrap_or_default()
                    .iter()
                    .map(|case| format!("#{} [{}] {} — {} alerts, {}", case.id, case.status, case.title, case.alert_count, case.assignee))
                    .collect::<Vec<_>>()
                    .join("\n"),
                TuiTab::Retention => {
                    let pol = logforge_core::retention::get_default_policy(service.db());
                    let metrics = logforge_core::storage_metrics::collect_storage_metrics(service.db());
                    match (pol, metrics) {
                        (Ok(p), Ok(m)) => {
                            let plan = logforge_core::retention::plan_retention_with_reasons(
                                service.db(), p.max_days, p.max_events, p.max_bytes,
                                &chrono::Utc::now().to_rfc3339(),
                            );
                            match plan {
                                Ok(plan) => format!(
                                    "Policy: {} ({} days, max {} events, {} bytes)\nStorage: {} bytes ({} events, {} alerts)\nDry run: {} events, {} bytes would be removed\nReasons: age={} count={} bytes={}\nCandidates: {}",
                                    p.name, p.max_days, p.max_events, p.max_bytes, m.total_bytes, m.events_count, m.alerts_count, plan.plan.count, plan.plan.bytes_to_free, plan.age_expired, plan.event_limit_exceeded, plan.byte_limit_exceeded,
                                    plan.plan.event_ids.iter().map(|id| id.to_string()).collect::<Vec<_>>().join(", ")
                                ),
                                Err(_) => "Retention preview unavailable.".to_string(),
                            }
                        }
                        _ => "Retention policy unavailable.".to_string(),
                    }
                }
                TuiTab::Audit => {
                    let verification = logforge_core::audit_verify::verify_audit_chain(service.db());
                    let entries = logforge_core::audit_verify::query_audit_log(
                        service.db(),
                        &logforge_core::audit_verify::AuditFilter {
                            limit: 25,
                            ..logforge_core::audit_verify::AuditFilter::default()
                        },
                    );
                    let mut lines = match verification {
                        Ok(result) => vec![format!("Audit entries: {} | Chain valid: {}", result.total_entries, result.valid)],
                        Err(_) => vec!["Audit verification unavailable.".to_string()],
                    };
                    lines.push("Recent activity:".to_string());
                    lines.extend(entries.unwrap_or_default().iter().map(|entry| format!(
                        "{} · {} · {} · {}", entry.created_at, entry.actor, entry.action, entry.target
                    )));
                    lines.join("\\n")
                },
                TuiTab::Health => {
                    let ingestion = logforge_core::stats::ingestion_stats(service.db());
                    let sources = logforge_core::stats::source_health(service.db()).unwrap_or_default();
                    let rules = logforge_core::query_report::alert_counts_by_rule(service.db()).unwrap_or_default();
                    let mut lines = vec!["Operational health".to_string()];
                    if let Ok(liveness) = logforge_core::stats::source_liveness(
                        service.db(),
                        &chrono::Utc::now().to_rfc3339(),
                        300,
                    ) {
                        lines.push(format!(
                            "Source freshness: active={} stale={} never_seen={} disabled={}",
                            liveness.active, liveness.stale, liveness.never_seen, liveness.disabled
                        ));
                    }
                    if let Ok(metrics) = ingestion {
                        lines.push(format!("Received {} · accepted {} · duplicate {:.1}% · malformed {:.1}% · dropped {}", metrics.received, metrics.accepted, metrics.duplicate_rate_percent, metrics.malformed_rate_percent, metrics.dropped));
                    }
                    lines.extend(sources.iter().map(|source| format!("{} [{}] events={} critical={} errors={} last={}", source.name, if source.enabled { "enabled" } else { "disabled" }, source.total_events, source.critical_events, source.error_events, source.last_event_at.as_deref().unwrap_or("never"))));
                    if let Ok(queue) = logforge_core::cases::case_queue_metrics(
                        service.db(),
                        &chrono::Utc::now().to_rfc3339(),
                        24,
                    ) {
                        lines.push(format!(
                            "Cases open={} in_progress={} closed={} overdue={} unassigned={}",
                            queue.open_cases, queue.in_progress_cases, queue.closed_cases,
                            queue.overdue_active, queue.unassigned_active
                        ));
                    }
                    if let Ok(risk) = logforge_core::risk_scoring::score_host_risk(
                        service.db(),
                        "1970-01-01T00:00:00Z",
                        20,
                    ) {
                        lines.push("Host risk (all retained events):".to_string());
                        lines.extend(risk.hosts.iter().map(|host| format!(
                            "{} score={} band={} events={} alerts={}",
                            host.host, host.score, host.band, host.total_events, host.open_alerts
                        )));
                    }
                    if let Ok(workload) = logforge_core::cases::case_workload_by_assignee(service.db()) {
                        lines.push("Active case workload:".to_string());
                        lines.extend(workload.iter().map(|item| format!(
                            "{} active={} critical={} oldest={}",
                            item.assignee, item.active_cases, item.critical_cases,
                            item.oldest_case_at.as_deref().unwrap_or("none")
                        )));
                    }
                    lines.push("Alerts by rule:".to_string());
                    lines.extend(rules.iter().map(|rule| format!("{} total={} open={}", rule.rule_name, rule.total_alerts, rule.open_alerts)));
                    lines.join("\\n")
                }
                TuiTab::About => "LogForge-SIEM v0.1.0-dev\nCopyright (c) 2026 Sonix Devs\nRepository: https://github.com/sonix-devs/logforge-siem".to_string(),
                TuiTab::Settings => {
                    let theme = service.db().get_setting("theme").ok().flatten().unwrap_or_else(|| "dark".into());
                    let accent = service.db().get_setting("accent").ok().flatten().unwrap_or_else(|| "amber".into());
                    let density = service.db().get_setting("density").ok().flatten().unwrap_or_else(|| "compact".into());
                    let scale = service.db().get_setting("text_scale").ok().flatten().unwrap_or_else(|| "100".into());
                    format!("Appearance settings\nTheme: {theme} (default: dark)\nAccent: {accent} (default: amber)\nDensity: {density} (default: compact)\nText scale: {scale}% (default: 100%)\nNetwork: disabled by default; no remote destination configured\nNotifications: local outbox; no delivery adapter configured\n\nPress t to switch dark/light, a to cycle accent, +/- to scale text.")
                }
            };

            let mut records = body
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .filter(|line| filter.is_empty() || line.to_lowercase().contains(&filter.to_lowercase()))
                .map(str::to_string)
                .collect::<Vec<_>>();
            if reverse_sort {
                records.sort_by_key(|line| line.to_lowercase());
            }
            if records.is_empty() {
                selected_row = 0;
                page = 0;
            } else {
                selected_row = selected_row.min(records.len() - 1);
            }
            let page_size = (chunks[1].height as usize).saturating_sub(11).max(1);
            selected_row = selected_row.min(records.len().saturating_sub(1));
            page = if records.is_empty() { 0 } else { selected_row / page_size };
            let pages = records.len().div_ceil(page_size).max(1);
            let first = page * page_size;
            let last = (first + page_size).min(records.len());
            let details_height = if detail_open { Constraint::Length(6) } else { Constraint::Length(0) };
            let areas = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(3), Constraint::Min(3), details_height])
                .split(chunks[1]);
            let filter_label = if filter_active { "FILTER EDIT · type to search · Enter to finish" } else { "Filter: / · move: ↑↓ · page: ←→ · sort: S · columns: C · detail: D · export: E" };
            let status = format!("{} · {} rows · page {}/{} · {} {}", filter_label, records.len(), page + 1, pages, if records.is_empty() { "Empty" } else { "Success" }, export_notice);
            frame.render_widget(Paragraph::new(status).block(Block::default().borders(Borders::ALL)), areas[0]);
            let table_rows = records[first..last]
                .iter()
                .enumerate()
                .map(|(offset, record)| {
                    let cells = table::split_record(record);
                    let mut row = cells.iter().enumerate().filter(|(index, _)| visible_columns[*index]).map(|(_, value)| Cell::from(value.clone())).collect::<Vec<_>>();
                    row.insert(0, Cell::from(format!("{}", first + offset + 1)));
                    Row::new(row)
                })
                .collect::<Vec<_>>();
            let max_columns = records[first..last].iter().map(|record| table::split_record(record).len()).max().unwrap_or(1).min(6);
            let mut widths = vec![Constraint::Length(4)];
            widths.extend((0..max_columns).filter(|index| visible_columns[*index]).map(|_| Constraint::Ratio(1, max_columns.max(1) as u32)));
            let table = Table::new(table_rows, widths)
                .header(Row::new(std::iter::once("#".to_string()).chain((0..max_columns).filter(|index| visible_columns[*index]).map(|index| format!("Field {}", index + 1))).collect::<Vec<_>>()).style(Style::default().fg(accent).add_modifier(Modifier::BOLD)))
                .row_highlight_style(Style::default().fg(accent).add_modifier(Modifier::BOLD))
                .highlight_symbol("▶ ")
                .block(Block::default().borders(Borders::ALL).title(active_tab.label()));
            let local_index = selected_row.saturating_sub(first).min(last.saturating_sub(first).saturating_sub(1));
            let mut widget_state = WidgetTableState::default();
            if first < last {
                widget_state.select(Some(local_index));
            }
            frame.render_stateful_widget(table, areas[1], &mut widget_state);
            let detail = records.get(selected_row).cloned().unwrap_or_else(|| "No records match this view. Load demo data or ingest a source.".to_string());
            if detail_open {
                frame.render_widget(Paragraph::new(detail).wrap(ratatui::widgets::Wrap { trim: true }).block(Block::default().borders(Borders::ALL).title("Selected record detail")), areas[2]);
            }
            if columns_open {
                let chooser = visible_columns.iter().enumerate().map(|(index, visible)| format!("{}:{}", index + 1, if *visible { "shown" } else { "hidden" })).collect::<Vec<_>>().join(" · ");
                frame.render_widget(Paragraph::new(chooser).block(Block::default().borders(Borders::ALL).title("Columns · press 1–6 to toggle")), areas[0]);
            }
            if export_requested {
                let mut csv = String::from("record\n");
                for record in &records {
                    csv.push_str(&table::csv_cell(record));
                    csv.push('\n');
                }
                export_notice = match std::fs::write("logforge-export.csv", csv) {
                    Ok(()) => "Exported filtered rows to logforge-export.csv".to_string(),
                    Err(error) => format!("Export failed: {error}"),
                };
                export_requested = false;
            }
        })?;

        if event::poll(Duration::from_millis(250))? {
            if let Event::Key(key) = event::read()? {
                if filter_active {
                    match key.code {
                        KeyCode::Esc | KeyCode::Enter => filter_active = false,
                        KeyCode::Backspace => {
                            filter.pop();
                            selected_row = 0;
                            page = 0;
                        }
                        KeyCode::Char(value) => {
                            filter.push(value);
                            selected_row = 0;
                            page = 0;
                        }
                        _ => {}
                    }
                } else if columns_open {
                    match key.code {
                        KeyCode::Char(value @ '1'..='6') => {
                            let index = value.to_digit(10).unwrap_or(1) as usize - 1;
                            if let Some(column) = visible_columns.get_mut(index) {
                                *column = !*column;
                            }
                        }
                        KeyCode::Esc | KeyCode::Char('C') => columns_open = false,
                        _ => {}
                    }
                } else {
                    match key.code {
                        KeyCode::Char('t') if active_tab == TuiTab::Settings => {
                            let current = service
                                .db()
                                .get_setting("theme")
                                .ok()
                                .flatten()
                                .unwrap_or_else(|| "dark".into());
                            let next = if current == "dark" { "light" } else { "dark" };
                            let _ = service.db().set_setting(
                                "theme",
                                next,
                                &chrono::Utc::now().to_rfc3339(),
                            );
                        }
                        KeyCode::Char('a') if active_tab == TuiTab::Settings => {
                            let current = service
                                .db()
                                .get_setting("accent")
                                .ok()
                                .flatten()
                                .unwrap_or_else(|| "amber".into());
                            let next = match current.as_str() {
                                "amber" => "blue",
                                "blue" => "teal",
                                _ => "amber",
                            };
                            let _ = service.db().set_setting(
                                "accent",
                                next,
                                &chrono::Utc::now().to_rfc3339(),
                            );
                        }
                        KeyCode::Char('+') | KeyCode::Char('=')
                            if active_tab == TuiTab::Settings =>
                        {
                            let current = service
                                .db()
                                .get_setting("text_scale")
                                .ok()
                                .flatten()
                                .and_then(|value| value.parse::<u8>().ok())
                                .unwrap_or(100);
                            let next = current.saturating_add(10).min(150);
                            let _ = service.db().set_setting(
                                "text_scale",
                                &next.to_string(),
                                &chrono::Utc::now().to_rfc3339(),
                            );
                        }
                        KeyCode::Char('-') if active_tab == TuiTab::Settings => {
                            let current = service
                                .db()
                                .get_setting("text_scale")
                                .ok()
                                .flatten()
                                .and_then(|value| value.parse::<u8>().ok())
                                .unwrap_or(100);
                            let next = current.saturating_sub(10).max(80);
                            let _ = service.db().set_setting(
                                "text_scale",
                                &next.to_string(),
                                &chrono::Utc::now().to_rfc3339(),
                            );
                        }
                        KeyCode::Char('q') => break,
                        KeyCode::Esc if detail_open => detail_open = false,
                        KeyCode::Esc => break,
                        KeyCode::Char('/') => filter_active = true,
                        KeyCode::Char('S') => reverse_sort = !reverse_sort,
                        KeyCode::Char('C') => columns_open = true,
                        KeyCode::Char('d') => detail_open = !detail_open,
                        KeyCode::Char('e') => export_requested = true,
                        KeyCode::Up => selected_row = selected_row.saturating_sub(1),
                        KeyCode::Down => selected_row = selected_row.saturating_add(1),
                        KeyCode::Left => {
                            page = page.saturating_sub(1);
                            selected_row = page.saturating_mul(20);
                        }
                        KeyCode::Right => {
                            page = page.saturating_add(1);
                            selected_row = page.saturating_mul(20);
                        }
                        KeyCode::Char('1') => {
                            active_tab = TuiTab::Overview;
                            page = 0;
                            selected_row = 0;
                        }
                        KeyCode::Char('2') => {
                            active_tab = TuiTab::Search;
                            page = 0;
                            selected_row = 0;
                        }
                        KeyCode::Char('3') => {
                            active_tab = TuiTab::SavedQueries;
                            page = 0;
                            selected_row = 0;
                        }
                        KeyCode::Char('4') => {
                            active_tab = TuiTab::Sources;
                            page = 0;
                            selected_row = 0;
                        }
                        KeyCode::Char('5') => {
                            active_tab = TuiTab::Schema;
                            page = 0;
                            selected_row = 0;
                        }
                        KeyCode::Char('6') => {
                            active_tab = TuiTab::Detections;
                            page = 0;
                            selected_row = 0;
                        }
                        KeyCode::Char('7') => {
                            active_tab = TuiTab::Alerts;
                            page = 0;
                            selected_row = 0;
                        }
                        KeyCode::Char('c') => {
                            active_tab = TuiTab::Cases;
                            page = 0;
                            selected_row = 0;
                        }
                        KeyCode::Char('8') => {
                            active_tab = TuiTab::Retention;
                            page = 0;
                            selected_row = 0;
                        }
                        KeyCode::Char('9') => {
                            active_tab = TuiTab::Audit;
                            page = 0;
                            selected_row = 0;
                        }
                        KeyCode::Char('h') => {
                            active_tab = TuiTab::Health;
                            page = 0;
                            selected_row = 0;
                        }
                        KeyCode::Char('0') => {
                            active_tab = TuiTab::About;
                            page = 0;
                            selected_row = 0;
                        }
                        KeyCode::Char('s') => {
                            active_tab = TuiTab::Settings;
                            page = 0;
                            selected_row = 0;
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}
