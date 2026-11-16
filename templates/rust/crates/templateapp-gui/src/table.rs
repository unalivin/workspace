use app_core::theme::Tokens;
use egui::{Align, Color32, Label, Rect, Response, RichText, Sense, Ui, pos2, vec2};

use crate::icons::Icons;
use crate::skin::{ICON_SCALE, alpha, color};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spec {
    pub fixed: f32,
    pub weight: f32,
}

pub const fn fixed(width: f32) -> Spec {
    Spec { fixed: width, weight: 0.0 }
}

pub const fn flexible(weight: f32, least: f32) -> Spec {
    Spec { fixed: least, weight }
}

pub fn layout(specs: &[Spec], available: f32, gap: f32) -> Vec<(f32, f32)> {
    let gaps = gap * specs.len().saturating_sub(1) as f32;
    let fixed_total: f32 = specs.iter().map(|spec| spec.fixed).sum();
    let weights: f32 = specs.iter().map(|spec| spec.weight).sum();
    let spare = (available - gaps - fixed_total).max(0.0);
    let mut left = 0.0;
    specs
        .iter()
        .map(|spec| {
            let width = spec.fixed + if weights > 0.0 { spare * spec.weight / weights } else { 0.0 };
            let slot = (left, width);
            left += width + gap;
            slot
        })
        .collect()
}

pub enum Cell {
    Text(String),
    Muted(String),
    Icon(&'static str, Color32, String),
}

pub fn header(ui: &mut Ui, tokens: &Tokens, slots: &[(f32, f32)], titles: &[&str]) {
    let height = tokens.spacing.row_height;
    let (area, _) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
    for ((left, width), title) in slots.iter().zip(titles) {
        let cell = Rect::from_min_size(pos2(area.left() + tokens.spacing.s + left, area.top()), vec2(*width, height));
        ui.put(cell, Label::new(RichText::new(*title).small().strong()).truncate().halign(Align::LEFT));
    }
    ui.painter().hline(area.x_range(), area.bottom(), (1.0, color(tokens.palette.divider)));
}

pub fn row(ui: &mut Ui, icons: &Icons, tokens: &Tokens, slots: &[(f32, f32)], cells: Vec<Cell>, selected: bool) -> Response {
    let height = tokens.spacing.row_height;
    let (area, response) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::click());
    let fill = if selected {
        color(tokens.selection)
    } else if response.hovered() {
        alpha(tokens.palette.text, 0x14)
    } else {
        Color32::TRANSPARENT
    };
    ui.painter().rect_filled(area, tokens.radius, fill);
    if response.has_focus() {
        ui.painter().rect_stroke(area, tokens.radius, (2.0, color(tokens.focus)), egui::StrokeKind::Inside);
    }
    let gap = tokens.spacing.s;
    for ((left, width), cell) in slots.iter().zip(cells) {
        let place = Rect::from_min_size(pos2(area.left() + gap + left, area.top()), vec2(*width, height));
        match cell {
            Cell::Text(text) => {
                ui.put(place, Label::new(text).truncate().halign(Align::LEFT));
            }
            Cell::Muted(text) => {
                ui.put(place, Label::new(RichText::new(text).weak()).truncate().halign(Align::LEFT));
            }
            Cell::Icon(name, tint, text) => {
                let size = tokens.type_scale.body * ICON_SCALE;
                let mark = Rect::from_center_size(pos2(place.left() + size / 2.0, place.center().y), vec2(size, size));
                ui.put(mark, icons.image(ui.ctx(), name, size, tint));
                let rest = Rect::from_min_max(pos2(place.left() + size + gap, place.top()), place.max);
                ui.put(rest, Label::new(text).truncate().halign(Align::LEFT));
            }
        }
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flexible_columns_share_what_the_fixed_ones_leave() {
        let slots = layout(&[fixed(60.0), flexible(2.0, 100.0), flexible(1.0, 100.0), fixed(80.0)], 700.0, 10.0);
        assert_eq!(slots.len(), 4);
        assert_eq!(slots[0], (0.0, 60.0));
        assert_eq!(slots[1], (70.0, 320.0));
        assert_eq!(slots[2], (400.0, 210.0));
        assert_eq!(slots[3], (620.0, 80.0));
        let last = slots[3];
        assert_eq!(last.0 + last.1, 700.0);
    }

    #[test]
    fn hidden_columns_leave_no_gap_and_narrow_windows_keep_the_least_width() {
        let all = layout(&[fixed(60.0), flexible(1.0, 100.0), fixed(80.0)], 500.0, 10.0);
        let fewer = layout(&[fixed(60.0), flexible(1.0, 100.0)], 500.0, 10.0);
        assert_eq!(all[1].1, 340.0);
        assert_eq!(fewer[1], (70.0, 430.0));
        let narrow = layout(&[fixed(60.0), flexible(1.0, 100.0), fixed(80.0)], 120.0, 10.0);
        assert_eq!(narrow[1].1, 100.0);
        assert_eq!(narrow[2].0, 180.0);
        assert!(layout(&[], 100.0, 10.0).is_empty());
        assert_eq!(layout(&[fixed(40.0), fixed(50.0)], 300.0, 10.0), vec![(0.0, 40.0), (50.0, 50.0)]);
    }
}
