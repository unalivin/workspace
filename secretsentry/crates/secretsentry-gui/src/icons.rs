use std::cell::RefCell;
use std::collections::HashMap;

use egui::{Color32, ColorImage, Context, Image, TextureHandle, TextureOptions, load::SizedTexture, vec2};

pub const VIEW: f32 = 24.0;
const SAMPLES: usize = 4;
const ARC_STEP: f32 = 0.2;

pub const PATHS: &[(&str, &str)] = &[
    ("layout-dashboard", "M3 3h8v10H3z M13 3h8v6h-8z M13 11h8v10h-8z M3 15h8v6H3z"),
    ("list", "M3 5h2v2H3z M7 5h14v2H7z M3 11h2v2H3z M7 11h14v2H7z M3 17h2v2H3z M7 17h14v2H7z"),
    (
        "bell",
        "M12 2a2 2 0 0 1 2 2v.3A7 7 0 0 1 19 11v4l2 3v1H3v-1l2-3v-4a7 7 0 0 1 5-6.7V4a2 2 0 0 1 2-2z M9.5 20h5a2.5 2.5 0 0 1-5 0z",
    ),
    (
        "settings",
        "M3 5h9v2H3z M16 5h5v2h-5z M12 3h2v6h-2z M3 11h3v2H3z M10 11h11v2H10z M6 9h2v6H6z M3 17h11v2H3z M18 17h3v2h-3z M14 15h2v6h-2z",
    ),
    (
        "info",
        "M12 2a10 10 0 1 0 0 20a10 10 0 1 0 0-20z M12 4a8 8 0 1 1 0 16a8 8 0 1 1 0-16z M11 7h2v2h-2z M11 11h2v6h-2z",
    ),
    ("octagon-alert", "M8 2h8l6 6v8l-6 6H8l-6-6V8z M11 6v8h2V6z M11 16v2h2v-2z"),
    ("triangle-alert", "M12 2L23 21H1z M11 9v6h2V9z M11 17v2h2v-2z"),
    ("circle-alert", "M12 2a10 10 0 1 1 0 20a10 10 0 1 1 0-20z M11 6v8h2V6z M11 16v2h2v-2z"),
    ("circle-minus", "M12 2a10 10 0 1 1 0 20a10 10 0 1 1 0-20z M7 11v2h10v-2z"),
    ("status-open", "M12 3a9 9 0 1 0 0 18a9 9 0 1 0 0-18z M12 5a7 7 0 1 1 0 14a7 7 0 1 1 0-14z"),
    (
        "status-progress",
        "M12 3a9 9 0 1 0 0 18a9 9 0 1 0 0-18z M12 5a7 7 0 1 1 0 14a7 7 0 1 1 0-14z M12 5a7 7 0 0 1 0 14z",
    ),
    ("status-done", "M12 3a9 9 0 1 0 0 18a9 9 0 1 0 0-18z M8.5 11L10.5 13L16 7.5L17.5 9L10.5 16L7 12.5z"),
    ("sidebar", "M3 4h18v16H3z M5 6v12h4V6z"),
    ("terminal", "M2 4h20v16H2z M4 6v12h16V6z M6 9l1.4-1.4L11.8 12l-4.4 4.4L6 15l3-3z M12 15h6v2h-6z"),
    ("x", "M6.4 5L12 10.6L17.6 5L19 6.4L13.4 12L19 17.6L17.6 19L12 13.4L6.4 19L5 17.6L10.6 12L5 6.4z"),
    ("check", "M9 16.2L4.8 12L3.4 13.4L9 19L21 7L19.6 5.6z"),
    ("plus", "M11 5h2v6h6v2h-6v6h-2v-6H5v-2h6z"),
    ("copy", "M8 3h12v14H8z M10 5v10h8V5z M4 7h2v12h10v2H4z"),
    ("columns", "M3 4h18v16H3z M5 6v12h3V6z M10 6v12h4V6z M16 6v12h3V6z"),
    ("trash", "M9 3h6v2h5v2H4V5h5z M6 9h12v12H6z M8 11v8h8v-8z"),
    ("pencil", "M16.5 3L21 7.5L8.5 20H4v-4.5z M6 16.3V18h1.7L18.2 7.5L16.5 5.8z"),
    ("download", "M11 3h2v9.2l3.6-3.6L18 10l-6 6l-6-6l1.4-1.4L11 12.2z M4 19h16v2H4z"),
    ("upload", "M11 16h2V6.8l3.6 3.6L18 9l-6-6l-6 6l1.4 1.4L11 6.8z M4 19h16v2H4z"),
    ("arrow-right", "M4 11h12.2l-4.6-4.6L13 5l7 7l-7 7l-1.4-1.4l4.6-4.6H4z"),
    ("palette", "M3 3h8v8H3z M13 3h8v8h-8z M3 13h8v8H3z M13 13h8v8h-8z M15 15v4h4v-4z"),
    ("monitor", "M2 4h20v13H2z M4 6v9h16V6z M8 19h8v2H8z"),
    ("message", "M2 4h20v14H9l-5 4v-4H2z M4 6v10h2v2l2.5-2H20V6z"),
];

pub const FALLBACK: &str = "info";
const LOGO: &str = "#logo";

pub fn path(name: &str) -> &'static str {
    let found =
        PATHS.iter().find(|(known, _)| *known == name).or_else(|| PATHS.iter().find(|(known, _)| *known == FALLBACK));
    found.map(|(_, data)| *data).unwrap_or("")
}

pub type Point = (f32, f32);

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Command(char),
    Number(f32),
}

fn tokens(data: &str) -> Result<Vec<Token>, String> {
    let letters: Vec<char> = data.chars().collect();
    let mut found = Vec::new();
    let mut at = 0;
    while at < letters.len() {
        let letter = letters[at];
        if letter.is_whitespace() || letter == ',' {
            at += 1;
        } else if letter.is_ascii_alphabetic() {
            found.push(Token::Command(letter));
            at += 1;
        } else if letter == '-' || letter == '+' || letter == '.' || letter.is_ascii_digit() {
            let start = at;
            let mut dot = letter == '.';
            at += 1;
            while at < letters.len() && (letters[at].is_ascii_digit() || (letters[at] == '.' && !dot)) {
                dot = dot || letters[at] == '.';
                at += 1;
            }
            let text: String = letters[start..at].iter().collect();
            found.push(Token::Number(text.parse().map_err(|_| format!("\"{text}\" is not a number"))?));
        } else {
            return Err(format!("unexpected \"{letter}\" in the path"));
        }
    }
    Ok(found)
}

fn angle(ux: f32, uy: f32, vx: f32, vy: f32) -> f32 {
    (ux * vy - uy * vx).atan2(ux * vx + uy * vy)
}

fn arc(from: Point, radii: Point, rotation: f32, large: bool, sweep: bool, to: Point, out: &mut Vec<Point>) {
    let (mut rx, mut ry) = (radii.0.abs(), radii.1.abs());
    if rx == 0.0 || ry == 0.0 || from == to {
        out.push(to);
        return;
    }
    let (sin, cos) = rotation.to_radians().sin_cos();
    let dx = (from.0 - to.0) / 2.0;
    let dy = (from.1 - to.1) / 2.0;
    let x1 = cos * dx + sin * dy;
    let y1 = -sin * dx + cos * dy;
    let stretch = (x1 * x1) / (rx * rx) + (y1 * y1) / (ry * ry);
    if stretch > 1.0 {
        rx *= stretch.sqrt();
        ry *= stretch.sqrt();
    }
    let top = (rx * rx * ry * ry - rx * rx * y1 * y1 - ry * ry * x1 * x1).max(0.0);
    let bottom = rx * rx * y1 * y1 + ry * ry * x1 * x1;
    let sign = if large == sweep { -1.0 } else { 1.0 };
    let factor = if bottom == 0.0 { 0.0 } else { sign * (top / bottom).sqrt() };
    let cx1 = factor * rx * y1 / ry;
    let cy1 = -factor * ry * x1 / rx;
    let cx = cos * cx1 - sin * cy1 + (from.0 + to.0) / 2.0;
    let cy = sin * cx1 + cos * cy1 + (from.1 + to.1) / 2.0;
    let start = angle(1.0, 0.0, (x1 - cx1) / rx, (y1 - cy1) / ry);
    let mut span = angle((x1 - cx1) / rx, (y1 - cy1) / ry, (-x1 - cx1) / rx, (-y1 - cy1) / ry);
    if !sweep && span > 0.0 {
        span -= std::f32::consts::TAU;
    } else if sweep && span < 0.0 {
        span += std::f32::consts::TAU;
    }
    let steps = ((span.abs() / ARC_STEP).ceil() as usize).max(1);
    for step in 1..steps {
        let turn = start + span * step as f32 / steps as f32;
        let (turn_sin, turn_cos) = turn.sin_cos();
        out.push((cx + cos * rx * turn_cos - sin * ry * turn_sin, cy + sin * rx * turn_cos + cos * ry * turn_sin));
    }
    out.push(to);
}

fn arity(command: char) -> usize {
    match command.to_ascii_uppercase() {
        'M' | 'L' => 2,
        'H' | 'V' => 1,
        'A' => 7,
        _ => 0,
    }
}

pub fn outlines(data: &str) -> Result<Vec<Vec<Point>>, String> {
    let list = tokens(data)?;
    let mut shapes: Vec<Vec<Point>> = Vec::new();
    let mut current: Vec<Point> = Vec::new();
    let mut pen: Point = (0.0, 0.0);
    let mut start: Point = (0.0, 0.0);
    let mut command = ' ';
    let mut at = 0;
    while at < list.len() {
        if let Token::Command(letter) = list[at] {
            command = letter;
            at += 1;
            if !"MLHVAZmlhvaz".contains(letter) {
                return Err(format!("the path command \"{letter}\" is not supported"));
            }
            if letter.eq_ignore_ascii_case(&'z') {
                if current.len() > 2 {
                    shapes.push(std::mem::take(&mut current));
                }
                current.clear();
                pen = start;
            }
            continue;
        }
        let count = arity(command);
        if count == 0 {
            return Err("a number appears where a path command is expected".to_string());
        }
        let mut values = Vec::with_capacity(count);
        for token in list.iter().skip(at).take(count) {
            match token {
                Token::Number(value) => values.push(*value),
                Token::Command(_) => break,
            }
        }
        if values.len() != count {
            return Err(format!("the path command \"{command}\" needs {count} numbers"));
        }
        at += count;
        let relative = command.is_ascii_lowercase();
        let offset = |x: f32, y: f32| if relative { (pen.0 + x, pen.1 + y) } else { (x, y) };
        match command.to_ascii_uppercase() {
            'M' => {
                if current.len() > 2 {
                    shapes.push(std::mem::take(&mut current));
                }
                current.clear();
                pen = offset(values[0], values[1]);
                start = pen;
                current.push(pen);
                command = if relative { 'l' } else { 'L' };
            }
            'L' => {
                pen = offset(values[0], values[1]);
                current.push(pen);
            }
            'H' => {
                pen = (if relative { pen.0 + values[0] } else { values[0] }, pen.1);
                current.push(pen);
            }
            'V' => {
                pen = (pen.0, if relative { pen.1 + values[0] } else { values[0] });
                current.push(pen);
            }
            _ => {
                let to = offset(values[5], values[6]);
                if current.is_empty() {
                    current.push(pen);
                }
                arc(pen, (values[0], values[1]), values[2], values[3] != 0.0, values[4] != 0.0, to, &mut current);
                pen = to;
            }
        }
    }
    if current.len() > 2 {
        shapes.push(current);
    }
    Ok(shapes)
}

fn winding(shapes: &[Vec<Point>], x: f32, y: f32) -> i32 {
    let mut turns = 0;
    for shape in shapes {
        for (index, from) in shape.iter().enumerate() {
            let to = shape[(index + 1) % shape.len()];
            let side = (to.0 - from.0) * (y - from.1) - (x - from.0) * (to.1 - from.1);
            if from.1 <= y && to.1 > y && side > 0.0 {
                turns += 1;
            } else if from.1 > y && to.1 <= y && side < 0.0 {
                turns -= 1;
            }
        }
    }
    turns
}

pub fn mask(shapes: &[Vec<Point>], size: usize) -> Vec<u8> {
    let scale = VIEW / size.max(1) as f32;
    let mut coverage = vec![0u8; size * size];
    for py in 0..size {
        for px in 0..size {
            let mut inside = 0;
            for sy in 0..SAMPLES {
                for sx in 0..SAMPLES {
                    let x = (px as f32 + (sx as f32 + 0.5) / SAMPLES as f32) * scale;
                    let y = (py as f32 + (sy as f32 + 0.5) / SAMPLES as f32) * scale;
                    if winding(shapes, x, y) != 0 {
                        inside += 1;
                    }
                }
            }
            coverage[py * size + px] = (inside * 255 / (SAMPLES * SAMPLES)) as u8;
        }
    }
    coverage
}

#[derive(Default)]
pub struct Icons {
    textures: RefCell<HashMap<(String, usize), TextureHandle>>,
}

impl Icons {
    pub fn image(&self, ctx: &Context, name: &str, points: f32, tint: Color32) -> Image<'static> {
        let pixels = ((points * ctx.pixels_per_point()).round() as usize).clamp(8, 256);
        let key = (name.to_string(), pixels);
        let mut textures = self.textures.borrow_mut();
        let texture = textures.entry(key).or_insert_with(|| {
            let coverage = mask(&outlines(path(name)).unwrap_or_default(), pixels);
            let picture = ColorImage::new(
                [pixels, pixels],
                coverage.iter().map(|share| Color32::from_rgba_premultiplied(*share, *share, *share, *share)).collect(),
            );
            ctx.load_texture(format!("icon-{name}-{pixels}"), picture, TextureOptions::LINEAR)
        });
        Image::from_texture(SizedTexture::new(texture.id(), vec2(points, points))).tint(tint)
    }

    pub fn drawn(&self) -> Vec<String> {
        let textures = self.textures.borrow();
        let mut names: Vec<String> =
            textures.keys().map(|(name, _)| name.clone()).filter(|name| name != LOGO).collect();
        names.sort();
        names.dedup();
        names
    }

    pub fn logo(&self, ctx: &Context, points: f32) -> Image<'static> {
        let pixels = ((points * ctx.pixels_per_point()).round() as usize).clamp(16, 512);
        let key = (LOGO.to_string(), pixels);
        let mut textures = self.textures.borrow_mut();
        let texture = textures.entry(key).or_insert_with(|| {
            let picture = ColorImage::from_rgba_unmultiplied([pixels, pixels], &app_core::brand::icon(pixels));
            ctx.load_texture(format!("logo-{pixels}"), picture, TextureOptions::LINEAR)
        });
        Image::from_texture(SizedTexture::new(texture.id(), vec2(points, points)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn share(coverage: &[u8], size: usize, x: usize, y: usize) -> u8 {
        coverage[y * size + x]
    }

    #[test]
    fn every_icon_parses_into_closed_outlines_inside_the_view_box() {
        for (name, data) in PATHS {
            let shapes = outlines(data).unwrap_or_else(|problem| panic!("{name}: {problem}"));
            assert!(!shapes.is_empty(), "{name} has no outline");
            for shape in &shapes {
                assert!(shape.len() >= 3, "{name} has an open outline");
                for (x, y) in shape {
                    assert!(
                        (0.0..=VIEW).contains(x) && (0.0..=VIEW).contains(y),
                        "{name} leaves the view box at {x},{y}"
                    );
                }
            }
            let coverage = mask(&shapes, 24);
            let filled = coverage.iter().filter(|share| **share > 0).count();
            assert!(filled > 24 && filled < 24 * 24, "{name} fills {filled} pixels");
        }
    }

    #[test]
    fn inner_outlines_cut_holes() {
        let frame = mask(&outlines(path("sidebar")).unwrap(), 24);
        assert_eq!(share(&frame, 24, 3, 12), 255);
        assert_eq!(share(&frame, 24, 6, 12), 0);
        assert_eq!(share(&frame, 24, 14, 12), 255);
        assert_eq!(share(&frame, 24, 1, 1), 0);
        let ring = mask(&outlines(path("status-open")).unwrap(), 24);
        assert_eq!(share(&ring, 24, 12, 12), 0);
        assert_eq!(share(&ring, 24, 12, 3), 255);
        let done = mask(&outlines(path("status-done")).unwrap(), 24);
        assert_eq!(share(&done, 24, 12, 4), 255);
    }

    #[test]
    fn arcs_follow_the_circle() {
        let shapes = outlines("M12 2a10 10 0 1 1 0 20a10 10 0 1 1 0-20z").unwrap();
        assert_eq!(shapes.len(), 1);
        for (x, y) in &shapes[0] {
            let distance = ((x - 12.0).powi(2) + (y - 12.0).powi(2)).sqrt();
            assert!((distance - 10.0).abs() < 0.01, "{x},{y} is {distance} from the center");
        }
        assert!(shapes[0].iter().any(|(x, _)| *x > 21.9));
        assert!(shapes[0].iter().any(|(x, _)| *x < 2.1));
    }

    #[test]
    fn numbers_may_touch_and_commands_repeat() {
        assert_eq!(
            tokens("M1-2.5.5l3,4").unwrap(),
            vec![
                Token::Command('M'),
                Token::Number(1.0),
                Token::Number(-2.5),
                Token::Number(0.5),
                Token::Command('l'),
                Token::Number(3.0),
                Token::Number(4.0)
            ]
        );
        let shapes = outlines("M2 2 10 2 10 10z m1 1h2v2z").unwrap();
        assert_eq!(shapes[0], vec![(2.0, 2.0), (10.0, 2.0), (10.0, 10.0)]);
        assert_eq!(shapes[1], vec![(3.0, 3.0), (5.0, 3.0), (5.0, 5.0)]);
    }

    #[test]
    fn broken_paths_are_rejected_and_unknown_names_fall_back() {
        for broken in ["M1", "C1 2 3 4 5 6", "M1 2 L3", "5 5", "M1 2 # 3"] {
            assert!(outlines(broken).is_err(), "{broken}");
        }
        assert_eq!(path("no-such-icon"), path(FALLBACK));
        assert!(outlines("").unwrap().is_empty());
    }
}
