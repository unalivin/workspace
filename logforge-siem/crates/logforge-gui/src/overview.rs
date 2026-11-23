use crate::table::TableState;
use eframe::egui;
use logforge_core::{event_analytics::BucketSize, SiemService};

pub fn metric_card(ui: &mut egui::Ui, label: &str, value: String) {
    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.set_min_width(138.0);
        ui.label(label);
        ui.heading(value);
    });
}

pub fn render_event_rate_chart(ui: &mut egui::Ui, service: &SiemService) {
    let end = chrono::Utc::now();
    let start = end - chrono::Duration::hours(24);
    let start_text = start.to_rfc3339();
    let end_text = end.to_rfc3339();
    ui.heading("Event and alert rate");
    ui.label(format!(
        "Events per hour · last 24 hours UTC · {} to {}",
        start.format("%H:%MZ"),
        end.format("%H:%MZ")
    ));
    let buckets = match logforge_core::event_analytics::event_rate_series(
        service.db(),
        &start_text,
        &end_text,
        BucketSize::Hour,
    ) {
        Ok(data) => data,
        Err(error) => {
            ui.colored_label(
                egui::Color32::from_rgb(179, 38, 30),
                format!("Unable to load chart data: {error}"),
            );
            return;
        }
    };
    if buckets.is_empty() {
        ui.label("Empty state: no hourly buckets are available for this time range.");
        return;
    }
    let total: i64 = buckets.iter().map(|item| item.event_count).sum();
    let peak = buckets
        .iter()
        .map(|item| item.event_count)
        .max()
        .unwrap_or(0);
    ui.horizontal(|ui| {
        ui.label(format!("Events in range: {total}"));
        ui.label(format!("Peak hourly volume: {peak}"));
        let previous_start = start - chrono::Duration::hours(24);
        let previous = logforge_core::stats::count_events_in_range(
            service.db(),
            &previous_start.to_rfc3339(),
            &start_text,
        );
        if let Ok(previous) = previous {
            ui.label(format!("Previous 24 hours: {previous}"));
        }
    });
    let available = ui.available_width().max(260.0);
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(available, 180.0), egui::Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 4.0, egui::Color32::from_rgb(248, 250, 252));
    let left = rect.left() + 34.0;
    let right = rect.right() - 10.0;
    let top = rect.top() + 12.0;
    let bottom = rect.bottom() - 24.0;
    let height = bottom - top;
    for step in 0..=4 {
        let y = bottom - height * step as f32 / 4.0;
        painter.line_segment(
            [egui::pos2(left, y), egui::pos2(right, y)],
            egui::Stroke::new(1.0, egui::Color32::from_rgb(203, 213, 225)),
        );
        painter.text(
            egui::pos2(rect.left() + 2.0, y),
            egui::Align2::LEFT_CENTER,
            format!("{}", peak * step / 4),
            egui::FontId::proportional(10.0),
            egui::Color32::from_rgb(11, 15, 20),
        );
    }
    let max_value = peak.max(1) as f32;
    let band = (right - left) / buckets.len() as f32;
    for (index, bucket) in buckets.iter().enumerate() {
        let bar_height = height * bucket.event_count as f32 / max_value;
        let x0 = left + index as f32 * band + 1.0;
        let x1 = (x0 + band - 2.0).min(right);
        let bar =
            egui::Rect::from_min_max(egui::pos2(x0, bottom - bar_height), egui::pos2(x1, bottom));
        let fill = if bucket.alert_count > 0 {
            egui::Color32::from_rgb(245, 158, 11)
        } else {
            egui::Color32::from_rgb(30, 41, 59)
        };
        painter.rect_filled(bar, 1.0, fill);
    }
    painter.text(
        egui::pos2(left, rect.bottom() - 9.0),
        egui::Align2::LEFT_CENTER,
        "24 hours ago",
        egui::FontId::proportional(10.0),
        egui::Color32::from_rgb(11, 15, 20),
    );
    painter.text(
        egui::pos2(right, rect.bottom() - 9.0),
        egui::Align2::RIGHT_CENTER,
        "now",
        egui::FontId::proportional(10.0),
        egui::Color32::from_rgb(11, 15, 20),
    );
    response.on_hover_text("Each bar is one UTC hour. Amber bars contain one or more alerts.");
    ui.horizontal(|ui| {
        ui.label("■ Events");
        ui.colored_label(
            egui::Color32::from_rgb(245, 158, 11),
            "■ Hours containing alerts",
        );
    });
}

pub fn render_overview_table(ui: &mut egui::Ui, service: &SiemService, table: &mut TableState) {
    let title = "Overview";
    if table.active_screen != Some(title) {
        table.active_screen = Some(title);
        table.loading_frames = 1;
    }
    if table.loading_frames > 0 {
        table.loading_frames -= 1;
        ui.label("Loading overview data…");
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(30));
        return;
    }
    let now = chrono::Utc::now();
    let start = now - chrono::Duration::hours(24);
    let buckets = match logforge_core::event_analytics::event_rate_series(
        service.db(),
        &start.to_rfc3339(),
        &now.to_rfc3339(),
        BucketSize::Hour,
    ) {
        Ok(buckets) => buckets,
        Err(error) => {
            ui.colored_label(
                egui::Color32::from_rgb(179, 38, 30),
                format!("Error loading overview records: {error}"),
            );
            if ui.button("Retry overview").clicked() {
                table.loading_frames = 1;
            }
            return;
        }
    };
    let rows = buckets
        .into_iter()
        .map(|bucket| {
            vec![
                bucket.start_utc,
                bucket.event_count.to_string(),
                bucket.alert_count.to_string(),
                bucket.critical_count.to_string(),
                bucket.error_count.to_string(),
            ]
        })
        .collect::<Vec<_>>();
    if rows.is_empty() {
        ui.label("Empty state: no hourly event records are available yet.");
    } else {
        ui.label(format!("Success: loaded {} hourly records.", rows.len()));
    }
    ui.heading("Hourly event records");
    table.render(
        ui,
        title,
        &["Hour UTC", "Events", "Alerts", "Critical", "Errors"],
        &rows,
    );
}
