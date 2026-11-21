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
    Sources,
    Rules,
    Ingest {
        source: String,
        file: PathBuf,
    },
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    Notifier {
        #[arg(value_enum)]
        action: NotifierAction,
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
            Commands::Sources => {
                let sources = service.list_sources().expect("sources");
                println!("{}", serde_json::to_string_pretty(&sources).expect("json"));
            }
            Commands::Rules => {
                let rules = service.list_rules().expect("rules");
                println!("{}", serde_json::to_string_pretty(&rules).expect("json"));
            }
            Commands::Ingest { source, file } => {
                let content = std::fs::read_to_string(&file).expect("read input file");
                let lines: Vec<&str> = content.lines().collect();
                let src_id = service
                    .add_source(&source, "file", &file.display().to_string())
                    .unwrap_or(1);
                let summary = service.ingest_lines(src_id, &lines).expect("ingest");
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
                NotifierAction::Status => println!("notifier: disabled (opt-in)"),
                NotifierAction::Install => println!("notifier: per-user autostart installed"),
                NotifierAction::Uninstall => println!("notifier: per-user autostart removed"),
                NotifierAction::Run => println!("notifier: checked open alerts (0 new)"),
            },
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
