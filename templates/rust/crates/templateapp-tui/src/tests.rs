use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fs;
use std::rc::Rc;

use app_core::clock::{Clock, Zone};
use app_core::notifier::Autostart;
use app_core::platform::{Env, Os};
use app_core::records::{DEMO_RECORDS, Input, Status};
use app_core::service::{self, App, NAV_ABOUT, NAV_RECORDS, NAV_SETTINGS, NAVIGATION};
use app_core::settings;
use app_core::testkit::{TempDir, fixed_time};
use app_core::theme::Severity;
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
            executable: "/opt/app/templateapp".to_string(),
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

#[test]
fn the_first_start_welcomes_and_offers_demo_data() {
    let home = TempDir::new("tui-welcome");
    let mut app = open_app(&home);
    let mut model = Model::new(&mut app, options(&home, false, None));
    let screen = model.screen();
    assert!(screen.contains("Welcome to Template App"), "{screen}");
    assert!(screen.contains("Press l to load labeled demo data and look around."), "{screen}");
    model.handle(Key::Char('l'));
    let expected = format!("Added {} demo records. Remove them in Settings, Remove demo data.", DEMO_RECORDS.len());
    assert_eq!(message(&model), (Notice::Success, expected));
    let screen = model.screen();
    for wanted in ["What needs attention", "Open records", "Demo data is loaded", "3 Notifications (", "✔ Added"] {
        assert!(screen.contains(wanted), "missing {wanted:?} in\n{screen}");
    }
    model.handle(Key::Char('l'));
    assert_eq!(message(&model), (Notice::Success, "The demo data is already loaded".to_string()));
    model.handle(Key::Enter);
    assert_eq!(model.page_id(), NAV_RECORDS);
    drop(model);
    let mut model = Model::new(&mut app, options(&home, false, None));
    assert!(!model.screen().contains("Welcome to"));
}

#[test]
fn a_record_is_added_edited_disabled_and_removed_with_the_keyboard() {
    let home = TempDir::new("tui-records");
    let mut app = open_app(&home);
    let mut model = Model::new(&mut app, options(&home, false, None));
    press(&mut model, &[Key::Esc, Key::Char('2')]);
    assert!(model.screen().contains("No records yet. Press n to add the first record"));
    press(&mut model, &[Key::Char('n'), Key::Ctrl('s')]);
    assert!(problem(&model).starts_with("name:"), "{}", problem(&model));
    assert!(model.screen().contains("✖ Not saved: name:"));
    write(&mut model, "Renew the lease");
    model.handle(Key::Ctrl('s'));
    assert_eq!(message(&model), (Notice::Success, "Added record 1: Renew the lease".to_string()));
    let screen = model.screen();
    assert!(screen.contains("Renew the lease") && screen.contains("Row 1 of 1"), "{screen}");
    press(&mut model, &[Key::Char('e'), Key::Tab, Key::Right, Key::Tab, Key::Right, Key::Ctrl('s')]);
    assert_eq!(message(&model), (Notice::Success, "Updated record 1".to_string()));
    assert_eq!((model.rec.list[0].status, model.rec.list[0].severity), (Status::InProgress, Severity::Low));
    model.handle(Key::Enter);
    let screen = model.screen();
    assert!(screen.contains("Record 1: Renew the lease") && screen.contains("No note."), "{screen}");
    press(&mut model, &[Key::Esc, Key::Char('x')]);
    assert_eq!(message(&model), (Notice::Success, "Record 1 is disabled".to_string()));
    assert!(model.screen().contains("Disabled: shown"));
    model.handle(Key::Char('x'));
    assert_eq!(message(&model), (Notice::Success, "Record 1 is enabled".to_string()));
    model.handle(Key::Char('d'));
    assert!(model.screen().contains("▲ Press y to remove it, or n or Esc to cancel."));
    model.handle(Key::Char('n'));
    assert_eq!(message(&model), (Notice::Info, CANCELLED.to_string()));
    assert_eq!(model.rec.list.len(), 1);
    press(&mut model, &[Key::Char('d'), Key::Char('y')]);
    assert_eq!(message(&model), (Notice::Success, "Removed record 1".to_string()));
    assert!(model.screen().contains("No records yet."));
    assert!(model.app.records().get(1).is_err());
    model.app.settings().set(settings::KEY_CONFIRM, "false").unwrap();
    model.reload();
    press(&mut model, &[Key::Char('n')]);
    write(&mut model, "Short lived");
    press(&mut model, &[Key::Ctrl('s'), Key::Delete]);
    assert_eq!(message(&model), (Notice::Success, "Removed record 2".to_string()));
}

#[test]
fn an_edit_that_lost_a_race_keeps_the_form_open_and_explains() {
    let home = TempDir::new("tui-conflict");
    let mut app = open_app(&home);
    let other = open_app(&home);
    let mut model = Model::new(&mut app, options(&home, false, None));
    press(&mut model, &[Key::Esc, Key::Char('2'), Key::Char('n')]);
    write(&mut model, "Shared");
    press(&mut model, &[Key::Ctrl('s'), Key::Char('e')]);
    let current = other.records().get(1).unwrap();
    other
        .records()
        .update(1, current.revision, &Input { note: "changed elsewhere".to_string(), ..current.input() })
        .unwrap();
    write(&mut model, " edited");
    model.handle(Key::Ctrl('s'));
    assert!(problem(&model).starts_with("another window changed this record"), "{}", problem(&model));
    assert_eq!(model.rec.list[0].note, "changed elsewhere");
    model.handle(Key::Esc);
    assert_eq!(message(&model), (Notice::Info, CANCELLED.to_string()));
    other.records().create(&Input { name: "From the other window".to_string(), ..Input::default() }).unwrap();
    model.tick();
    assert_eq!(message(&model), (Notice::Info, "Updated by another window".to_string()));
    assert_eq!(model.rec.list.len(), 2);
    model.tick();
    assert_eq!(model.rec.list.len(), 2);
}

#[test]
fn records_are_searched_filtered_sorted_and_their_columns_chosen() {
    let home = TempDir::new("tui-filters");
    let mut app = open_app(&home);
    let mut model = Model::new(&mut app, options(&home, false, None));
    press(&mut model, &[Key::Char('l'), Key::Char('2')]);
    let enabled = DEMO_RECORDS.iter().filter(|record| record.enabled).count();
    assert_eq!(model.rec.list.len(), enabled);
    press(&mut model, &[Key::Char('/'), Key::Char('q'), Key::Backspace]);
    assert!(!model.finished());
    assert!(model.hints().starts_with("Type to search"));
    write(&mut model, "router");
    assert_eq!(model.rec.list.len(), 1);
    assert!(model.screen().contains("Search: router▏"));
    press(&mut model, &[Key::Enter, Key::Esc]);
    assert_eq!(model.rec.list.len(), enabled);
    model.handle(Key::Char('f'));
    assert!(model.screen().contains("Status: Open"));
    assert!(model.rec.list.iter().all(|record| record.status == Status::Open));
    press(&mut model, &[Key::Esc, Key::Char('v')]);
    assert!(model.screen().contains("No records match these filters. Press Esc to clear them."));
    press(&mut model, &[Key::Esc, Key::Char('a')]);
    assert_eq!(model.rec.list.len(), DEMO_RECORDS.len());
    press(&mut model, &[Key::End, Key::Char('k'), Key::Home, Key::Char('j')]);
    assert_eq!(model.rec.cursor, 1);
    model.handle(Key::Char('s'));
    let sorted = model.app.settings().text(settings::KEY_RECORD_SORT).unwrap();
    assert!(sorted.starts_with("name:"), "{sorted}");
    model.handle(Key::Char('S'));
    let reversed = model.app.settings().text(settings::KEY_RECORD_SORT).unwrap();
    assert!(reversed.starts_with("name:") && reversed != sorted, "{reversed}");
    let names: Vec<&str> = model.rec.list.iter().map(|record| record.name.as_str()).collect();
    let mut ordered = names.clone();
    ordered.sort_unstable();
    if reversed.ends_with(":desc") {
        ordered.reverse();
    }
    assert_eq!(names, ordered);
    model.handle(Key::Char('c'));
    let hidden = model.form.as_ref().unwrap().0.fields[0].key.clone();
    assert!(model.screen().contains("Columns of the record table"));
    press(&mut model, &[Key::Right, Key::Ctrl('s')]);
    let columns = model.app.settings().text(settings::KEY_RECORD_COLUMNS).unwrap();
    assert!(!columns.split(',').any(|column| column == hidden), "{columns}");
    assert!(!model.screen().contains(&format!("  {}  ", hidden.to_uppercase())));
}

#[test]
fn records_are_exported_and_imported_through_a_preview() {
    let home = TempDir::new("tui-transfer");
    let mut app = open_app(&home);
    let mut model = Model::new(&mut app, options(&home, false, None));
    press(&mut model, &[Key::Char('l'), Key::Char('2'), Key::Char('o')]);
    let target = home.path().join("out.csv").to_string_lossy().into_owned();
    replace(&mut model, &target);
    model.handle(Key::Ctrl('s'));
    let expected = format!("Exported {} records to {target}", DEMO_RECORDS.len());
    assert_eq!(message(&model), (Notice::Success, expected.clone()));
    assert!(fs::read_to_string(&target).unwrap().starts_with("name,status,severity,note,enabled"));
    model.handle(Key::Char('o'));
    replace(&mut model, &target);
    model.handle(Key::Ctrl('s'));
    assert!(problem(&model).contains("already exists. Choose another name"), "{}", problem(&model));
    press(&mut model, &[Key::Tab, Key::Tab, Key::Right, Key::Ctrl('s')]);
    assert_eq!(message(&model), (Notice::Success, expected));
    press(&mut model, &[Key::Char('i'), Key::Ctrl('s')]);
    assert_eq!(problem(&model), "give the path of the file to import");
    write(&mut model, &home.path().join("missing.csv").to_string_lossy());
    model.handle(Key::Ctrl('s'));
    assert!(problem(&model).contains("cannot open the import file"), "{}", problem(&model));
    let source = home.path().join("in.csv").to_string_lossy().into_owned();
    fs::write(&source, "name,status,severity,note,enabled\nImported row,open,high,,true\n,open,low,,true\n").unwrap();
    replace(&mut model, &source);
    model.handle(Key::Ctrl('s'));
    let screen = model.screen();
    for wanted in ["Import preview", "Dry run of", "✖ rejected row", "Press Enter to import these rows"] {
        assert!(screen.contains(wanted), "missing {wanted:?} in\n{screen}");
    }
    press(&mut model, &[Key::Down, Key::PageDown, Key::Up, Key::Esc]);
    assert_eq!(message(&model), (Notice::Info, CANCELLED.to_string()));
    assert!(model.app.records().by_name("Imported row").unwrap().is_none());
    model.handle(Key::Char('i'));
    write(&mut model, &source);
    press(&mut model, &[Key::Ctrl('s'), Key::Enter]);
    let (kind, text) = message(&model);
    assert!(kind == Notice::Success && text.starts_with("Import finished: "), "{text}");
    assert!(model.app.records().by_name("Imported row").unwrap().is_some());
    fs::write(&source, "name,status,severity,note,enabled\n,open,low,,true\n").unwrap();
    model.handle(Key::Char('i'));
    write(&mut model, &source);
    model.handle(Key::Ctrl('s'));
    let screen = model.screen();
    assert!(screen.contains("There is nothing to import.") && screen.contains("Esc close"), "{screen}");
    model.handle(Key::Enter);
    assert!(model.preview.is_none());
}

#[test]
fn notifications_are_read_and_rules_are_edited() {
    let home = TempDir::new("tui-inbox");
    let mut app = open_app(&home);
    let mut model = Model::new(&mut app, options(&home, false, None));
    model.resize(120, 30);
    press(&mut model, &[Key::Esc, Key::Char('3')]);
    assert!(model.screen().contains("No notifications. They appear when a record needs attention or is completed."));
    press(&mut model, &[Key::Char('1'), Key::Char('l'), Key::Char('3')]);
    let unread = model.inbox.list.len();
    assert!(unread >= 1);
    let screen = model.screen();
    for wanted in [format!("Showing all notifications · {unread} unread"), "A record needs attention".to_string()] {
        assert!(screen.contains(&wanted), "missing {wanted:?} in\n{screen}");
    }
    let first = model.inbox.list[0].clone();
    model.handle(Key::Char('m'));
    assert_eq!(message(&model), (Notice::Success, format!("Marked notification {} as read", first.id)));
    model.handle(Key::Enter);
    assert_eq!(model.page_id(), NAV_RECORDS);
    assert!(model.screen().contains(&format!("Record {}: ", first.record_id)));
    press(&mut model, &[Key::Char('3'), Key::Char('M')]);
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
    assert!(model.screen().contains(&format!("This removes {} demo records.", DEMO_RECORDS.len())));
    model.handle(Key::Char('y'));
    assert_eq!(message(&model), (Notice::Success, format!("Removed {} demo records", DEMO_RECORDS.len())));
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
    press(&mut model, &[Key::Esc, Key::Char('5')]);
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
    press(&mut model, &[Key::Char('5'), Key::Enter]);
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
    assert_eq!(model.page_id(), NAV_RECORDS);
    assert!(model.hints().starts_with("n add"));
    model.handle(Key::BackTab);
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
    for page in ['1', '2', '3', '4', '5'] {
        model.handle(Key::Char(page));
        check(&mut model, "page");
    }
    press(&mut model, &[Key::Char('2'), Key::Enter]);
    check(&mut model, "detail");
    model.handle(Key::Char('e'));
    write(
        &mut model,
        " 日本語のテキスト and a very long name that keeps going beyond the width of a narrow terminal window",
    );
    check(&mut model, "form");
    press(&mut model, &[Key::Esc, Key::Char('d')]);
    check(&mut model, "confirm");
    press(&mut model, &[Key::Esc, Key::Char(':')]);
    check(&mut model, "menu");
    press(&mut model, &[Key::Esc, Key::Char('3'), Key::Right]);
    check(&mut model, "rules");
    for _ in 0..4 {
        press(&mut model, &[Key::Char('5'), Key::Right]);
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
    assert!(line(0).starts_with("Template App · Overview"), "{}", line(0));
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
        let digit = char::from_digit(index as u32 + 1, 10).unwrap();
        model.handle(Key::Char(digit));
        screens.push(model.screen());
        hints.push(model.hints());
        for _ in 0..3 {
            model.handle(Key::PageDown);
            screens.push(model.screen());
        }
    }
    press(&mut model, &[Key::Char('3'), Key::Right]);
    screens.push(model.screen());
    hints.push(model.hints());
    press(&mut model, &[Key::Left, Key::Char('5')]);
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
