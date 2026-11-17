use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fs;
use std::rc::Rc;
use std::time::Duration;

use app_core::clock::{Clock, Zone};
use app_core::findings::{Scope, Status};
use app_core::meta;
use app_core::notifier::Autostart;
use app_core::platform::{Env, Os};
use app_core::scans;
use app_core::service::{self, App, NAV_ABOUT, NAV_FINDINGS, NAV_SCANS, NAV_SETTINGS, NAVIGATION};
use app_core::settings;
use app_core::testkit::{TempDir, fixed_time, sample_hosting_token};
use app_core::userconfig::FRONT_END_GUI;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::style::{Color, Modifier};

use crate::model::{CANCELLED, MIN_HEIGHT, MIN_WIDTH, short_hints};
use crate::screen::{convert, convert_style, dark_background, draw};
use crate::text::width;
use crate::{Key, Model, Notice, Options};

type Opened = Rc<RefCell<Vec<String>>>;

fn environment(home: &TempDir, extra: &[(&str, &str)]) -> Env {
    let directory = home.path().to_string_lossy().into_owned();
    let mut pairs = vec![("HOME", directory.as_str()), ("TERM", "xterm-256color")];
    pairs.extend_from_slice(extra);
    Env::from_pairs(&pairs)
}

fn open_app(home: &TempDir) -> App {
    App::open(service::Options {
        os: Os::Linux,
        env: environment(home, &[]),
        config_dir: None,
        database_path: None,
        overrides: BTreeMap::new(),
        clock: Clock::fixed(fixed_time()),
        zone: Zone::Utc,
        read_only: false,
    })
    .unwrap()
}

fn options(home: &TempDir, dashboard: bool, opened: Option<Opened>) -> Options {
    Options {
        env: environment(home, &[]),
        system_dark: true,
        autostart: Some(Autostart {
            os: Os::Linux,
            home_dir: home.path().to_string_lossy().into_owned(),
            config_home: String::new(),
            app_data: String::new(),
            executable: "/opt/app/secretsentry".to_string(),
        }),
        has_dashboard: dashboard,
        open_url: opened.map(|log| -> Box<crate::model::UrlOpener> {
            Box::new(move |url: &str| {
                log.borrow_mut().push(url.to_string());
                Ok(())
            })
        }),
    }
}

fn press(model: &mut Model<'_>, keys: &[Key]) {
    for key in keys {
        model.handle(*key);
    }
}

fn write(model: &mut Model<'_>, text: &str) {
    for letter in text.chars() {
        model.handle(Key::Char(letter));
    }
}

fn replace(model: &mut Model<'_>, text: &str) {
    model.handle(Key::Ctrl('u'));
    write(model, text);
}

fn message(model: &Model<'_>) -> (Notice, String) {
    let (kind, text) = model.notice();
    (kind, text.to_string())
}

fn problem(model: &Model<'_>) -> String {
    model.form.as_ref().map(|(form, _)| form.problem.clone()).unwrap_or_default()
}

fn select_setting(model: &mut Model<'_>, label: &str) {
    model.open(NAV_SETTINGS);
    model.set.cursor = model.set.rows.iter().position(|row| row.label == label).unwrap();
}

fn setting_value(model: &Model<'_>, label: &str) -> String {
    model.set.rows.iter().find(|row| row.label == label).unwrap().value.clone()
}

fn project(home: &TempDir) -> String {
    let root = home.path().join("project");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("deploy.sh"), format!("export GITHUB_TOKEN={}\n", sample_hosting_token())).unwrap();
    root.to_string_lossy().into_owned()
}

fn expect(screen: &str, wanted: &[&str]) {
    for part in wanted {
        assert!(screen.contains(part), "missing {part:?} in\n{screen}");
    }
}

#[test]
fn the_first_start_welcomes_and_offers_demo_data() {
    let home = TempDir::new("tui-welcome");
    let mut app = open_app(&home);
    let mut model = Model::new(&mut app, options(&home, false, None));
    let screen = model.screen();
    assert!(screen.contains("Welcome to SecretSentry"), "{screen}");
    assert!(screen.contains("Press l to load labeled demo data and look around."), "{screen}");
    model.handle(Key::Char('l'));
    let loaded = model.app.findings().count().unwrap();
    assert_eq!(loaded, 8);
    let expected = format!("Added {loaded} demo findings. Remove them in Settings, Remove demo data.");
    assert_eq!(message(&model), (Notice::Success, expected));
    let screen = model.screen();
    expect(
        &screen,
        &[
            "What needs attention",
            "In the baseline",
            "Last scan: ",
            "Findings that need attention by rule and confidence",
            "Files with the most findings",
            "Demo data is loaded",
            "8 Notifications (",
            "✔ Added",
        ],
    );
    model.handle(Key::Char('l'));
    assert_eq!(message(&model), (Notice::Success, "The demo data is already loaded".to_string()));
    model.handle(Key::Enter);
    assert_eq!(model.page_id(), NAV_FINDINGS);
    assert_eq!(model.find.query.scope, Scope::Attention);
    assert_eq!(model.find.list.len() as i64, model.overview.attention);
    drop(model);
    let mut model = Model::new(&mut app, options(&home, false, None));
    assert!(!model.screen().contains("Welcome to"));
}

#[test]
fn a_directory_is_scanned_in_the_background_and_the_secret_stays_redacted() {
    let home = TempDir::new("tui-scan");
    let mut app = open_app(&home);
    let root = project(&home);
    let mut model = Model::new(&mut app, options(&home, false, None));
    model.resize(150, 40);
    press(&mut model, &[Key::Esc, Key::Char('2')]);
    assert!(model.screen().contains("No scans yet. Press n to scan a directory"));
    model.handle(Key::Char('x'));
    assert_eq!(message(&model), (Notice::Info, "No scan is running".to_string()));
    let idle = model.interval();
    press(&mut model, &[Key::Char('n'), Key::Ctrl('s')]);
    assert!(problem(&model).starts_with("path: no directory was given"), "{}", problem(&model));
    write(&mut model, &home.path().join("missing").to_string_lossy());
    model.handle(Key::Ctrl('s'));
    assert!(problem(&model).contains("cannot be opened"), "{}", problem(&model));
    assert!(model.job.is_none());
    replace(&mut model, &root);
    model.handle(Key::Ctrl('s'));
    assert!(model.job.is_some() && model.form.is_none());
    assert_eq!(model.page_id(), NAV_SCANS);
    assert_eq!(message(&model), (Notice::Info, "The scan is running. Press x to cancel it.".to_string()));
    assert!(model.screen().contains("… Scanning "));
    assert!(model.interval() <= idle);
    model.handle(Key::Char('n'));
    let (kind, text) = message(&model);
    assert!(kind == Notice::Warning && text.starts_with("A scan is already running."), "{text}");
    assert!(model.wait_for_scan(Duration::from_secs(120)));
    let (kind, text) = message(&model);
    assert!(kind == Notice::Success && text.starts_with("Scanned ") && text.ends_with("1 new likely secret"), "{text}");
    assert!(model.job.is_none());
    assert_eq!(model.scan.list.len(), 1);
    let scan = model.scan.list[0].clone();
    assert_eq!((scan.status, scan.findings_seen, scan.findings_new), (scans::Status::Completed, 1, 1));
    model.handle(Key::Enter);
    let screen = model.screen();
    expect(&screen, &[&format!("Scan {}: ", scan.id), "Findings seen 1 · New findings 1", "No problem was reported."]);
    press(&mut model, &[Key::Esc, Key::Char('f')]);
    assert_eq!(model.page_id(), NAV_FINDINGS);
    assert_eq!((model.find.query.scan_id, model.find.list.len()), (scan.id, 1));
    assert!(model.screen().contains(&format!("Scan: {}", scan.id)));
    let secret = sample_hosting_token();
    assert!(!model.screen().contains(&secret[8..]));
    model.handle(Key::Enter);
    let screen = model.screen();
    assert!(!screen.contains(&secret) && !screen.contains(&secret[8..]), "{screen}");
    expect(&screen, &["Why this confidence", "Where it was seen", "deploy.sh", "never stored", "Line 1, column "]);
    press(&mut model, &[Key::Esc, Key::Char('2'), Key::Char('n'), Key::Ctrl('s'), Key::Char('x')]);
    let (kind, text) = message(&model);
    assert!(kind == Notice::Info && text.starts_with("Cancelling the scan."), "{text}");
    assert!(model.screen().contains("… Cancelling the scan of "));
    assert!(model.wait_for_scan(Duration::from_secs(120)));
    let (kind, text) = message(&model);
    assert!(matches!(kind, Notice::Success | Notice::Warning), "{text}");
    assert_eq!(model.scan.list.len(), 2);
    assert!(model.scan.list.iter().all(|scan| scan.status != scans::Status::Running));
    assert_eq!(model.app.findings().count().unwrap(), 1);
    press(&mut model, &[Key::Char('n'), Key::Ctrl('s')]);
    assert!(model.job.is_some());
    model.handle(Key::Char('q'));
    assert!(model.finished());
    model.stop_scan(Duration::from_secs(120));
    assert!(model.job.is_none());
}

#[test]
fn findings_are_filtered_shown_in_detail_and_decided() {
    let home = TempDir::new("tui-findings");
    let mut app = open_app(&home);
    let other = open_app(&home);
    let mut model = Model::new(&mut app, options(&home, false, None));
    model.resize(150, 60);
    press(&mut model, &[Key::Esc, Key::Char('3')]);
    assert!(model.screen().contains("No findings yet. Press 2 and then n to scan a directory"));
    press(&mut model, &[Key::Char('1'), Key::Char('l'), Key::Char('3')]);
    let total = model.find.list.len();
    assert_eq!(total, 8);
    let screen = model.screen();
    expect(&screen, &["Scope: All findings", "CONFIDENCE", "config/settings.py", "Row 1 of 8"]);
    model.handle(Key::Char('f'));
    assert_eq!(model.find.query.scope, Scope::Attention);
    assert_eq!(model.find.list.len() as i64, model.overview.attention);
    assert!(model.find.list.len() < total);
    press(&mut model, &[Key::Char('f'), Key::Char('f'), Key::Char('f')]);
    assert_eq!(model.find.query.scope, Scope::Allowlisted);
    let paths: Vec<&str> = model.find.list.iter().map(|finding| finding.path.as_str()).collect();
    assert_eq!(paths, ["demo-fixtures/payment_test.py"]);
    model.handle(Key::Esc);
    assert_eq!((model.find.query.scope, model.find.list.len()), (Scope::All, total));
    model.handle(Key::Char('/'));
    assert!(model.hints().starts_with("Type to search"));
    write(&mut model, "release.sh");
    assert!(!model.finished());
    model.handle(Key::Enter);
    assert!(!model.find.list.is_empty());
    assert!(model.find.list.iter().all(|finding| finding.path == "deploy/release.sh"));
    let shown = model.find.list.len();
    model.handle(Key::Enter);
    assert!(model.hints().starts_with("e decide"));
    let screen = model.screen();
    expect(
        &screen,
        &[
            &format!("Finding 1 of {shown}: "),
            "Why this confidence",
            "Where it was seen",
            "deploy/release.sh",
            "(demo data)",
            "never stored",
        ],
    );
    let id = model.find.list[0].id;
    press(&mut model, &[Key::Char('e'), Key::Right, Key::Right, Key::Tab]);
    write(&mut model, "Rotated in March");
    model.handle(Key::Ctrl('s'));
    assert_eq!(message(&model), (Notice::Success, format!("Finding {id} is false positive")));
    let decided = model.find.list.iter().find(|finding| finding.id == id).unwrap();
    assert_eq!((decided.status, decided.note.as_str()), (Status::FalsePositive, "Rotated in March"));
    assert!(model.screen().contains("Note: Rotated in March"));
    let current = other.findings().get(id).unwrap();
    other.findings().decide(id, current.revision, Status::Confirmed, "seen elsewhere").unwrap();
    press(&mut model, &[Key::Char('e'), Key::Ctrl('s')]);
    assert!(problem(&model).starts_with("another window changed this finding"), "{}", problem(&model));
    assert_eq!(model.find.list.iter().find(|finding| finding.id == id).unwrap().note, "seen elsewhere");
    model.handle(Key::Esc);
    assert_eq!(message(&model), (Notice::Info, CANCELLED.to_string()));
    press(&mut model, &[Key::Esc, Key::Esc]);
    assert_eq!(model.find.list.len(), total);
    model.handle(Key::Char('s'));
    assert_eq!((model.find.query.sort.as_str(), model.find.query.descending), ("rule", false));
    model.handle(Key::Char('S'));
    assert!(model.screen().contains("Sort: rule descending"));
    press(&mut model, &[Key::Char('v'), Key::Char('v'), Key::Char('v')]);
    assert!(model.screen().contains("Confidence: High"));
    assert!(model.find.list.iter().all(|finding| finding.level() == app_core::confidence::Level::High));
    model.handle(Key::Char('t'));
    assert!(model.find.list.iter().all(|finding| finding.status == Status::Open));
    press(&mut model, &[Key::Esc, Key::Char('B')]);
    assert!(model.confirm.as_ref().unwrap().statement.starts_with("This marks every current finding as known."));
    model.handle(Key::Char('y'));
    let (kind, text) = message(&model);
    assert!(kind == Notice::Success && text.starts_with("Added ") && text.ends_with(" to the baseline"), "{text}");
    assert!(model.overview.baseline > 0 && model.overview.new == 0);
    model.handle(Key::Char('1'));
    assert!(model.screen().contains("In the baseline"));
    press(&mut model, &[Key::Char('3'), Key::Char('R'), Key::Char('y')]);
    let (kind, text) = message(&model);
    assert!(kind == Notice::Success && text.starts_with("Removed ") && text.ends_with(" from the baseline"), "{text}");
    assert_eq!(model.overview.baseline, 0);
    other.findings().accept_baseline("").unwrap();
    model.tick();
    assert_eq!(message(&model), (Notice::Info, "Updated by another window".to_string()));
    assert!(model.overview.baseline > 0);
}

#[test]
fn rules_and_allowlist_entries_are_managed_with_the_keyboard() {
    let home = TempDir::new("tui-rules");
    let mut app = open_app(&home);
    let mut model = Model::new(&mut app, options(&home, false, None));
    model.resize(150, 60);
    press(&mut model, &[Key::Esc, Key::Char('5')]);
    assert_eq!(model.rules.list.len(), 16);
    let id = model.rules.list[0].spec.id.clone();
    model.handle(Key::Char('x'));
    assert_eq!(message(&model), (Notice::Success, format!("Rule {id} is off. It applies from the next scan.")));
    assert!(model.screen().contains("○ off"));
    model.handle(Key::Char('x'));
    assert_eq!(message(&model), (Notice::Success, format!("Rule {id} is on. It applies from the next scan.")));
    model.handle(Key::Enter);
    expect(&model.screen(), &["Pattern", "Secret group", "Starting confidence", "Built-in"]);
    model.handle(Key::Char('e'));
    let (kind, text) = message(&model);
    assert!(kind == Notice::Info && text.starts_with("A built-in rule cannot be edited."), "{text}");
    assert!(model.form.is_none());
    model.handle(Key::Char('x'));
    assert!(model.rules.list.iter().find(|rule| rule.spec.id == id).unwrap().changed);
    assert!(model.screen().contains("Built-in, changed"));
    model.handle(Key::Char('u'));
    assert_eq!(message(&model), (Notice::Success, format!("Rule {id} is back to its built-in definition")));
    let restored = model.rules.list.iter().find(|rule| rule.spec.id == id).unwrap();
    assert!(!restored.changed && restored.spec.enabled);
    model.handle(Key::Char('d'));
    let (kind, text) = message(&model);
    assert!(kind == Notice::Info && text.starts_with("A built-in rule cannot be removed."), "{text}");
    model.handle(Key::Char('n'));
    write(&mut model, "internal-token");
    model.handle(Key::Tab);
    write(&mut model, "Internal token");
    model.handle(Key::Tab);
    write(&mut model, "tok_(");
    model.handle(Key::Ctrl('s'));
    assert!(!problem(&model).is_empty());
    write(&mut model, "[a-z0-9]{12})");
    model.handle(Key::Tab);
    replace(&mut model, "1");
    model.handle(Key::Tab);
    write(&mut model, "tok_, internal");
    model.handle(Key::Tab);
    replace(&mut model, "high");
    model.handle(Key::Ctrl('s'));
    assert!(problem(&model).starts_with("entropy: \"high\" is not an entropy threshold"), "{}", problem(&model));
    replace(&mut model, "2.5");
    press(&mut model, &[Key::Tab, Key::Right, Key::Tab]);
    replace(&mut model, "70");
    model.handle(Key::Ctrl('s'));
    let added = "Added rule internal-token. It applies from the next scan.".to_string();
    assert_eq!(message(&model), (Notice::Success, added));
    assert_eq!(model.rules.list.len(), 17);
    model.rules.cursor = model.rules.list.iter().position(|rule| rule.spec.id == "internal-token").unwrap();
    assert_eq!(model.rules.list[model.rules.cursor].spec.entropy_tenths, 25);
    model.handle(Key::Char('u'));
    let (kind, text) = message(&model);
    assert!(kind == Notice::Info && text.starts_with("Only a built-in rule can be restored."), "{text}");
    model.handle(Key::Char('t'));
    let value = ["tok", "_", "a1b2c3d4e5f6"].concat();
    write(&mut model, &format!("internal = {value}"));
    model.handle(Key::Ctrl('s'));
    let preview = model.preview.as_ref().unwrap();
    assert_eq!(preview.lines[0], "Rule internal-token reports 1 match in the sample.");
    let screen = model.screen();
    assert!(screen.contains("Try rule internal-token") && !screen.contains("a1b2c3d4e5f6"), "{screen}");
    press(&mut model, &[Key::Esc, Key::Char('e')]);
    replace(&mut model, "Internal API token");
    model.handle(Key::Ctrl('s'));
    let saved = "Saved rule internal-token. It applies from the next scan.".to_string();
    assert_eq!(message(&model), (Notice::Success, saved));
    model.handle(Key::Char('d'));
    let statement = model.confirm.as_ref().unwrap().statement.clone();
    assert!(statement.starts_with("This removes the rule internal-token (Internal API token)."), "{statement}");
    model.handle(Key::Char('y'));
    assert_eq!(message(&model), (Notice::Success, "Removed rule internal-token".to_string()));
    assert_eq!(model.rules.list.len(), 16);
    model.handle(Key::Char('6'));
    assert!(model.screen().contains("No allowlist entries yet. Press n to add one."));
    press(&mut model, &[Key::Char('n'), Key::Ctrl('s')]);
    assert!(!problem(&model).is_empty());
    model.handle(Key::Tab);
    write(&mut model, "tests/**");
    press(&mut model, &[Key::Tab, Key::Tab]);
    write(&mut model, "Fixtures");
    model.handle(Key::Ctrl('s'));
    let entry = model.allow.list[0].spec.clone();
    let added = format!("Added allowlist entry {}. It applies from the next scan.", entry.id);
    assert_eq!(message(&model), (Notice::Success, added));
    assert_eq!((entry.pattern.as_str(), entry.note.as_str(), entry.enabled), ("tests/**", "Fixtures", true));
    expect(&model.screen(), &["Path pattern", "tests/**", "every rule", "Row 1 of 1"]);
    model.handle(Key::Char('x'));
    let off = format!("Allowlist entry {} is off. It applies from the next scan.", entry.id);
    assert_eq!(message(&model), (Notice::Success, off));
    press(&mut model, &[Key::Char('e'), Key::Tab]);
    replace(&mut model, "docs/**");
    model.handle(Key::Ctrl('s'));
    let saved = format!("Saved allowlist entry {}. It applies from the next scan.", entry.id);
    assert_eq!(message(&model), (Notice::Success, saved));
    assert_eq!(model.allow.list[0].spec.pattern, "docs/**");
    press(&mut model, &[Key::Char('d'), Key::Char('y')]);
    assert_eq!(message(&model), (Notice::Success, format!("Removed allowlist entry {}", entry.id)));
    assert!(model.allow.list.is_empty());
}

#[test]
fn the_git_history_is_listed_and_findings_are_exported() {
    let home = TempDir::new("tui-export");
    let mut app = open_app(&home);
    let mut model = Model::new(&mut app, options(&home, false, None));
    model.resize(150, 40);
    press(&mut model, &[Key::Esc, Key::Char('4')]);
    assert!(model.screen().contains("No findings in Git history yet."));
    press(&mut model, &[Key::Char('1'), Key::Char('l'), Key::Char('4')]);
    assert!(!model.hist.list.is_empty());
    expect(&model.screen(), &["COMMIT", "SUBJECT", "Add the deployment settings", "Demo Author"]);
    let commit = model.hist.list[model.hist.cursor].clone();
    model.handle(Key::Enter);
    assert_eq!(model.page_id(), NAV_FINDINGS);
    assert_eq!(model.find.query.commit_id, commit.commit_id);
    assert_eq!(model.find.list.len() as i64, commit.findings);
    assert!(model.screen().contains(&format!("Commit: {}", &commit.commit_id[..10])));
    model.handle(Key::Char('7'));
    expect(&model.screen(), &["Export findings", "Findings to export  Needs attention", "SARIF 2.1.0"]);
    let attention = model.overview.attention;
    let report = home.path().join("reports").join("findings.sarif");
    model.handle(Key::Char('o'));
    replace(&mut model, "findings.txt");
    model.handle(Key::Ctrl('s'));
    assert!(problem(&model).contains("is not clear from its name"), "{}", problem(&model));
    replace(&mut model, &report.to_string_lossy());
    model.handle(Key::Ctrl('s'));
    let done = format!("Exported {attention} findings as SARIF 2.1.0 to {}", report.display());
    assert_eq!(message(&model), (Notice::Success, done.clone()));
    assert!(model.screen().contains("✔ Exported"));
    let text = fs::read_to_string(&report).unwrap();
    assert!(text.contains("secretSentryFinding/v1") && text.contains("2.1.0"), "{text}");
    model.handle(Key::Char('o'));
    replace(&mut model, &report.to_string_lossy());
    model.handle(Key::Ctrl('s'));
    assert!(problem(&model).contains("already exists. Choose another name"), "{}", problem(&model));
    press(&mut model, &[Key::Tab, Key::Right, Key::Tab, Key::Tab, Key::Right, Key::Ctrl('s')]);
    assert_eq!(
        message(&model),
        (Notice::Success, format!("Exported {attention} findings as JSON to {}", report.display()))
    );
    assert!(fs::read_to_string(&report).unwrap().contains("secretsentry-findings"));
    model.handle(Key::Char('f'));
    assert_eq!(model.exp.scope, Scope::New);
}

#[test]
fn notifications_are_read_and_rules_are_edited() {
    let home = TempDir::new("tui-inbox");
    let mut app = open_app(&home);
    let mut model = Model::new(&mut app, options(&home, false, None));
    model.resize(120, 30);
    press(&mut model, &[Key::Esc, Key::Char('8')]);
    assert!(model.screen().contains("No notifications. They appear when a scan finds a new likely secret or fails."));
    press(&mut model, &[Key::Char('1'), Key::Char('l'), Key::Char('8')]);
    let unread = model.inbox.list.len();
    assert_eq!(unread, 2);
    let screen = model.screen();
    for wanted in [format!("Showing all notifications · {unread} unread"), "new likely secret".to_string()] {
        assert!(screen.contains(&wanted), "missing {wanted:?} in\n{screen}");
    }
    let first = model.inbox.list[0].clone();
    model.handle(Key::Char('m'));
    assert_eq!(message(&model), (Notice::Success, format!("Marked notification {} as read", first.id)));
    model.handle(Key::Enter);
    assert_eq!(model.page_id(), NAV_SCANS);
    assert!(model.screen().contains(&format!("Scan {}: ", first.record_id)));
    press(&mut model, &[Key::Char('8'), Key::Char('M')]);
    assert_eq!(message(&model), (Notice::Success, format!("Marked {} notifications as read", unread - 1)));
    model.handle(Key::Char('u'));
    assert!(model.screen().contains("No unread notifications. Press u to show all of them."));
    press(&mut model, &[Key::Char('u'), Key::Right]);
    let screen = model.screen();
    assert!(screen.contains("MIN SEVERITY") && screen.contains("▌ Rules"), "{screen}");
    assert!(model.hints().starts_with("e edit rule"));
    let event = model.inbox.rule_list[0].event_type.clone();
    press(&mut model, &[Key::Char('e'), Key::Tab, Key::Tab, Key::Tab, Key::Tab]);
    replace(&mut model, "soon");
    model.handle(Key::Ctrl('s'));
    assert_eq!(problem(&model), "de-duplication needs a whole number of minutes, for example 60");
    replace(&mut model, "30");
    press(&mut model, &[Key::BackTab, Key::BackTab]);
    replace(&mut model, "25:00");
    model.handle(Key::Ctrl('s'));
    assert!(!problem(&model).is_empty());
    replace(&mut model, "");
    model.handle(Key::Ctrl('s'));
    assert_eq!(message(&model), (Notice::Success, format!("Saved the rule for {event}")));
    assert_eq!((model.inbox.rule_list[0].dedup_minutes, model.inbox.rule_list[0].customized), (30, true));
    assert!(model.screen().contains("changed"));
    model.handle(Key::Char('u'));
    assert_eq!(message(&model), (Notice::Success, format!("Rule for {event} is back to its defaults")));
    assert!(!model.inbox.rule_list[0].customized);
    model.handle(Key::Left);
    assert!(model.screen().contains("▌ Inbox"));
}

#[test]
fn settings_are_changed_reset_and_the_actions_ask_first() {
    let home = TempDir::new("tui-settings");
    let mut app = open_app(&home);
    let mut model = Model::new(&mut app, options(&home, false, None));
    model.resize(120, 70);
    press(&mut model, &[Key::Esc, Key::Char(',')]);
    let screen = model.screen();
    for wanted in ["Appearance", "Database location", "Remove demo data", "Notifier at sign-in", "Open with"] {
        assert!(screen.contains(wanted), "missing {wanted:?} in\n{screen}");
    }
    select_setting(&mut model, "Density");
    press(&mut model, &[Key::Enter, Key::Right, Key::Ctrl('s')]);
    assert_eq!(message(&model), (Notice::Success, "Saved Density".to_string()));
    assert_ne!(model.app.settings().text(settings::KEY_DENSITY).unwrap(), "comfortable");
    model.handle(Key::Char('u'));
    assert_eq!(message(&model), (Notice::Success, "Density is back to its default".to_string()));
    assert_eq!(model.app.settings().text(settings::KEY_DENSITY).unwrap(), "comfortable");
    select_setting(&mut model, "Notifier interval");
    model.handle(Key::Enter);
    replace(&mut model, "5");
    model.handle(Key::Ctrl('s'));
    assert!(!problem(&model).is_empty());
    replace(&mut model, "120");
    model.handle(Key::Ctrl('s'));
    assert_eq!(model.app.settings().number(settings::KEY_NOTIFIER_INTERVAL).unwrap(), 120);
    assert_eq!(setting_value(&model, "Notifier interval"), "120");
    select_setting(&mut model, "Remove demo data");
    model.handle(Key::Enter);
    assert_eq!(message(&model), (Notice::Info, "There is no demo data to remove".to_string()));
    press(&mut model, &[Key::Char('1'), Key::Char('l')]);
    select_setting(&mut model, "Remove demo data");
    model.handle(Key::Enter);
    assert!(model.screen().contains("This removes 8 demo findings with their scans."));
    model.handle(Key::Char('y'));
    assert_eq!(message(&model), (Notice::Success, "Removed 8 demo findings".to_string()));
    assert_eq!((model.scan.list.len(), model.find.list.len(), model.allow.list.len()), (0, 0, 0));
    assert_eq!(model.overview.unread_notifications, 0);
    select_setting(&mut model, "Open with");
    model.handle(Key::Enter);
    assert_eq!(message(&model), (Notice::Success, "Open with: Dashboard window".to_string()));
    assert_eq!(model.app.front_end(), FRONT_END_GUI);
    model.handle(Key::Enter);
    assert_eq!(setting_value(&model, "Open with"), "Terminal UI");
    model.handle(Key::Char('u'));
    assert_eq!(message(&model), (Notice::Success, "Open with: Ask every time".to_string()));
    select_setting(&mut model, "Notifier at sign-in");
    assert_eq!(setting_value(&model, "Notifier at sign-in"), "Off");
    model.handle(Key::Enter);
    assert!(model.confirm.as_ref().unwrap().statement.contains("notifier run\" starts when you sign in"));
    model.handle(Key::Char('y'));
    assert_eq!(message(&model), (Notice::Success, "The notifier starts the next time you sign in".to_string()));
    assert_eq!(setting_value(&model, "Notifier at sign-in"), "On");
    press(&mut model, &[Key::Enter, Key::Char('y')]);
    assert_eq!(message(&model), (Notice::Success, "The notifier no longer starts at sign-in".to_string()));
    select_setting(&mut model, "Database location");
    model.handle(Key::Enter);
    model.handle(Key::Ctrl('s'));
    assert!(!problem(&model).is_empty());
    let target = home.path().join("moved").join("data.db");
    write(&mut model, &target.to_string_lossy());
    model.handle(Key::Ctrl('s'));
    assert!(model.confirm.is_some());
    model.handle(Key::Char('y'));
    assert_eq!(message(&model), (Notice::Success, format!("Now using {}", target.display())));
    assert_eq!(model.app.location().path, target);
    assert!(target.exists());
}

#[test]
fn about_opens_links_only_on_request_and_shows_the_guide_and_diagnostics() {
    let home = TempDir::new("tui-about");
    let mut app = open_app(&home);
    let mut model = Model::new(&mut app, options(&home, false, None));
    model.resize(120, 40);
    press(&mut model, &[Key::Esc, Key::Char('9')]);
    let screen = model.screen();
    let publisher = format!("Publisher: {}", meta::PUBLISHER);
    for wanted in [
        publisher.as_str(),
        "License: MIT",
        "Source Code",
        "Report a vulnerability privately",
        "Nothing is sent by the application.",
    ] {
        assert!(screen.contains(wanted), "missing {wanted:?} in\n{screen}");
    }
    model.handle(Key::Enter);
    let (kind, text) = message(&model);
    assert!(
        kind == Notice::Failure && text.starts_with("no browser can be started from here. Copy this link: https://"),
        "{text}"
    );
    drop(model);
    let opened: Opened = Rc::new(RefCell::new(Vec::new()));
    let mut model = Model::new(&mut app, options(&home, false, Some(opened.clone())));
    model.resize(120, 40);
    press(&mut model, &[Key::Char('9'), Key::Enter]);
    assert_eq!(message(&model), (Notice::Success, "Opened Source Code in the browser".to_string()));
    assert_eq!(opened.borrow().as_slice(), [service::source_link().url]);
    press(&mut model, &[Key::Down, Key::Enter]);
    let screen = model.screen();
    assert!(
        screen.contains("Report a bug") && screen.contains("Press Enter to open the report form in the browser"),
        "{screen}"
    );
    assert_eq!(opened.borrow().len(), 1);
    model.handle(Key::Enter);
    assert_eq!(message(&model), (Notice::Success, "Opened Report a bug in the browser".to_string()));
    assert!(opened.borrow()[1].contains("/issues/new?title="), "{:?}", opened.borrow());
    model.handle(Key::Right);
    assert!(model.screen().contains("Search: press / to search"));
    model.handle(Key::Enter);
    assert!(model.screen().contains("Esc back to the contents"));
    press(&mut model, &[Key::Right, Key::Down, Key::PageDown]);
    assert!(model.screen().contains("Esc back to the contents"));
    press(&mut model, &[Key::Esc, Key::Char('/')]);
    write(&mut model, "zzqzz");
    assert!(!model.finished());
    assert!(model.screen().contains("No section mentions these words. Press Esc to show the contents."));
    press(&mut model, &[Key::Esc, Key::Right]);
    assert!(model.screen().contains("Open the menu"));
    model.handle(Key::Right);
    assert!(model.screen().contains("Diagnostics stay on this computer."));
    press(&mut model, &[Key::Right, Key::PageDown, Key::PageDown]);
    assert!(model.screen().contains("▌Licenses"));
    press(&mut model, &[Key::Right, Key::Left, Key::Char('1'), Key::Char('?')]);
    assert_eq!(model.page_id(), NAV_ABOUT);
    assert!(model.screen().contains("▌Usage guide"));
}

#[test]
fn the_menu_the_sidebar_and_quitting_work_from_the_keyboard() {
    let home = TempDir::new("tui-menu");
    let mut app = open_app(&home);
    let mut model = Model::new(&mut app, options(&home, false, None));
    press(&mut model, &[Key::Esc, Key::Char(':')]);
    let screen = model.screen();
    assert!(screen.contains("Menu") && screen.contains("Not available: this build has no dashboard"), "{screen}");
    model.handle(Key::Enter);
    let (kind, text) = message(&model);
    assert!(kind == Notice::Warning && text.starts_with("This build has no dashboard."), "{text}");
    press(&mut model, &[Key::F(10), Key::Down, Key::Down, Key::Down, Key::Down, Key::Enter]);
    assert_eq!(message(&model), (Notice::Success, "Refreshed".to_string()));
    press(&mut model, &[Key::Char(':'), Key::Esc, Key::Tab]);
    assert_eq!(model.page_id(), NAV_SCANS);
    assert!(model.hints().starts_with("n new scan"));
    press(&mut model, &[Key::Char('0')]);
    assert_eq!(model.page_id(), NAV_SETTINGS);
    press(&mut model, &[Key::Char('2'), Key::BackTab]);
    assert!(model.screen().contains("1 Overview"));
    model.handle(Key::Char('b'));
    assert!(!model.screen().contains("1 Overview"));
    assert_eq!(model.app.settings().text(settings::KEY_SIDEBAR_COLLAPSED).unwrap(), "true");
    model.resize(MIN_WIDTH - 1, MIN_HEIGHT);
    assert!(model.screen().contains("needs a terminal of at least 60 columns and 16 rows."));
    model.resize(100, 30);
    press(&mut model, &[Key::Char(':'), Key::Up, Key::Enter]);
    assert!(model.finished() && model.switch_to().is_empty() && model.view().is_empty());
    drop(model);
    let mut model = Model::new(&mut app, options(&home, true, None));
    assert!(!model.screen().contains("1 Overview"));
    press(&mut model, &[Key::Char(':'), Key::Enter]);
    assert!(model.finished());
    assert_eq!(model.switch_to(), FRONT_END_GUI);
    drop(model);
    let mut model = Model::new(&mut app, options(&home, true, None));
    model.handle(Key::Ctrl('c'));
    assert!(model.finished());
}

#[test]
fn every_screen_fills_the_terminal_exactly_at_every_size() {
    let home = TempDir::new("tui-sizes");
    let mut app = open_app(&home);
    let mut model = Model::new(&mut app, options(&home, false, None));
    let check = |model: &mut Model<'_>, what: &str| {
        for (columns, rows) in [(MIN_WIDTH, MIN_HEIGHT), (80, 24), (110, 30), (200, 60)] {
            model.resize(columns, rows);
            let lines = model.view();
            assert_eq!(lines.len(), rows - 1, "{what} at {columns}x{rows}");
            for line in &lines {
                assert_eq!(line.width(), columns, "{what} at {columns}x{rows}: {:?}", line.text());
            }
        }
    };
    check(&mut model, "welcome");
    model.handle(Key::Char('l'));
    check(&mut model, "notice");
    for page in ['1', '2', '3', '4', '5', '6', '7', '8', '9', '0'] {
        model.handle(Key::Char(page));
        check(&mut model, "page");
        model.handle(Key::Enter);
        check(&mut model, "page after Enter");
        model.handle(Key::Esc);
    }
    press(&mut model, &[Key::Char('3'), Key::Enter]);
    check(&mut model, "detail");
    model.handle(Key::PageDown);
    check(&mut model, "scrolled detail");
    press(&mut model, &[Key::Char('e'), Key::Tab]);
    write(
        &mut model,
        " 日本語のテキスト and a very long note that keeps going beyond the width of a narrow terminal window",
    );
    check(&mut model, "form");
    press(&mut model, &[Key::Esc, Key::Char('B')]);
    check(&mut model, "confirm");
    press(&mut model, &[Key::Esc, Key::Char(':')]);
    check(&mut model, "menu");
    press(&mut model, &[Key::Esc, Key::Char('5'), Key::Char('t')]);
    write(&mut model, "nothing to find here");
    model.handle(Key::Ctrl('s'));
    check(&mut model, "trial");
    press(&mut model, &[Key::Esc, Key::Char('8'), Key::Right]);
    check(&mut model, "rules");
    for _ in 0..4 {
        press(&mut model, &[Key::Char('9'), Key::Right]);
        check(&mut model, "about");
    }
    assert_eq!(short_hints("a · b · c", 40), "a · b · c");
    let short = short_hints(&model.hints(), 58);
    assert!(width(&short) <= 58 && short.ends_with("… · ? guide · q quit"), "{short}");
    assert_eq!(short_hints(&model.hints(), 10), "… · ? guide · q quit");
}

#[test]
fn keys_and_styles_are_converted_and_the_screen_is_painted() {
    let plain = |code: KeyCode| KeyEvent::new(code, KeyModifiers::NONE);
    assert_eq!(convert(plain(KeyCode::Char('n'))), Some(Key::Char('n')));
    assert_eq!(convert(KeyEvent::new(KeyCode::Char('C'), KeyModifiers::CONTROL)), Some(Key::Ctrl('c')));
    assert_eq!(convert(KeyEvent::new(KeyCode::Tab, KeyModifiers::SHIFT)), Some(Key::BackTab));
    assert_eq!(convert(plain(KeyCode::BackTab)), Some(Key::BackTab));
    assert_eq!(convert(plain(KeyCode::F(10))), Some(Key::F(10)));
    assert_eq!(convert(plain(KeyCode::Insert)), None);
    assert_eq!(convert(KeyEvent::new_with_kind(KeyCode::Enter, KeyModifiers::NONE, KeyEventKind::Release)), None);
    let home = TempDir::new("tui-paint");
    let backgrounds = [("15;0", true), ("0;15", false), ("0;default;7", false), ("12;8", true), ("unknown", true)];
    for (reported, dark) in backgrounds {
        assert_eq!(dark_background(&environment(&home, &[("COLORFGBG", reported)])), dark, "{reported}");
    }
    assert!(dark_background(&environment(&home, &[])));
    let mut app = open_app(&home);
    let mut model = Model::new(&mut app, options(&home, false, None));
    model.handle(Key::Esc);
    let heading = convert_style(model.style(crate::text::Look::Heading), model.profile());
    assert!(matches!(heading.fg, Some(Color::Indexed(_))), "{heading:?}");
    assert!(heading.add_modifier.contains(Modifier::BOLD));
    let mut terminal = Terminal::new(TestBackend::new(90, 24)).unwrap();
    terminal.draw(|frame| draw(frame, &mut model)).unwrap();
    let buffer = terminal.backend().buffer();
    let line = |y: u16| -> String { (0..90).filter_map(|x| buffer.cell((x, y)).map(|cell| cell.symbol())).collect() };
    assert!(line(0).starts_with("SecretSentry · Overview"), "{}", line(0));
    assert_eq!(line(1), "─".repeat(90));
    assert!(line(2).starts_with("▌1 Overview"), "{}", line(2));
    assert!(line(22).contains(": menu · ? guide · q quit"), "{}", line(22));
    let cell = buffer.cell((0, 0)).unwrap();
    assert_eq!(cell.fg, heading.fg.unwrap());
}

#[test]
fn every_terminal_capability_is_reachable() {
    let home = TempDir::new("tui-capabilities");
    let mut app = open_app(&home);
    let mut model = Model::new(&mut app, options(&home, true, None));
    model.resize(140, 50);
    let welcome = model.screen();
    assert!(welcome.contains("Welcome to"), "{welcome}");
    let mut screens: Vec<String> = Vec::new();
    let mut hints: Vec<String> = Vec::new();
    model.handle(Key::Esc);
    hints.push(model.hints());
    model.handle(Key::Char('l'));
    for index in 0..NAVIGATION.len() {
        let digit = char::from_digit((index as u32 + 1) % 10, 10).unwrap();
        model.handle(Key::Char(digit));
        screens.push(model.screen());
        hints.push(model.hints());
        for _ in 0..3 {
            model.handle(Key::PageDown);
            screens.push(model.screen());
        }
    }
    press(&mut model, &[Key::Char('8'), Key::Right]);
    screens.push(model.screen());
    hints.push(model.hints());
    press(&mut model, &[Key::Left, Key::Char('9')]);
    for _ in 0..6 {
        model.handle(Key::Right);
        screens.push(model.screen());
    }
    press(&mut model, &[Key::Char('1'), Key::Char(':')]);
    screens.push(model.screen());
    let text = screens.join("\n");
    let keys: Vec<&str> = hints.iter().flat_map(|line| line.split(" · ")).collect();
    let mut missing = Vec::new();
    for capability in service::CAPABILITIES {
        if capability.terminal == service::NOT_APPLICABLE || capability.terminal == "First-start screen" {
            continue;
        }
        for part in capability.terminal.split(", ") {
            let found = if part.chars().count() <= 2 || part.contains(" and ") {
                part.split(" and ").all(|key| keys.iter().any(|hint| hint.starts_with(&format!("{key} "))))
            } else {
                text.contains(part) || keys.iter().any(|hint| hint.starts_with(&format!("{part} ")))
            };
            if !found {
                missing.push(format!("{}: {part}", capability.id));
            }
        }
    }
    assert!(missing.is_empty(), "not found in the terminal UI: {missing:?}\nhints: {keys:?}\n{text}");
}
