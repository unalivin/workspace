use std::time::Instant;

use app_core::error::{AppError, Result};
use app_core::platform::Env;
use app_core::service::App;
use app_core::theme::{self, Profile, TermColor};
use ratatui::Frame;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::style::{Color, Modifier, Style};

use crate::model::{Model, Options};
use crate::text::{Line, width};
use crate::widgets::Key;

pub fn convert(event: KeyEvent) -> Option<Key> {
    if event.kind == KeyEventKind::Release {
        return None;
    }
    let control = event.modifiers.contains(KeyModifiers::CONTROL);
    Some(match event.code {
        KeyCode::Char(letter) if control => Key::Ctrl(letter.to_ascii_lowercase()),
        KeyCode::Char(letter) => Key::Char(letter),
        KeyCode::F(number) => Key::F(number),
        KeyCode::Enter => Key::Enter,
        KeyCode::Esc => Key::Esc,
        KeyCode::Tab if event.modifiers.contains(KeyModifiers::SHIFT) => Key::BackTab,
        KeyCode::Tab => Key::Tab,
        KeyCode::BackTab => Key::BackTab,
        KeyCode::Up => Key::Up,
        KeyCode::Down => Key::Down,
        KeyCode::Left => Key::Left,
        KeyCode::Right => Key::Right,
        KeyCode::Home => Key::Home,
        KeyCode::End => Key::End,
        KeyCode::PageUp => Key::PageUp,
        KeyCode::PageDown => Key::PageDown,
        KeyCode::Backspace => Key::Backspace,
        KeyCode::Delete => Key::Delete,
        _ => return None,
    })
}

pub fn dark_background(env: &Env) -> bool {
    let reported = env.get("COLORFGBG").unwrap_or_default();
    match reported.rsplit(';').next().and_then(|background| background.trim().parse::<u8>().ok()) {
        Some(background) => background < 7 || background == 8,
        None => true,
    }
}

fn color(value: TermColor, profile: Profile) -> Option<Color> {
    match value {
        TermColor::Default => None,
        TermColor::Ansi(code) => Some(Color::Indexed(code)),
        TermColor::Rgb(rgb) if profile == Profile::Ansi256 => Some(Color::Indexed(theme::ansi256(rgb))),
        TermColor::Rgb(rgb) => Some(Color::Rgb(rgb.r, rgb.g, rgb.b)),
    }
}

pub fn convert_style(style: theme::Style, profile: Profile) -> Style {
    let mut out = Style::new();
    if let Some(foreground) = color(style.foreground, profile) {
        out = out.fg(foreground);
    }
    if let Some(background) = color(style.background, profile) {
        out = out.bg(background);
    }
    for (wanted, modifier) in
        [(style.bold, Modifier::BOLD), (style.underline, Modifier::UNDERLINED), (style.reverse, Modifier::REVERSED)]
    {
        if wanted {
            out = out.add_modifier(modifier);
        }
    }
    out
}

pub fn paint(frame: &mut Frame, lines: &[Line], style: &dyn Fn(crate::text::Look) -> Style) {
    let area = frame.area();
    let buffer = frame.buffer_mut();
    for (line, y) in lines.iter().zip(area.top()..area.bottom()) {
        let mut x = area.left();
        for span in &line.0 {
            if x >= area.right() {
                break;
            }
            buffer.set_stringn(x, y, &span.text, usize::from(area.right() - x), style(span.look));
            x = x.saturating_add(u16::try_from(width(&span.text)).unwrap_or(u16::MAX));
        }
    }
}

pub fn draw(frame: &mut Frame, model: &mut Model<'_>) {
    let area = frame.area();
    model.resize(usize::from(area.width), usize::from(area.height));
    let lines = model.view();
    let profile = model.profile();
    paint(frame, &lines, &|look| convert_style(model.style(look), profile));
}

fn failed(error: std::io::Error) -> AppError {
    AppError::Io(format!("the terminal UI stopped: {error}"))
}

pub fn run(app: &mut App, options: Options) -> Result<String> {
    let mut model = Model::new(app, options);
    let mut terminal = ratatui::try_init().map_err(failed)?;
    let mut deadline = Instant::now() + model.interval();
    let outcome = loop {
        if let Err(error) = terminal.draw(|frame| draw(frame, &mut model)) {
            break Err(error);
        }
        if model.finished() {
            break Ok(());
        }
        match event::poll(deadline.saturating_duration_since(Instant::now())) {
            Ok(true) => match event::read() {
                Ok(Event::Key(pressed)) => {
                    if let Some(key) = convert(pressed) {
                        model.handle(key);
                    }
                }
                Ok(_) => {}
                Err(error) => break Err(error),
            },
            Ok(false) => {
                model.tick();
                deadline = Instant::now() + model.interval();
            }
            Err(error) => break Err(error),
        }
    };
    let restored = ratatui::try_restore();
    outcome.map_err(failed)?;
    restored.map_err(failed)?;
    Ok(model.switch_to().to_string())
}
