use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::rc::Rc;

use app_core::clock::{Clock, Zone};
use app_core::launcher::MODE_TUI;
use app_core::meta;
use app_core::notifier::Autostart;
use app_core::platform::{Env, Os};
use app_core::records::{Input, Status};
use app_core::service::{self, App, NAV_ABOUT, NAV_NOTIFICATIONS, NAV_OVERVIEW, NAV_RECORDS, NAV_SETTINGS, NAVIGATION};
use app_core::settings;
use app_core::testkit::{TempDir, fixed_time};
use app_core::theme::Severity;
use app_core::userconfig::FRONT_END_TUI;
use egui::{Event, Key, Modifiers};

use crate::model::{AboutTab, Dashboard, NotifierState, Options, Shortcut};
use crate::render::{Picture, Stage};
use crate::shots;

pub(crate) fn environment(home: &TempDir) -> Env {
    let directory = home.path().to_string_lossy().into_owned();
    Env::from_pairs(&[("HOME", directory.as_str())])
}

pub(crate) fn open_app(home: &TempDir) -> App {
    App::open(service::Options {
        os: Os::Linux,
        env: environment(home),
        config_dir: None,
        database_path: None,
        overrides: BTreeMap::new(),
        clock: Clock::fixed(fixed_time()),
        zone: Zone::Utc,
        read_only: false,
    })
    .unwrap()
}

pub(crate) fn options(home: &TempDir) -> Options {
    Options {
        env: environment(home),
        system_dark: false,
        autostart: None,
        in_terminal: false,
        open_terminal: None,
        open_url: None,
    }
}

fn tints(picture: &Picture) -> usize {
    picture.pixels.as_chunks::<4>().0.iter().collect::<BTreeSet<_>>().len()
}

#[test]
fn every_main_screen_renders_to_a_picture() {
    let home = TempDir::new("gui-screens");
    let mut app = open_app(&home);
    let pictures = shots::screens(&mut app, options(&home));
    let names: Vec<&str> = pictures.iter().map(|(name, _)| *name).collect();
    assert_eq!(names, shots::NAMES);
    for (name, picture) in &pictures {
        assert!(picture.width >= 400 && picture.height >= 200, "{name} is {}x{}", picture.width, picture.height);
        assert!(tints(picture) > 24, "{name} shows only {} colors", tints(picture));
    }
    let light = &pictures.iter().find(|(name, _)| *name == "settings").unwrap().1;
    let dark = &pictures.iter().find(|(name, _)| *name == "settings-dark").unwrap().1;
    let brightness = |picture: &Picture| {
        picture
            .pixels
            .as_chunks::<4>()
            .0
            .iter()
            .map(|pixel| u64::from(pixel[0]) + u64::from(pixel[1]) + u64::from(pixel[2]))
            .sum::<u64>()
    };
    assert!(brightness(dark) * 2 < brightness(light));
}

type Log = Rc<RefCell<Vec<String>>>;

fn wired(home: &TempDir, opened: &Log, terminals: &Log) -> Options {
    let (links, windows) = (opened.clone(), terminals.clone());
    Options {
        autostart: Some(Autostart {
            os: Os::Linux,
            home_dir: home.path().to_string_lossy().into_owned(),
            config_home: String::new(),
            app_data: String::new(),
            executable: "/opt/app/templateapp".to_string(),
        }),
        open_terminal: Some(Box::new(move |arguments: &[String]| {
            windows.borrow_mut().push(arguments.join(" "));
            Ok(())
        })),
        open_url: Some(Box::new(move |url: &str| {
            links.borrow_mut().push(url.to_string());
            Ok(())
        })),
        ..options(home)
    }
}

fn said(board: &Dashboard<'_>) -> String {
    let mut text = board.notice().map(|(_, text)| text.to_string()).unwrap_or_default();
    if let Some(form) = &board.form {
        text = format!("{text}\n{}\n{}", form.title, form.problem);
        for field in &form.fields {
            text = format!("{text}\n{}\n{}", field.label, field.help);
        }
    }
    if let Some(confirm) = &board.confirm {
        text = format!("{text}\n{}\n{}\n{}", confirm.title, confirm.statement, confirm.accept);
    }
    text
}

fn shows(board: &Dashboard<'_>, parts: &[&str]) {
    let text = said(board);
    for part in parts {
        assert!(text.contains(part), "the window lacks {part:?}:\n{text}");
    }
}

fn fill(board: &mut Dashboard<'_>, values: &[(&str, &str)]) {
    let form = board.form.as_mut().expect("no form is open");
    for (key, value) in values {
        form.set(key, value);
    }
}

fn started<'a>(app: &'a mut App, options: Options) -> Dashboard<'a> {
    let mut board = Dashboard::new(app, options);
    board.answer(false);
    board
}

fn input(name: &str, severity: Severity) -> Input {
    Input { name: name.to_string(), status: Status::Open, severity, note: String::new(), enabled: true }
}

#[test]
fn the_first_start_welcomes_and_the_overview_counts() {
    let home = TempDir::new("gui-welcome");
    let mut app = open_app(&home);
    let mut board = Dashboard::new(&mut app, options(&home));
    shows(&board, &[&format!("Welcome to {}", meta::APP_NAME), "A new database was created at", "Load demo data"]);
    board.answer(true);
    shows(&board, &["Added 6 demo records"]);
    assert_eq!(
        (board.overview.open, board.overview.urgent, board.overview.unread_notifications, board.overview.disabled),
        (4, 2, 2, 1)
    );
    assert_eq!(board.overview.demo, 6);
    let bell = NAVIGATION.iter().find(|item| item.id == NAV_NOTIFICATIONS).unwrap();
    assert_eq!(board.nav_label(bell), "Notifications (2)");
    board.load_demo();
    shows(&board, &["The demo data is already loaded"]);
    drop(board);
    let mut again = open_app(&home);
    let board = Dashboard::new(&mut again, options(&home));
    assert!(board.confirm.is_none(), "the welcome dialog appears again for an existing database");
}

#[test]
fn navigation_the_sidebar_and_the_terminal_hand_over_work() {
    let home = TempDir::new("gui-navigation");
    let (opened, terminals) = (Log::default(), Log::default());
    let mut app = open_app(&home);
    let mut board = started(&mut app, wired(&home, &opened, &terminals));
    for (index, item) in NAVIGATION.iter().enumerate() {
        board.shortcut(Shortcut::Page(index));
        assert_eq!(board.page_id(), item.id);
    }
    board.toggle_sidebar();
    assert!(board.collapsed);
    assert_eq!(board.app.settings().get(settings::KEY_SIDEBAR_COLLAPSED).unwrap().raw, "true");
    board.shortcut(Shortcut::Sidebar);
    assert!(!board.collapsed);
    board.shortcut(Shortcut::Guide);
    assert_eq!((board.page_id(), board.about.tab), (NAV_ABOUT, AboutTab::Guide));
    board.show_about(AboutTab::Shortcuts);
    assert_eq!(board.about.tab, AboutTab::Shortcuts);
    board.shortcut(Shortcut::Settings);
    assert_eq!(board.page_id(), NAV_SETTINGS);
    board.shortcut(Shortcut::Add);
    assert_eq!(board.page_id(), NAV_RECORDS);
    shows(&board, &["Add record", "Name"]);
    board.shortcut(Shortcut::Page(0));
    assert_eq!(board.page_id(), NAV_RECORDS, "a shortcut changed the page behind an open form");
    board.cancel();
    board.shortcut(Shortcut::Search);
    assert!(board.focus_search);
    board.refresh();
    shows(&board, &["Refreshed"]);
    board.open_terminal();
    assert_eq!(terminals.borrow().as_slice(), ["--ui tui"]);
    assert!(board.finished());
    assert_eq!(board.next(), "");
    drop(board);

    let mut inside = started(&mut app, Options { in_terminal: true, ..wired(&home, &opened, &terminals) });
    inside.open_terminal();
    assert_eq!(terminals.borrow().len(), 1);
    assert_eq!((inside.finished(), inside.next()), (true, MODE_TUI));
    drop(inside);

    let mut none = started(&mut app, options(&home));
    none.open_terminal();
    shows(&none, &["No terminal window can be opened from here"]);
    assert!(!none.finished());
    drop(none);

    let failing = Options {
        open_terminal: Some(Box::new(|_: &[String]| {
            Err(app_core::error::AppError::Io("no terminal window could be opened".to_string()))
        })),
        ..options(&home)
    };
    let mut board = started(&mut app, failing);
    board.open_terminal();
    shows(&board, &["no terminal window could be opened"]);
    assert!(!board.finished());
    board.shortcut(Shortcut::Quit);
    assert!(board.finished());
}

#[test]
fn a_record_is_added_edited_disabled_filtered_and_removed() {
    let home = TempDir::new("gui-records");
    let mut app = open_app(&home);
    let mut board = started(&mut app, options(&home));
    board.show(NAV_RECORDS);
    assert!(board.empty_records_text().starts_with("No records yet. Choose Add record"));
    board.add_record();
    shows(&board, &["Add record", "Name", "Required, unique"]);
    board.submit();
    shows(&board, &["the name is empty"]);
    fill(&mut board, &[("name", "Core switch"), ("severity", "high"), ("note", "rack 4")]);
    board.submit();
    assert!(board.form.is_none(), "the form stayed open after a successful save: {}", said(&board));
    shows(&board, &["Added record 1: Core switch"]);
    let chosen = board.selected_record().cloned().unwrap();
    let details = board.record_details(&chosen);
    assert!(details.contains("Record 1") && details.contains("Severity: High"), "{details}");
    assert_eq!(chosen.note, "rack 4");
    board.add_record();
    fill(&mut board, &[("name", "Core switch")]);
    board.submit();
    shows(&board, &["already exists"]);
    assert_eq!(board.form.as_ref().unwrap().value("name"), "Core switch");
    fill(&mut board, &[("name", "Edge router")]);
    board.submit();
    shows(&board, &["Added record 2: Edge router"]);
    assert_eq!(board.rec.list.len(), 2);
    board.edit_record();
    shows(&board, &["Edit record 2"]);
    fill(&mut board, &[("status", Status::InProgress.as_str())]);
    board.submit();
    shows(&board, &["Updated record 2"]);
    let chosen = board.selected_record().cloned().unwrap();
    let details = board.record_details(&chosen);
    assert!(details.contains("Status: In progress") && details.contains("Revision: 2"), "{details}");
    board.toggle_record();
    shows(&board, &["Record 2 is disabled"]);
    assert!(board.rec.query.include_disabled, "a disabled record vanished from the list");
    assert!(board.record_details(&board.selected_record().cloned().unwrap()).contains("State: Disabled"));
    board.toggle_record();
    shows(&board, &["Record 2 is enabled"]);
    board.rec.query.search = "rack".to_string();
    board.load_records();
    assert_eq!(board.rec.list.len(), 1);
    assert!(board.selected_record().is_none(), "the detail panel shows a record that is filtered out");
    board.rec.query.search = "zzz".to_string();
    board.load_records();
    assert!(board.empty_records_text().starts_with("No records match these filters"));
    board.clear_filters();
    board.rec.query.status = Some(Status::InProgress);
    board.load_records();
    assert_eq!(board.rec.list.len(), 1);
    board.rec.query.status = None;
    board.rec.query.severity = Some(Severity::High);
    board.load_records();
    assert_eq!(board.rec.list.iter().map(|record| record.name.as_str()).collect::<Vec<_>>(), ["Core switch"]);
    board.clear_filters();
    board.sort_by("name", false);
    assert_eq!(
        board.rec.list.iter().map(|record| record.name.as_str()).collect::<Vec<_>>(),
        ["Core switch", "Edge router"]
    );
    board.rec.selected = Some(2);
    board.copy_record();
    assert!(board.clipboard.as_deref().unwrap_or_default().contains("Edge router"));
    shows(&board, &["Copied record 2 to the clipboard"]);
    board.remove_record();
    shows(&board, &["Remove record", "This removes record 2 (Edge router) permanently."]);
    board.answer(false);
    assert!(board.app.records().get(2).is_ok(), "cancelling removed the record");
    board.shortcut(Shortcut::Remove);
    board.answer(true);
    shows(&board, &["Removed record 2"]);
    assert!(board.app.records().get(2).is_err());
    assert_eq!(board.rec.list.len(), 1);
    assert!(board.selected_record().is_none(), "the detail panel of a removed record is still open");
}

#[test]
fn an_edit_that_lost_a_race_keeps_the_form_and_explains() {
    let home = TempDir::new("gui-conflict");
    let mut app = open_app(&home);
    let other = open_app(&home);
    let mut board = started(&mut app, options(&home));
    let created = board.app.records().create(&input("shared", Severity::Low)).unwrap();
    board.refresh();
    board.show(NAV_RECORDS);
    board.rec.selected = Some(created.id);
    board.edit_record();
    fill(&mut board, &[("name", "shared mine")]);
    let mut changed = created.input();
    changed.note = "from the other window".to_string();
    other.records().update(created.id, created.revision, &changed).unwrap();
    board.tick();
    assert_eq!(board.form.as_ref().unwrap().value("name"), "shared mine", "an external change discarded unsaved input");
    board.submit();
    shows(&board, &["another window changed this record"]);
    let stored = board.app.records().get(created.id).unwrap();
    assert_eq!((stored.name.as_str(), stored.note.as_str()), ("shared", "from the other window"));
    board.cancel();
    board.edit_record();
    assert_eq!(board.form.as_ref().unwrap().value("note"), "from the other window");
    fill(&mut board, &[("name", "shared mine")]);
    board.submit();
    shows(&board, &["Updated record 1"]);
}

#[test]
fn changes_from_another_window_keep_the_selection_and_unsaved_input() {
    let home = TempDir::new("gui-external");
    let mut app = open_app(&home);
    let other = open_app(&home);
    let mut board = started(&mut app, options(&home));
    for name in ["alpha", "beta", "gamma"] {
        other.records().create(&input(name, Severity::Critical)).unwrap();
    }
    board.show(NAV_RECORDS);
    board.tick();
    shows(&board, &["Updated by another window"]);
    assert_eq!(board.rec.list.len(), 3);
    let bell = NAVIGATION.iter().find(|item| item.id == NAV_NOTIFICATIONS).unwrap();
    assert_eq!(board.nav_label(bell), "Notifications (3)");
    board.sort_by("name", false);
    board.rec.selected = board.rec.list.iter().find(|record| record.name == "beta").map(|record| record.id);
    board.show(NAV_SETTINGS);
    board.set.drafts.insert(settings::KEY_TEXT_SCALE.to_string(), "12".to_string());
    board.set.dirty.insert(settings::KEY_TEXT_SCALE.to_string());
    other.records().create(&input("aaa first", Severity::Info)).unwrap();
    other.settings().set(settings::KEY_THEME, "green").unwrap();
    board.tick();
    assert_eq!(board.selected_record().map(|record| record.name.as_str()), Some("beta"));
    assert_eq!(board.set.drafts[settings::KEY_TEXT_SCALE], "12", "an external change discarded unsaved input");
    assert_eq!(board.tokens.name, "green");
    assert_eq!(board.set.drafts[settings::KEY_THEME], "green");
    shows(&board, &["Updated by another window"]);
}

#[test]
fn records_are_imported_through_a_preview_and_exported() {
    let home = TempDir::new("gui-transfer");
    let mut app = open_app(&home);
    let mut board = started(&mut app, options(&home));
    board.show(NAV_RECORDS);
    let source = home.path().join("in.csv");
    fs::write(&source, "name,severity,enabled\nalpha,high,true\nbeta,nope,true\ngamma,low,false\n").unwrap();
    board.import_records();
    shows(&board, &["Import records", "Nothing is written until you confirm the preview"]);
    board.submit();
    shows(&board, &["choose the file to import"]);
    fill(&mut board, &[("path", &home.path().join("missing.csv").to_string_lossy())]);
    board.submit();
    shows(&board, &["cannot open the import file"]);
    fill(&mut board, &[("path", &source.to_string_lossy())]);
    board.submit();
    shows(&board, &["Import preview", "would have created 2, updated 0, unchanged 0, rejected 1", "Rejected row 3"]);
    assert_eq!(board.app.records().count().unwrap(), 0, "the preview wrote records");
    board.answer(true);
    shows(&board, &["Import finished: created 2"]);
    assert_eq!(board.rec.list.len(), 2);
    let target = home.path().join("out.json");
    let target_text = target.to_string_lossy().into_owned();
    board.export_records();
    shows(&board, &["Export records", "readable only by your user account"]);
    fill(&mut board, &[("path", &target_text)]);
    board.submit();
    shows(&board, &[&format!("Exported 2 records to {target_text}")]);
    board.export_records();
    fill(&mut board, &[("path", &target_text)]);
    board.submit();
    shows(&board, &["already exists", "Overwrite it"]);
    fill(&mut board, &[("overwrite", "true"), ("format", "csv")]);
    board.submit();
    assert!(board.form.is_none(), "{}", said(&board));
    assert!(fs::read_to_string(&target).unwrap().starts_with("name,status"));
}

#[test]
fn notifications_are_read_and_rules_are_edited_and_reset() {
    let home = TempDir::new("gui-inbox");
    let mut app = open_app(&home);
    let mut board = Dashboard::new(&mut app, options(&home));
    board.answer(true);
    board.show(NAV_NOTIFICATIONS);
    assert_eq!(board.inbox_summary(), "2 shown, 2 unread");
    board.mark_read();
    shows(&board, &["Select a notification first"]);
    board.inbox.selected = Some(board.inbox.list[0].id);
    board.mark_read();
    shows(&board, &["Marked the notification as read"]);
    assert_eq!(board.inbox_summary(), "2 shown, 1 unread");
    board.inbox.unread_only = true;
    board.load_inbox();
    assert_eq!(board.inbox_summary(), "1 shown, 1 unread");
    board.mark_all_read();
    shows(&board, &["Marked 1 notifications as read"]);
    assert!(board.empty_inbox_text().starts_with("No unread notifications"));
    board.inbox.unread_only = false;
    board.load_inbox();
    let item = board.inbox.list[1].clone();
    board.inbox.selected = Some(item.id);
    board.open_notification_record();
    assert_eq!(board.page_id(), NAV_RECORDS);
    assert_eq!(board.selected_record().map(|record| record.id), Some(item.record_id));
    board.show(NAV_NOTIFICATIONS);
    board.inbox.rules = true;
    board.edit_rule();
    shows(&board, &["Select an event type first"]);
    let event = board.inbox.rule_list[0].event_type.clone();
    board.inbox.rule = Some(event.clone());
    board.edit_rule();
    shows(&board, &["Rule for ", "Minimum severity", "Quiet hours start", "De-duplication"]);
    fill(&mut board, &[("start", "22:00")]);
    board.submit();
    shows(&board, &["both a start and an end"]);
    fill(&mut board, &[("end", "07:00"), ("dedup", "soon")]);
    board.submit();
    shows(&board, &["whole number of minutes"]);
    fill(&mut board, &[("dedup", "15"), ("severity", "critical")]);
    board.submit();
    shows(&board, &["Saved the rule for"]);
    let rule = board.inbox.rule_list[0].clone();
    assert_eq!((rule.quiet_start.as_str(), rule.dedup_minutes, rule.min_severity), ("22:00", 15, Severity::Critical));
    assert!(rule.customized);
    board.reset_rule();
    assert!(!board.inbox.rule_list[0].customized, "the rule was not reset");
}

#[test]
fn settings_are_saved_explained_and_reset() {
    let home = TempDir::new("gui-settings");
    let variable = settings::env_name(settings::KEY_DENSITY);
    let directory = home.path().to_string_lossy().into_owned();
    let env = Env::from_pairs(&[("HOME", directory.as_str()), (variable.as_str(), "compact")]);
    let mut app = App::open(service::Options {
        os: Os::Linux,
        env: env.clone(),
        config_dir: None,
        database_path: None,
        overrides: BTreeMap::new(),
        clock: Clock::fixed(fixed_time()),
        zone: Zone::Utc,
        read_only: false,
    })
    .unwrap();
    let mut board = started(&mut app, Options { env, ..options(&home) });
    board.show(NAV_SETTINGS);
    for definition in settings::DEFINITIONS {
        assert!(board.set.list.iter().any(|value| value.key == definition.key), "{} has no control", definition.key);
    }
    for group in ["Data", "Notifications", "Launcher"] {
        assert!(board.set.groups.iter().any(|name| name == group), "no group {group}");
    }
    let density = board.set.list.iter().find(|value| value.key == settings::KEY_DENSITY).cloned().unwrap();
    let note = board.setting_note(&density);
    assert!(note.contains(&format!("overridden by environment variable {variable}")), "{note}");
    let before = board.tokens.palette.accent;
    board.save_setting(settings::KEY_THEME, "green");
    shows(&board, &["Saved Color theme"]);
    assert_eq!(board.tokens.name, "green");
    assert_ne!(board.tokens.palette.accent, before);
    assert!(board.restyled, "the toolkit style was not rebuilt");
    assert_eq!(open_app(&home).settings().get(settings::KEY_THEME).unwrap().raw, "green");
    board.reset_setting(settings::KEY_THEME);
    assert_eq!(board.tokens.name, "slate");
    let theme = board.set.list.iter().find(|value| value.key == settings::KEY_THEME).unwrap();
    assert!(theme.stored.is_none());
    board.save_setting(settings::KEY_MODE, "dark");
    assert!(board.tokens.dark);
    board.save_setting(settings::KEY_MODE, "light");
    assert!(!board.tokens.dark);
    board.save_setting(settings::KEY_ACCENT, "red");
    assert!(board.set.problems[settings::KEY_ACCENT].contains("#RRGGBB"));
    board.save_setting(settings::KEY_ACCENT, "#FFFF00");
    shows(&board, &["was adjusted"]);
    assert!(!board.set.problems.contains_key(settings::KEY_ACCENT), "the error stayed after a valid value");
    board.save_setting(settings::KEY_DENSITY, "compact");
    shows(&board, &[&format!("Saved Density, but the environment variable {variable} still takes precedence")]);
    board.save_setting(settings::KEY_TEXT_SCALE, "9000");
    let problem = &board.set.problems[settings::KEY_TEXT_SCALE];
    assert!(problem.contains("80") && problem.contains("200"), "{problem}");
    let small = board.tokens.type_scale.body;
    board.save_setting(settings::KEY_TEXT_SCALE, "150");
    assert!(board.tokens.type_scale.body > small);
    board.save_setting(settings::KEY_CONFIRM, "false");
    assert_eq!(board.app.settings().get(settings::KEY_CONFIRM).unwrap().raw, "false");
    board.remember(FRONT_END_TUI);
    assert_eq!(board.app.front_end(), FRONT_END_TUI);
    board.remember("");
    assert_eq!(board.app.front_end(), "");
}

#[test]
fn the_settings_actions_ask_before_they_change_anything() {
    let home = TempDir::new("gui-actions");
    let (opened, terminals) = (Log::default(), Log::default());
    let mut app = open_app(&home);
    let mut board = Dashboard::new(&mut app, wired(&home, &opened, &terminals));
    board.answer(true);
    board.show(NAV_SETTINGS);
    assert_eq!(board.overview.demo, 6);
    board.remove_demo();
    shows(&board, &["This removes 6 demo records. Your own records are not touched."]);
    board.answer(true);
    shows(&board, &["Removed 6 demo records"]);
    assert_eq!(board.overview.demo, 0);
    board.remove_demo();
    shows(&board, &["There is no demo data to remove"]);
    let entry = home.path().join(".config").join("autostart").join(format!("{}-notifier.desktop", meta::SLUG));
    assert_eq!(board.set.notifier, NotifierState::Off);
    board.toggle_notifier();
    shows(&board, &[&format!("This writes {}", entry.display()), "needs no administrator rights"]);
    board.answer(false);
    assert!(!entry.exists(), "cancelling turned the notifier on");
    board.toggle_notifier();
    board.answer(true);
    assert!(entry.exists(), "the entry was not written");
    shows(&board, &["The notifier starts the next time you sign in"]);
    assert_eq!(board.set.notifier, NotifierState::On(entry.display().to_string()));
    board.toggle_notifier();
    assert!(!entry.exists(), "the entry was not removed");
    board.change_database();
    shows(&board, &["Database location", "New location"]);
    board.submit();
    assert!(!board.form.as_ref().unwrap().problem.is_empty());
    let target = home.path().join("elsewhere").join("data.db");
    fill(&mut board, &[("path", &target.to_string_lossy())]);
    board.submit();
    shows(&board, &[&format!("Copy the current database to {}", target.display())]);
    board.answer(true);
    shows(&board, &[&format!("Now using {}", target.display())]);
    assert_eq!(board.app.location().path, target);
    assert_eq!(board.database_path(), target.display().to_string());
}

#[test]
fn about_opens_links_only_on_request_and_shows_the_guide_and_diagnostics() {
    let home = TempDir::new("gui-about");
    let (opened, terminals) = (Log::default(), Log::default());
    let mut app = open_app(&home);
    let mut board = started(&mut app, wired(&home, &opened, &terminals));
    board.show(NAV_ABOUT);
    board.follow(&service::source_link());
    assert_eq!(opened.borrow().as_slice(), [meta::source_url()]);
    shows(&board, &["Opened Source Code in your browser"]);
    board.report_bug();
    shows(&board, &["Report a bug", "prefilled with exactly this text and nothing else", "Operating system:"]);
    let directory = home.path().to_string_lossy().into_owned();
    assert!(!board.confirm.as_ref().unwrap().statement.contains(&directory), "the preview contains a local path");
    assert_eq!(opened.borrow().len(), 1, "the browser opened before the preview was confirmed");
    board.answer(false);
    board.report_bug();
    board.answer(true);
    assert_eq!(opened.borrow().len(), 2);
    assert!(opened.borrow()[1].starts_with(&format!("{}/issues/new?", meta::source_url())));
    board.follow(&service::advisory_link());
    assert!(opened.borrow()[2].ends_with("/security/advisories/new"));
    board.show_about(AboutTab::Guide);
    assert_eq!(board.about.hits.len(), board.about.usage.sections.len());
    board.about.search = "quiet hours".to_string();
    board.filter_guide();
    assert!(!board.about.hits.is_empty() && board.about.hits.len() < board.about.usage.sections.len());
    let first = board.about.hits[0];
    assert!(board.about.usage.sections[first].body.to_lowercase().contains("quiet hours"));
    board.about.search = "zzzzqq".to_string();
    board.filter_guide();
    assert!(board.about.hits.is_empty());
    board.show_about(AboutTab::Diagnostics);
    let report = board.diagnostics_text();
    assert!(report.contains("Schema version") && report.contains("Integrity check"), "{report}");
    board.check_database();
    shows(&board, &["The database is healthy"]);
    board.copy_diagnostics();
    assert!(board.clipboard.as_deref().unwrap_or_default().contains("Schema version"));
    drop(board);
    let mut blocked = started(&mut app, options(&home));
    blocked.follow(&service::source_link());
    shows(&blocked, &[&format!("Copy this link: {}", meta::source_url())]);
}

#[test]
fn the_window_size_is_saved_and_closing_twice_is_harmless() {
    let home = TempDir::new("gui-size");
    let mut app = open_app(&home);
    let mut board = started(&mut app, options(&home));
    let (width, height) = board.window_size();
    assert!(width >= 720.0 && height >= 480.0, "{width}x{height}");
    board.keep_window_size(900.4, 700.0);
    board.close("");
    board.close("");
    assert!(board.finished());
    drop(board);
    let values = open_app(&home).settings().effective().unwrap();
    assert_eq!(values[settings::KEY_WINDOW_WIDTH].int(), 900);
    assert_eq!(values[settings::KEY_WINDOW_HEIGHT].int(), 700);
    assert_eq!(crate::model::window_size(&values), (900.0, 700.0));
}

#[test]
fn reduced_motion_skips_the_notice_fade() {
    let home = TempDir::new("gui-motion");
    let mut app = open_app(&home);
    let mut board = started(&mut app, options(&home));
    board.refresh();
    board.flash_time = 10.0;
    assert!(board.notice_wash(10.1) > 0);
    assert_eq!(board.notice_wash(11.0), 0);
    board.save_setting(settings::KEY_REDUCED_MOTION, "true");
    assert!(!board.tokens.motion);
    board.flash_time = 10.0;
    assert_eq!(board.notice_wash(10.1), 0);
}

fn key(key: Key, modifiers: Modifiers) -> Vec<Event> {
    let event = |pressed| Event::Key { key, physical_key: None, pressed, repeat: false, modifiers };
    vec![event(true), event(false)]
}

#[test]
fn keys_reach_the_dashboard_through_the_window() {
    let home = TempDir::new("gui-keys");
    let mut app = open_app(&home);
    let mut board = started(&mut app, options(&home));
    board.load_demo();
    let mut stage = Stage::new(shots::WIDTH, shots::HEIGHT, 1.0);
    stage.settle(2, |ui| board.ui(ui));
    assert!(!board.restyled);
    stage.frame(key(Key::Num2, Modifiers::COMMAND), |ui| board.ui(ui));
    assert_eq!(board.page_id(), NAV_RECORDS);
    stage.frame(key(Key::F1, Modifiers::NONE), |ui| board.ui(ui));
    assert_eq!((board.page_id(), board.about.tab), (NAV_ABOUT, AboutTab::Guide));
    stage.frame(key(Key::B, Modifiers::COMMAND), |ui| board.ui(ui));
    assert!(board.collapsed);
    stage.frame(key(Key::N, Modifiers::COMMAND), |ui| board.ui(ui));
    assert!(board.form.is_some());
    stage.settle(2, |ui| board.ui(ui));
    stage.frame(key(Key::Escape, Modifiers::NONE), |ui| board.ui(ui));
    stage.settle(2, |ui| board.ui(ui));
    assert!(board.form.is_none(), "Escape did not close the form");
    board.show(NAV_RECORDS);
    board.rec.selected = board.rec.list.first().map(|record| record.id);
    stage.settle(2, |ui| board.ui(ui));
    stage.frame(key(Key::Delete, Modifiers::NONE), |ui| board.ui(ui));
    shows(&board, &["Remove record"]);
    stage.frame(key(Key::Q, Modifiers::COMMAND), |ui| board.ui(ui));
    assert!(board.finished());
    assert_eq!(board.page_id(), NAV_RECORDS);
}

#[test]
fn a_record_table_wider_than_its_space_scrolls_sideways() {
    let home = TempDir::new("gui-scroll");
    let mut app = open_app(&home);
    let mut board = started(&mut app, options(&home));
    board.load_demo();
    board.show(NAV_RECORDS);
    board.rec.selected = board.rec.list.first().map(|record| record.id);
    let mut stage = Stage::new(900.0, 600.0, 1.0);
    let before = stage.settle(3, |ui| board.ui(ui));
    let over = egui::pos2(420.0, 300.0);
    let wheel = Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: egui::vec2(-400.0, 0.0),
        phase: egui::TouchPhase::Move,
        source: egui::MouseWheelSource::Wheel,
        modifiers: Modifiers::NONE,
    };
    stage.frame(vec![Event::PointerMoved(over)], |ui| board.ui(ui));
    stage.frame(vec![wheel], |ui| board.ui(ui));
    let after = stage.settle(8, |ui| board.ui(ui));
    let row = |picture: &Picture| -> Vec<[u8; 4]> { (230..520).map(|x| picture.pixel(x, 300)).collect() };
    assert_ne!(row(&before), row(&after), "the table did not move");
    assert_eq!(board.rec.selected, board.rec.list.first().map(|record| record.id));
}

fn painted(stage: &mut Stage, board: &mut Dashboard<'_>) -> String {
    stage.settle(3, |ui| board.ui(ui));
    stage.text()
}

#[test]
fn every_dashboard_capability_is_reachable() {
    let home = TempDir::new("gui-capabilities");
    let (opened, terminals) = (Log::default(), Log::default());
    let mut app = open_app(&home);
    let mut board = Dashboard::new(&mut app, wired(&home, &opened, &terminals));
    let mut stage = Stage::new(1280.0, 2600.0, 1.0);
    let mut seen: Vec<String> = Vec::new();
    let welcome = painted(&mut stage, &mut board);
    assert!(welcome.contains(&format!("Welcome to {}", meta::APP_NAME)), "the first start shows no dialog:\n{welcome}");
    seen.push("First-start dialog".to_string());
    board.answer(false);
    board.show(NAV_OVERVIEW);
    let empty = painted(&mut stage, &mut board);
    assert!(!empty.contains("Welcome to"), "the dialog stays open:\n{empty}");
    seen.push(empty);
    board.load_demo();
    for item in NAVIGATION {
        board.show(item.id);
        seen.push(painted(&mut stage, &mut board));
    }
    board.show(NAV_RECORDS);
    let first = board.rec.list.first().map(|record| (record.id, record.name.clone())).expect("the demo data is listed");
    for wanted in ["Disable", "Enable"] {
        board.rec.selected = Some(first.0);
        let detail = painted(&mut stage, &mut board);
        assert!(detail.matches(first.1.as_str()).count() >= 2, "no detail panel for {}:\n{detail}", first.1);
        assert!(detail.lines().any(|line| line == wanted), "the detail panel does not offer {wanted}:\n{detail}");
        seen.push("row detail panel".to_string());
        seen.push(detail);
        board.toggle_record();
    }
    board.show(NAV_NOTIFICATIONS);
    board.inbox.rules = true;
    seen.push(painted(&mut stage, &mut board));
    board.inbox.rules = false;
    for tab in [AboutTab::Overview, AboutTab::Guide, AboutTab::Shortcuts, AboutTab::Diagnostics, AboutTab::Licenses] {
        board.show_about(tab);
        seen.push(painted(&mut stage, &mut board));
    }
    board.show(NAV_OVERVIEW);
    stage.settle(2, |ui| board.ui(ui));
    assert!(stage.click("Menu", |ui| board.ui(ui)), "the menu bar has no Menu:\n{}", stage.text());
    seen.push(stage.text());
    let text = seen.join("\n");
    let mut missing = Vec::new();
    for capability in service::CAPABILITIES {
        if capability.dashboard == service::NOT_APPLICABLE {
            continue;
        }
        for part in capability.dashboard.split(", ") {
            if !text.contains(part) {
                missing.push(format!("{}: {part}", capability.id));
            }
        }
    }
    assert!(missing.is_empty(), "not found in the dashboard: {missing:?}\n{text}");
}
