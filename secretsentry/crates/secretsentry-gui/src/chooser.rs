use app_core::launcher::{MODE_GUI, MODE_TUI};
use app_core::meta;
use app_core::theme::Tokens;
use egui::{Align, Button, CentralPanel, Frame, Label, Layout, Margin, Ui, vec2};

use crate::forms::{muted, primary};
use crate::icons::Icons;
use crate::skin::{color, strong, subheading};

pub const DASHBOARD: &str = "Dashboard (graphical window)";
pub const TERMINAL: &str = "Terminal (text interface)";
pub const REMEMBER: &str = "Remember my choice";
pub const LATER: &str = "Change this later in Settings, Open with.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Choice {
    Waiting,
    Mode(&'static str),
    Quit,
}

pub fn title() -> String {
    format!("How do you want to open {}?", meta::APP_NAME)
}

pub fn show(
    ui: &mut Ui,
    tokens: &Tokens,
    icons: &Icons,
    default_mode: &str,
    remember: &mut bool,
    focus: bool,
) -> Choice {
    let mut choice = Choice::Waiting;
    let frame =
        Frame::new().fill(color(tokens.palette.background)).inner_margin(Margin::same(tokens.spacing.l.round() as i8));
    CentralPanel::default().frame(frame).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.add(icons.logo(ui.ctx(), 48.0));
            ui.add(Label::new(strong(title()).text_style(subheading())).wrap());
        });
        ui.add_space(tokens.spacing.s);
        let size = vec2(ui.available_width(), tokens.spacing.row_height);
        for (mode, label) in [(MODE_GUI, DASHBOARD), (MODE_TUI, TERMINAL)] {
            let preferred = (default_mode == MODE_TUI) == (mode == MODE_TUI);
            let button = if preferred { primary(label, tokens) } else { Button::new(label) };
            let pressed = ui.add(button.min_size(size));
            if preferred && focus {
                pressed.request_focus();
            }
            if pressed.clicked() {
                choice = Choice::Mode(mode);
            }
        }
        ui.checkbox(remember, REMEMBER);
        ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
            if ui.button("Quit").clicked() {
                choice = Choice::Quit;
            }
        });
        muted(ui, LATER);
    });
    choice
}
