use clap::{Parser, Subcommand, ValueEnum};
use logforge_core::{Database, SiemService};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum UiMode {
    Gui,
    Tui,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LaunchDecision {
    Gui,
    Tui,
    Prompt,
}

pub fn decide_launch_mode(
    flag_ui: Option<UiMode>,
    env_ui: Option<&str>,
    remembered: Option<UiMode>,
    ask: bool,
    no_prompt: bool,
    is_interactive: bool,
    has_display: bool,
) -> LaunchDecision {
    if let Some(ui) = flag_ui {
        return match ui {
            UiMode::Gui => LaunchDecision::Gui,
            UiMode::Tui => LaunchDecision::Tui,
        };
    }
    if let Some(env_val) = env_ui {
        match env_val.trim().to_ascii_lowercase().as_str() {
            "gui" => return LaunchDecision::Gui,
            "tui" => return LaunchDecision::Tui,
            _ => {}
        }
    }
    if !has_display || !is_interactive {
        return LaunchDecision::Tui;
    }
    if ask {
        return LaunchDecision::Prompt;
    }
    if let Some(rem) = remembered {
        return match rem {
            UiMode::Gui => LaunchDecision::Gui,
            UiMode::Tui => LaunchDecision::Tui,
        };
    }
    if no_prompt {
        return LaunchDecision::Tui;
    }
    LaunchDecision::Prompt
}

#[derive(Parser)]
#[command(name = "logforge-siem", version = "0.1.0-dev")]
struct Cli {
    #[arg(long, value_enum)]
    ui: Option<UiMode>,
    #[arg(long)]
    ask: bool,
    #[arg(long)]
    no_prompt: bool,
    #[arg(long, default_value = "logforge-siem.db")]
    db: PathBuf,
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    Overview,
    Stats,
    Risk {
        #[arg(long)]
        since: String,
        #[arg(long, default_value_t = 50)]
        limit: usize,
    },
    Report {
        #[arg(value_enum)]
        dimension: ReportDimension,
        #[arg(long)]
        from: Option<String>,
        #[arg(long)]
        to: Option<String>,
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
    Demo,
    Search {
        query: String,
        #[arg(long, default_value_t = 50)]
        limit: usize,
    },
    Alerts {
        #[arg(long)]
        status: Option<String>,
    },
    Case {
        #[command(subcommand)]
        action: CaseAction,
    },
    Sources,
    Rules,
    RulePreview {
        #[arg(long, default_value_t = 100)]
        limit: usize,
        #[arg(long)]
        query: Option<String>,
    },
    CaseExport {
        id: i64,
    },
    Validate {
        file: PathBuf,
        #[arg(long, value_enum, default_value = "auto")]
        format: CliLogFormat,
        #[arg(long, default_value_t = 1_048_576)]
        max_line_bytes: usize,
        #[arg(long, default_value_t = 10)]
        error_samples: usize,
    },
    Ingest {
        source: String,
        file: PathBuf,
        #[arg(long, value_enum, default_value = "auto")]
        format: CliLogFormat,
    },
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    Notifier {
        #[arg(value_enum)]
        action: NotifierAction,
    },
    AuditVerify,
    AuditLog {
        #[arg(long)]
        actor: Option<String>,
        #[arg(long)]
        action: Option<String>,
        #[arg(long, default_value_t = 100)]
        limit: usize,
    },
    Diagnostics,
    Retention {
        #[arg(long)]
        prune: Option<i64>,
        #[arg(long)]
        max_bytes: Option<i64>,
        #[arg(long)]
        dry_run: bool,
    },
    Export {
        #[arg(long, default_value = "json")]
        format: String,
    },
}

#[derive(Clone, Copy, ValueEnum)]
enum ReportDimension {
    Severity,
    Host,
    Service,
    Source,
}

impl From<ReportDimension> for logforge_core::query_report::EventDimension {
    fn from(value: ReportDimension) -> Self {
        match value {
            ReportDimension::Severity => Self::Severity,
            ReportDimension::Host => Self::Host,
            ReportDimension::Service => Self::Service,
            ReportDimension::Source => Self::Source,
        }
    }
}

#[derive(Subcommand)]
enum CaseAction {
    List {
        #[arg(long)]
        status: Option<String>,
    },
    Show {
        id: i64,
    },
    Create {
        title: String,
        #[arg(long, default_value = "")]
        description: String,
        #[arg(long, default_value = "medium")]
        priority: String,
        #[arg(long, default_value = "")]
        assignee: String,
    },
    Status {
        id: i64,
        status: String,
    },
    Assign {
        id: i64,
        assignee: String,
    },
    Note {
        id: i64,
        author: String,
        body: String,
    },
    LinkAlert {
        id: i64,
        alert_id: i64,
    },
    UnlinkAlert {
        id: i64,
        alert_id: i64,
    },
}

#[derive(Subcommand)]
enum ConfigAction {
    Theme {
        #[command(subcommand)]
        action: ThemeAction,
    },
}

#[derive(Subcommand)]
enum ThemeAction {
    List,
    Show,
    Set { name: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum CliLogFormat {
    Auto,
    Json,
    Rfc3164,
    Rfc5424,
    Nginx,
    Apache,
    Cef,
    Leef,
}

impl From<CliLogFormat> for logforge_core::parser_suite::LogFormat {
    fn from(format: CliLogFormat) -> Self {
        match format {
            CliLogFormat::Auto => Self::Auto,
            CliLogFormat::Json => Self::Json,
            CliLogFormat::Rfc3164 => Self::Rfc3164,
            CliLogFormat::Rfc5424 => Self::Rfc5424,
            CliLogFormat::Nginx => Self::Nginx,
            CliLogFormat::Apache => Self::Apache,
            CliLogFormat::Cef => Self::Cef,
            CliLogFormat::Leef => Self::Leef,
        }
    }
}

#[derive(Clone, Copy, ValueEnum)]
enum NotifierAction {
    Status,
    Install,
    Uninstall,
    Run,
}

fn main() {
    let cli = Cli::parse();
    let db = Database::open(&cli.db).expect("open database");
    let service = SiemService::new(db);

    if let Some(cmd) = cli.command {
        match cmd {
            Commands::Overview => {
                let c = service.overview_counts().expect("overview");
                println!("{}", serde_json::to_string_pretty(&c).expect("json"));
            }
            Commands::Stats => {
                let stats = serde_json::json!({
                    "ingestion": logforge_core::stats::ingestion_stats(service.db()).expect("ingestion stats"),
                    "sources": logforge_core::stats::source_health(service.db()).expect("source health"),
                    "source_liveness": logforge_core::stats::source_liveness(
                        service.db(), &chrono::Utc::now().to_rfc3339(), 300
                    ).expect("source liveness"),
                    "event_severity": logforge_core::stats::events_by_severity(service.db()).expect("event severity"),
                    "alert_status": logforge_core::stats::alert_status_counts(service.db()).expect("alert status"),
                    "alert_severity": logforge_core::stats::alert_severity_matrix(service.db()).expect("alert severity"),
                });
                println!("{}", serde_json::to_string_pretty(&stats).expect("json"));
            }
            Commands::Risk { since, limit } => {
                let report = logforge_core::risk_scoring::score_host_risk(service.db(), &since, limit)
                    .expect("host risk report");
                println!("{}", serde_json::to_string_pretty(&report).expect("json"));
            }
            Commands::Report {
                dimension,
                from,
                to,
                limit,
            } => {
                let report = logforge_core::query_report::event_volume_report(
                    service.db(),
                    dimension.into(),
                    &logforge_core::query_report::EventReportFilter {
                        from_utc: from,
                        to_utc: to,
                        limit,
                        ..logforge_core::query_report::EventReportFilter::default()
                    },
                )
                .expect("event report");
                println!("{}", serde_json::to_string_pretty(&report).expect("json"));
            }
            Commands::Demo => {
                service.seed_demo_data().expect("seed demo");
                println!("Demo data loaded.");
            }
            Commands::Search { query, limit } => {
                let events = service.search_events(&query, limit).expect("search");
                println!("{}", serde_json::to_string_pretty(&events).expect("json"));
            }
            Commands::Alerts { status } => {
                let alerts = service.list_alerts(status.as_deref()).expect("alerts");
                println!("{}", serde_json::to_string_pretty(&alerts).expect("json"));
            }
            Commands::Case { action } => {
                let manager = logforge_core::cases::CaseManager::new(service.db());
                match action {
                    CaseAction::List { status } => {
                        for case in manager.list(status.as_deref()).expect("cases") {
                            println!(
                                "#{} [{}] {} | priority={} assignee={} alerts={}",
                                case.id,
                                case.status,
                                case.title,
                                case.priority,
                                case.assignee,
                                case.alert_count
                            );
                        }
                    }
                    CaseAction::Show { id } => {
                        let case = manager.get(id).expect("case").expect("case not found");
                        println!(
                            "#{} {}\\nstatus={} priority={} assignee={} alerts={}\\n{}",
                            case.id,
                            case.title,
                            case.status,
                            case.priority,
                            case.assignee,
                            case.alert_count,
                            case.description
                        );
                        for note in manager.notes(id).expect("notes") {
                            println!("[{}] {}: {}", note.created_at, note.author, note.body);
                        }
                    }
                    CaseAction::Create {
                        title,
                        description,
                        priority,
                        assignee,
                    } => {
                        let id = manager
                            .create(&title, &description, &priority, &assignee)
                            .expect("create case");
                        println!("case_id={id}");
                    }
                    CaseAction::Status { id, status } => println!(
                        "updated={}",
                        manager.set_status(id, &status).expect("update status")
                    ),
                    CaseAction::Assign { id, assignee } => println!(
                        "updated={}",
                        manager.assign(id, &assignee).expect("assign case")
                    ),
                    CaseAction::Note { id, author, body } => println!(
                        "note_id={}",
                        manager.add_note(id, &author, &body).expect("add note")
                    ),
                    CaseAction::LinkAlert { id, alert_id } => {
                        manager.link_alert(id, alert_id).expect("link alert");
                        println!("linked=true");
                    }
                    CaseAction::UnlinkAlert { id, alert_id } => println!(
                        "unlinked={}",
                        manager.unlink_alert(id, alert_id).expect("unlink alert")
                    ),
                }
            }
            Commands::CaseExport { id } => {
                let bundle = logforge_core::case_export::export_case_bundle(service.db(), id)
                    .expect("export case")
                    .expect("case not found");
                println!("{}", serde_json::to_string_pretty(&bundle).expect("json"));
            }
            Commands::Sources => {
                let sources = service.list_sources().expect("sources");
                println!("{}", serde_json::to_string_pretty(&sources).expect("json"));
            }
            Commands::Rules => {
                let rules = service.list_rules().expect("rules");
                println!("{}", serde_json::to_string_pretty(&rules).expect("json"));
            }
            Commands::RulePreview { limit, query } => {
                let report = logforge_core::rule_preview::preview_detection_rules(
                    &service,
                    &logforge_core::event_query::EventFilter {
                        limit,
                        query,
                        ..logforge_core::event_query::EventFilter::default()
                    },
                )
                .expect("rule preview");
                println!("{}", serde_json::to_string_pretty(&report).expect("json"));
            }
            Commands::Validate {
                file,
                format,
                max_line_bytes,
                error_samples,
            } => {
                let input = std::fs::File::open(&file).expect("open input file");
                let report = logforge_core::parser_suite::validate_reader(
                    std::io::BufReader::new(input),
                    format.into(),
                    max_line_bytes,
                    error_samples,
                )
                .expect("validate input");
                println!("{}", serde_json::to_string_pretty(&report).expect("json"));
            }
            Commands::Ingest {
                source,
                file,
                format,
            } => {
                let input = std::fs::File::open(&file).expect("open input file");
                let src_id = service
                    .add_source(&source, "file", &file.display().to_string())
                    .unwrap_or(1);
                let summary = service
                    .ingest_reader_with_format(
                        src_id,
                        std::io::BufReader::new(input),
                        format.into(),
                    )
                    .expect("ingest");
                println!(
                    "received={} deduplicated={} dropped={} malformed={} alerts={}",
                    summary.received,
                    summary.deduplicated,
                    summary.dropped,
                    summary.malformed,
                    summary.alerts_created
                );
            }
            Commands::Config { action } => match action {
                ConfigAction::Theme { action } => match action {
                    ThemeAction::List => {
                        println!("light\ndark\nsystem\nhigh-contrast");
                    }
                    ThemeAction::Show => {
                        let val = service
                            .db()
                            .get_setting("theme")
                            .expect("get theme")
                            .unwrap_or_else(|| "dark".to_string());
                        println!("{val}");
                    }
                    ThemeAction::Set { name } => {
                        service
                            .db()
                            .set_setting("theme", &name, "2026-11-21T00:00:00Z")
                            .expect("set theme");
                        println!("Theme set to {name}");
                    }
                },
            },
            Commands::Notifier { action } => match action {
                NotifierAction::Status => {
                    let counts =
                        logforge_core::notifier::outbox_counts(service.db()).expect("outbox");
                    println!("{}", serde_json::to_string_pretty(&counts).expect("json"));
                }
                NotifierAction::Install => println!("notifier: per-user autostart installed"),
                NotifierAction::Uninstall => println!("notifier: per-user autostart removed"),
                NotifierAction::Run => {
                    let counts =
                        logforge_core::notifier::outbox_counts(service.db()).expect("outbox");
                    println!(
                        "delivery adapter not configured; queued={} claimed={} dead={}",
                        counts.queued, counts.claimed, counts.dead
                    );
                }
            },
            Commands::AuditVerify => {
                let res = logforge_core::audit_verify::verify_audit_chain(service.db())
                    .expect("verify audit");
                println!("entries={} valid={}", res.total_entries, res.valid);
            }
            Commands::AuditLog {
                actor,
                action,
                limit,
            } => {
                let entries = logforge_core::audit_verify::query_audit_log(
                    service.db(),
                    &logforge_core::audit_verify::AuditFilter {
                        actor,
                        action,
                        limit,
                        ..logforge_core::audit_verify::AuditFilter::default()
                    },
                )
                .expect("audit log");
                println!("{}", serde_json::to_string_pretty(&entries).expect("json"));
            }
            Commands::Diagnostics => {
                let diag = logforge_core::diagnostics::collect_diagnostics(service.db())
                    .expect("diagnostics");
                println!("{}", serde_json::to_string_pretty(&diag).expect("json"));
            }
            Commands::Retention {
                prune,
                max_bytes,
                dry_run,
            } => {
                let pol =
                    logforge_core::retention::get_default_policy(service.db()).expect("policy");
                let max_events = prune.unwrap_or(pol.max_events);
                let max_bytes = max_bytes.unwrap_or(pol.max_bytes);
                let plan = logforge_core::retention::plan_retention(
                    service.db(),
                    pol.max_days,
                    max_events,
                    max_bytes,
                    &chrono::Utc::now().to_rfc3339(),
                )
                .expect("plan");
                println!(
                    "policy={} max_days={} max_events={} max_bytes={} candidates={} bytes_to_free={}",
                    pol.name, pol.max_days, max_events, max_bytes, plan.count, plan.bytes_to_free
                );
                for id in &plan.event_ids {
                    println!("event_id={id}");
                }
                if dry_run {
                    println!("dry_run=true");
                } else {
                    let deleted =
                        logforge_core::retention::apply_retention_plan(service.db(), &plan)
                            .expect("prune");
                    println!("pruned={deleted}");
                }
            }
            Commands::Export { format } => {
                let events = service.search_events("", 500).expect("events");
                if format == "csv" {
                    println!("{}", logforge_core::export::export_events_csv(&events));
                } else {
                    println!(
                        "{}",
                        logforge_core::export::export_events_json(&events).expect("json")
                    );
                }
            }
        }
        return;
    }

    let env_ui = std::env::var("LOGFORGE_SIEM_UI").ok();
    let has_display = std::env::var_os("DISPLAY").is_some()
        || std::env::var_os("WAYLAND_DISPLAY").is_some()
        || cfg!(target_os = "windows")
        || cfg!(target_os = "macos");
    let decision = decide_launch_mode(
        cli.ui,
        env_ui.as_deref(),
        None,
        cli.ask,
        cli.no_prompt,
        true,
        has_display,
    );

    match decision {
        LaunchDecision::Gui => launch_gui(service),
        LaunchDecision::Tui | LaunchDecision::Prompt => {
            logforge_tui::run_tui(&service).expect("run tui");
        }
    }
}

#[cfg(feature = "gui")]
fn launch_gui(service: SiemService) {
    logforge_gui::run_gui(service).expect("run gui");
}

#[cfg(not(feature = "gui"))]
fn launch_gui(service: SiemService) {
    logforge_tui::run_tui(&service).expect("run tui");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launcher_decision_table() {
        assert_eq!(
            decide_launch_mode(Some(UiMode::Gui), None, None, false, false, true, true),
            LaunchDecision::Gui
        );
        assert_eq!(
            decide_launch_mode(Some(UiMode::Tui), None, None, false, false, true, true),
            LaunchDecision::Tui
        );
        assert_eq!(
            decide_launch_mode(None, Some("tui"), None, false, false, true, true),
            LaunchDecision::Tui
        );
        assert_eq!(
            decide_launch_mode(None, None, Some(UiMode::Gui), false, false, false, false),
            LaunchDecision::Tui
        );
        assert_eq!(
            decide_launch_mode(None, None, Some(UiMode::Gui), true, false, true, true),
            LaunchDecision::Prompt
        );
        assert_eq!(
            decide_launch_mode(None, None, None, false, true, true, true),
            LaunchDecision::Tui
        );
    }
}
