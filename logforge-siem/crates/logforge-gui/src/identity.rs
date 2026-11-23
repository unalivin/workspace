use eframe::egui;

pub fn accent_color(name: &str) -> egui::Color32 {
    match name {
        "blue" => egui::Color32::from_rgb(43, 108, 212),
        "teal" => egui::Color32::from_rgb(14, 148, 163),
        _ => egui::Color32::from_rgb(245, 158, 11),
    }
}

pub fn apply_theme(ctx: &egui::Context, theme: &str, accent: &str) {
    let light = theme == "light";
    let mut visuals = if light {
        egui::Visuals::light()
    } else {
        egui::Visuals::dark()
    };
    if theme == "high-contrast" {
        visuals.window_fill = egui::Color32::BLACK;
        visuals.panel_fill = egui::Color32::BLACK;
        visuals.extreme_bg_color = egui::Color32::BLACK;
        visuals.faint_bg_color = egui::Color32::from_rgb(11, 15, 20);
    }
    let foreground = if light {
        egui::Color32::from_rgb(11, 15, 20)
    } else {
        egui::Color32::WHITE
    };
    visuals.override_text_color = Some(foreground);
    visuals.selection.bg_fill = accent_color(accent);
    visuals.selection.stroke = egui::Stroke::new(1.0, foreground);
    visuals.widgets.active.bg_fill = accent_color(accent);
    visuals.widgets.active.fg_stroke.color = if accent == "amber" {
        egui::Color32::from_rgb(11, 15, 20)
    } else {
        egui::Color32::WHITE
    };
    ctx.set_visuals(visuals);
}

pub fn draw_identity(ui: &mut egui::Ui, accent: egui::Color32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(48.0, 48.0), egui::Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 11.0, egui::Color32::from_rgb(15, 23, 42));
    let inner = rect.shrink(5.0);
    painter.rect_filled(inner, 8.0, egui::Color32::from_rgb(30, 41, 59));
    for (row, width) in [(14.0, 21.0), (23.0, 17.0), (32.0, 13.0)] {
        painter.line_segment(
            [
                egui::pos2(rect.left() + 10.0, rect.top() + row),
                egui::pos2(rect.left() + 10.0 + width, rect.top() + row),
            ],
            egui::Stroke::new(3.0, egui::Color32::WHITE),
        );
    }
    let center = egui::pos2(rect.left() + 35.0, rect.top() + 28.0);
    painter.add(egui::Shape::convex_polygon(
        vec![
            egui::pos2(center.x, center.y - 12.0),
            egui::pos2(center.x + 11.0, center.y + 9.0),
            egui::pos2(center.x - 11.0, center.y + 9.0),
        ],
        accent,
        egui::Stroke::new(1.0, egui::Color32::from_rgb(11, 15, 20)),
    ));
    painter.line_segment(
        [
            egui::pos2(center.x, center.y - 5.0),
            egui::pos2(center.x, center.y + 2.0),
        ],
        egui::Stroke::new(2.0, egui::Color32::from_rgb(11, 15, 20)),
    );
    painter.circle_filled(
        egui::pos2(center.x, center.y + 5.0),
        1.2,
        egui::Color32::from_rgb(11, 15, 20),
    );
}
