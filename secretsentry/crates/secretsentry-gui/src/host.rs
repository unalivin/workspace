use std::cell::RefCell;
use std::time::Duration;

use app_core::brand;
use app_core::error::{AppError, Result};
use app_core::launcher::PromptResult;
use app_core::meta;
use app_core::service::App;
use app_core::theme::{self, Mode, Tokens};
use eframe::NativeOptions;
use egui::{Context, IconData, Theme, Ui, ViewportBuilder, ViewportCommand, Visuals};

use crate::chooser::{self, Choice};
use crate::model::{Dashboard, Options, window_size};
use crate::skin;

pub const CHOOSER_SIZE: [f32; 2] = [460.0, 290.0];
pub const MIN_SIZE: [f32; 2] = [720.0, 480.0];
const ICON_SIDE: usize = 256;

fn icon() -> IconData {
    IconData { rgba: brand::icon(ICON_SIDE), width: ICON_SIDE as u32, height: ICON_SIDE as u32 }
}

fn native(viewport: ViewportBuilder) -> NativeOptions {
    NativeOptions {
        viewport: viewport.with_icon(icon()).with_app_id(meta::APP_ID),
        run_and_return: true,
        centered: true,
        persist_window: false,
        ..NativeOptions::default()
    }
}

fn failure(error: &eframe::Error) -> AppError {
    AppError::Io(format!(
        "The window could not be opened: {error}. Start {} --ui tui in a terminal instead.",
        meta::SLUG
    ))
}

fn system_dark(ctx: &Context) -> Option<bool> {
    ctx.system_theme().map(|theme| theme == Theme::Dark)
}

struct Shown {
    size: Option<egui::Vec2>,
    full: bool,
}

fn shown(ctx: &Context) -> (f64, bool, Shown) {
    ctx.input(|input| {
        let viewport = input.viewport();
        let full =
            viewport.maximized == Some(true) || viewport.fullscreen == Some(true) || viewport.minimized == Some(true);
        (input.time, viewport.close_requested(), Shown { size: viewport.inner_rect.map(|area| area.size()), full })
    })
}

struct Window<'a, 'b> {
    board: Dashboard<'a>,
    next: &'b RefCell<String>,
    ticked: f64,
    size: Option<egui::Vec2>,
}

impl eframe::App for Window<'_, '_> {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let (now, closing, window) = shown(&ctx);
        if let (Some(size), false) = (window.size, window.full) {
            self.size = Some(size);
        }
        if let Some(dark) = system_dark(&ctx) {
            self.board.follow_system(dark);
        }
        let interval = self.board.interval();
        if now - self.ticked >= interval {
            self.ticked = now;
            self.board.tick();
        }
        ctx.request_repaint_after(Duration::from_secs_f64(interval));
        self.board.ui(ui);
        if closing && !self.board.finished() {
            self.board.close("");
        }
        if self.board.finished() {
            if let Some(size) = self.size.take() {
                self.board.keep_window_size(size.x, size.y);
            }
            *self.next.borrow_mut() = self.board.next().to_string();
            if !closing {
                ctx.send_viewport_cmd(ViewportCommand::Close);
            }
        }
    }

    fn clear_color(&self, visuals: &Visuals) -> [f32; 4] {
        visuals.panel_fill.to_normalized_gamma_f32()
    }
}

pub fn run(app: &mut App, mut options: Options) -> Result<String> {
    let size = window_size(&app.settings().effective()?);
    let next = RefCell::new(String::new());
    let outcome = {
        let next = &next;
        let creator: eframe::AppCreator<'_> = Box::new(move |creation| {
            skin::install(&creation.egui_ctx);
            if let Some(dark) = system_dark(&creation.egui_ctx) {
                options.system_dark = dark;
            }
            let board = Dashboard::new(app, options);
            Ok(Box::new(Window { board, next, ticked: 0.0, size: None }))
        });
        let viewport =
            ViewportBuilder::default().with_title(meta::APP_NAME).with_inner_size(size).with_min_inner_size(MIN_SIZE);
        eframe::run_native(meta::APP_NAME, native(viewport), creator)
    };
    outcome.map_err(|error| failure(&error))?;
    Ok(next.into_inner())
}

struct Prompt<'b> {
    default_mode: String,
    remember: bool,
    tokens: Option<Tokens>,
    result: &'b RefCell<Option<PromptResult>>,
    icons: crate::icons::Icons,
    focused: bool,
}

pub fn chooser_tokens(dark: bool) -> Tokens {
    theme::resolve(&theme::Options {
        name: "slate".to_string(),
        mode: if dark { Mode::Dark } else { Mode::Light },
        text_scale: 100,
        ..theme::Options::default()
    })
}

impl eframe::App for Prompt<'_> {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let dark = system_dark(&ctx).unwrap_or(false);
        if self.tokens.as_ref().is_none_or(|tokens| tokens.dark != dark) {
            let tokens = chooser_tokens(dark);
            skin::apply(&ctx, &tokens);
            *ui.style_mut() = skin::style(&tokens);
            self.tokens = Some(tokens);
        }
        let Some(tokens) = &self.tokens else {
            return;
        };
        let focus = !std::mem::replace(&mut self.focused, true);
        let choice = chooser::show(ui, tokens, &self.icons, &self.default_mode, &mut self.remember, focus);
        let closing = ctx.input(|input| input.viewport().close_requested());
        let result = match choice {
            Choice::Mode(mode) => Some(PromptResult { mode: mode.to_string(), remember: self.remember, quit: false }),
            Choice::Quit => Some(PromptResult { quit: true, ..PromptResult::default() }),
            Choice::Waiting if closing => Some(PromptResult { quit: true, ..PromptResult::default() }),
            Choice::Waiting => None,
        };
        if let Some(result) = result {
            *self.result.borrow_mut() = Some(result);
            if !closing {
                ctx.send_viewport_cmd(ViewportCommand::Close);
            }
        }
    }

    fn clear_color(&self, visuals: &Visuals) -> [f32; 4] {
        visuals.panel_fill.to_normalized_gamma_f32()
    }
}

pub fn choose(default_mode: &str) -> Result<PromptResult> {
    let result = RefCell::new(None);
    let title = chooser::title();
    let outcome = {
        let result = &result;
        let default_mode = default_mode.to_string();
        let creator: eframe::AppCreator<'_> = Box::new(move |creation| {
            skin::install(&creation.egui_ctx);
            Ok(Box::new(Prompt {
                default_mode,
                remember: false,
                tokens: None,
                result,
                icons: crate::icons::Icons::default(),
                focused: false,
            }))
        });
        let viewport =
            ViewportBuilder::default().with_title(&title).with_inner_size(CHOOSER_SIZE).with_resizable(false);
        eframe::run_native(&title, native(viewport), creator)
    };
    outcome.map_err(|error| failure(&error))?;
    Ok(result.into_inner().unwrap_or(PromptResult { quit: true, ..PromptResult::default() }))
}
