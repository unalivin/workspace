use app_core::records::{Breakdown, Status};
use app_core::theme::{self, Severity, Tokens};
use egui::{Align2, FontFamily, FontId, Rect, Sense, Ui, pos2, vec2};

use crate::icons::Icons;
use crate::skin::{ICON_SCALE, color, medium, severity_icon, status_icon};

const UNITS: [i64; 13] = [1, 2, 5, 10, 20, 50, 100, 200, 500, 1000, 2000, 5000, 10000];
const ROW_GAP: f32 = 4.0;
const LABEL_EMS: f32 = 8.0;
const WIDEST_FIGURE: &str = "0000";
const DONE_FADE: f64 = 0.6;
const SQUARE_RADIUS: f32 = 3.0;

pub fn squares(total: i64, unit: i64) -> i64 {
    let (total, unit) = (total.max(0), unit.max(1));
    total / unit + i64::from(total % unit != 0)
}

pub fn unit(largest: i64, capacity: i64) -> i64 {
    let capacity = capacity.max(1);
    UNITS.into_iter().find(|unit| squares(largest, *unit) <= capacity).unwrap_or(squares(largest, capacity))
}

pub fn caption(unit: i64) -> String {
    if unit <= 1 {
        "One square is one enabled record.".to_string()
    } else {
        format!("One square is {unit} enabled records, rounded up.")
    }
}

pub fn count(rows: &[Breakdown], status: Status, level: Severity) -> i64 {
    rows.iter().filter(|row| row.status == status && row.severity == level).map(|row| row.total).sum()
}

pub fn largest(rows: &[Breakdown]) -> i64 {
    let mut largest = 0;
    for status in Status::ALL {
        for level in Severity::ALL {
            largest = largest.max(count(rows, status, level));
        }
    }
    largest
}

pub fn height(tokens: &Tokens) -> f32 {
    let lines = (Severity::ALL.len() + 1) as f32;
    lines * tokens.spacing.row_height + (lines - 1.0) * ROW_GAP
}

pub fn show(ui: &mut Ui, icons: &Icons, tokens: &Tokens, rows: &[Breakdown]) -> i64 {
    let gaps = &tokens.spacing;
    let sizes = &tokens.type_scale;
    let (area, _) = ui.allocate_exact_size(vec2(ui.available_width(), height(tokens)), Sense::hover());
    let painter = ui.painter_at(area);
    let muted = color(tokens.palette.text_muted);
    let ink = color(tokens.palette.text);
    let figures = FontId::new(sizes.caption, FontFamily::Monospace);
    let label = LABEL_EMS * sizes.body;
    let column = ((area.width() - label) / Status::ALL.len() as f32).max(0.0);
    let step = gaps.row_height + ROW_GAP;
    let square = gaps.m;
    let pitch = square + gaps.xs;
    let widest = painter.layout_no_wrap(WIDEST_FIGURE.to_string(), figures.clone(), muted).size().x;
    let capacity = ((column - gaps.l - widest - gaps.s) / pitch).floor() as i64;
    let unit = unit(largest(rows), capacity);
    let side = sizes.body * ICON_SCALE;
    let mark = |ui: &Ui, name: &str, tint, left: f32, middle: f32| {
        let place = Rect::from_center_size(pos2(left + side / 2.0, middle), vec2(side, side));
        icons.image(ui.ctx(), name, side, tint).paint_at(ui, place);
        side
    };

    let middle = area.top() + gaps.row_height / 2.0;
    for (index, status) in Status::ALL.into_iter().enumerate() {
        let mut left = area.left() + label + index as f32 * column;
        left += mark(ui, status_icon(status), muted, left, middle) + gaps.s;
        let name = FontId::new(sizes.body, medium());
        left += painter.text(pos2(left, middle), Align2::LEFT_CENTER, status.label(), name, ink).width() + gaps.s;
        let total: i64 = Severity::ALL.into_iter().map(|level| count(rows, status, level)).sum();
        painter.text(pos2(left, middle), Align2::LEFT_CENTER, total, figures.clone(), muted);
    }

    for (line, level) in Severity::ALL.into_iter().rev().enumerate() {
        let top = area.top() + (line + 1) as f32 * step;
        let middle = top + gaps.row_height / 2.0;
        painter.hline(area.x_range(), top - ROW_GAP / 2.0, (1.0, color(tokens.palette.divider)));
        let tint = tokens.semantic.severity(level);
        let left = area.left() + mark(ui, severity_icon(level), color(tint), area.left(), middle) + gaps.s;
        painter.text(pos2(left, middle), Align2::LEFT_CENTER, level.label(), FontId::proportional(sizes.body), ink);
        for (index, status) in Status::ALL.into_iter().enumerate() {
            let total = count(rows, status, level);
            if total == 0 {
                continue;
            }
            let fill = if status == Status::Done { theme::mix(tint, tokens.palette.surface, DONE_FADE) } else { tint };
            let start = area.left() + label + index as f32 * column;
            let drawn = squares(total, unit);
            for position in 0..drawn {
                let corner = pos2(start + position as f32 * pitch, middle - square / 2.0);
                let shape = Rect::from_min_size(corner, vec2(square, square));
                painter.rect_filled(shape, tokens.radius.min(SQUARE_RADIUS), color(fill));
            }
            let after = start + drawn as f32 * pitch + gaps.xs;
            painter.text(pos2(after, middle), Align2::LEFT_CENTER, total, figures.clone(), muted);
        }
    }
    unit
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(status: Status, severity: Severity, total: i64) -> Breakdown {
        Breakdown { status, severity, total, demo: 0 }
    }

    #[test]
    fn the_unit_grows_in_round_steps_until_the_largest_cell_fits() {
        assert_eq!(unit(0, 7), 1);
        assert_eq!(unit(7, 7), 1);
        assert_eq!(unit(8, 7), 2);
        assert_eq!(unit(15, 7), 5);
        assert_eq!(unit(36, 7), 10);
        assert_eq!(unit(700, 7), 100);
        assert_eq!(unit(701, 7), 200);
        assert_eq!(unit(70_000, 7), 10_000);
        assert_eq!(unit(70_001, 7), 10_001);
        assert_eq!(unit(12, 0), 20);
        assert_eq!(unit(-3, 7), 1);
        for largest in [1, 9, 99, 12_345, 987_654] {
            for capacity in [1, 3, 7, 12] {
                let chosen = unit(largest, capacity);
                assert!(squares(largest, chosen) <= capacity, "{largest} in {capacity}: unit {chosen}");
                assert!(squares(largest, chosen) >= 1);
            }
        }
    }

    #[test]
    fn squares_round_up_so_a_small_count_never_disappears() {
        assert_eq!(squares(0, 5), 0);
        assert_eq!(squares(1, 5), 1);
        assert_eq!(squares(5, 5), 1);
        assert_eq!(squares(6, 5), 2);
        assert_eq!(squares(3, 0), 3);
    }

    #[test]
    fn the_caption_names_the_unit() {
        assert_eq!(caption(1), "One square is one enabled record.");
        assert_eq!(caption(0), "One square is one enabled record.");
        assert_eq!(caption(20), "One square is 20 enabled records, rounded up.");
    }

    #[test]
    fn counts_add_up_per_cell_and_the_largest_cell_sets_the_scale() {
        let rows = [
            line(Status::Open, Severity::High, 2),
            line(Status::Open, Severity::High, 3),
            line(Status::Done, Severity::Low, 4),
            line(Status::InProgress, Severity::Critical, 1),
        ];
        assert_eq!(count(&rows, Status::Open, Severity::High), 5);
        assert_eq!(count(&rows, Status::Done, Severity::Low), 4);
        assert_eq!(count(&rows, Status::Done, Severity::High), 0);
        assert_eq!(largest(&rows), 5);
        assert_eq!(largest(&[]), 0);
    }
}
