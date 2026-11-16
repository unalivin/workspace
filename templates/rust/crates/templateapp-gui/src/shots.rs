use app_core::launcher::MODE_GUI;
use app_core::service::{App, NAV_ABOUT, NAV_NOTIFICATIONS, NAV_OVERVIEW, NAV_RECORDS, NAV_SETTINGS};
use app_core::settings;

use crate::chooser;
use crate::host::{CHOOSER_SIZE, chooser_tokens};
use crate::icons::Icons;
use crate::model::{AboutTab, Dashboard, Options};
use crate::render::{Picture, Stage};
use crate::skin;

pub const WIDTH: f32 = 1180.0;
pub const HEIGHT: f32 = 760.0;
const FRAMES: usize = 4;

pub const NAMES: &[&str] = &[
    "first-start",
    "overview",
    "records",
    "columns",
    "notifications",
    "rules",
    "settings",
    "settings-dark",
    "about",
    "guide",
    "shortcuts",
    "diagnostics",
    "collapsed",
    "chooser",
];

pub fn dashboard(stage: &mut Stage, board: &mut Dashboard<'_>) -> Picture {
    stage.settle(FRAMES, |ui| board.ui(ui))
}

pub fn chooser(dark: bool) -> Picture {
    let mut stage = Stage::new(CHOOSER_SIZE[0], CHOOSER_SIZE[1], 1.0);
    let tokens = chooser_tokens(dark);
    let icons = Icons::default();
    skin::apply(&stage.ctx, &tokens);
    let mut remember = false;
    stage.settle(FRAMES, |ui| {
        *ui.style_mut() = skin::style(&tokens);
        chooser::show(ui, &tokens, &icons, MODE_GUI, &mut remember, false);
    })
}

pub fn screens(app: &mut App, options: Options) -> Vec<(&'static str, Picture)> {
    let mut stage = Stage::new(WIDTH, HEIGHT, 1.0);
    let mut board = Dashboard::new(app, options);
    let mut shots = Vec::new();
    shots.push(("first-start", dashboard(&mut stage, &mut board)));
    board.answer(false);
    board.load_demo();
    board.show(NAV_OVERVIEW);
    shots.push(("overview", dashboard(&mut stage, &mut board)));
    board.show(NAV_RECORDS);
    board.rec.selected = board.rec.list.first().map(|record| record.id);
    shots.push(("records", dashboard(&mut stage, &mut board)));
    board.choose_columns();
    shots.push(("columns", dashboard(&mut stage, &mut board)));
    board.cancel();
    board.show(NAV_NOTIFICATIONS);
    board.inbox.rules = false;
    shots.push(("notifications", dashboard(&mut stage, &mut board)));
    board.inbox.rules = true;
    shots.push(("rules", dashboard(&mut stage, &mut board)));
    board.inbox.rules = false;
    board.show(NAV_SETTINGS);
    shots.push(("settings", dashboard(&mut stage, &mut board)));
    board.save_setting(settings::KEY_MODE, "dark");
    shots.push(("settings-dark", dashboard(&mut stage, &mut board)));
    board.reset_setting(settings::KEY_MODE);
    for (name, tab) in [
        ("about", AboutTab::Overview),
        ("guide", AboutTab::Guide),
        ("shortcuts", AboutTab::Shortcuts),
        ("diagnostics", AboutTab::Diagnostics),
    ] {
        board.show(NAV_ABOUT);
        board.show_about(tab);
        shots.push((name, dashboard(&mut stage, &mut board)));
    }
    board.show(NAV_OVERVIEW);
    board.toggle_sidebar();
    shots.push(("collapsed", dashboard(&mut stage, &mut board)));
    board.toggle_sidebar();
    shots.push(("chooser", chooser(false)));
    shots
}
