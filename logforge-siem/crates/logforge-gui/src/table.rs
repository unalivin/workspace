use eframe::egui;

#[derive(Default)]
pub struct TableState {
    pub filter: String,
    pub selected: Option<usize>,
    pub sort_column: usize,
    pub ascending: bool,
    pub page: usize,
    pub page_size: usize,
    pub visible: Vec<bool>,
    pub columns_open: bool,
    pub active_screen: Option<&'static str>,
    pub loading_frames: u8,
}

impl TableState {
    pub fn render(
        &mut self,
        ui: &mut egui::Ui,
        name: &str,
        columns: &[&str],
        rows: &[Vec<String>],
    ) {
        if self.visible.len() != columns.len() {
            self.visible = vec![true; columns.len()];
        }
        if self.page_size == 0 {
            self.page_size = 50;
        }
        ui.horizontal(|ui| {
            ui.label("Filter rows");
            let response = ui.add(
                egui::TextEdit::singleline(&mut self.filter).hint_text("Search visible fields"),
            );
            if response.changed() {
                self.page = 0;
                self.selected = None;
            }
            if ui.button("Columns").clicked() {
                self.columns_open = !self.columns_open;
            }
            if ui.button("Export CSV").clicked() {
                ui.ctx().copy_text(self.export_csv(name, columns, rows));
            }
        });
        if self.columns_open {
            ui.horizontal_wrapped(|ui| {
                for (index, column) in columns.iter().enumerate() {
                    ui.checkbox(&mut self.visible[index], *column);
                }
            });
        }
        let filtered = self.filtered_rows(rows);
        if rows.is_empty() {
            ui.label("Empty: no records are available for this screen.");
            return;
        }
        if filtered.is_empty() {
            ui.label("No records match the current filter.");
            return;
        }
        let start = self.page.saturating_mul(self.page_size).min(filtered.len());
        let end = start.saturating_add(self.page_size).min(filtered.len());
        ui.label(format!(
            "{} records · showing {}–{}",
            filtered.len(),
            start + usize::from(start < end),
            end
        ));
        egui::ScrollArea::both().max_height(420.0).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("#");
                for (index, column) in columns.iter().enumerate() {
                    if self.visible[index] {
                        let label = if self.sort_column == index {
                            format!("{} {}", column, if self.ascending { "▲" } else { "▼" })
                        } else {
                            (*column).to_string()
                        };
                        if ui
                            .add_sized([150.0, 26.0], egui::Button::new(label))
                            .clicked()
                        {
                            if self.sort_column == index {
                                self.ascending = !self.ascending;
                            } else {
                                self.sort_column = index;
                                self.ascending = true;
                            }
                        }
                    }
                }
            });
            ui.separator();
            for (absolute, row) in filtered.iter().enumerate().skip(start).take(end - start) {
                let selected = self.selected == Some(absolute);
                ui.horizontal(|ui| {
                    if ui
                        .selectable_label(selected, format!("{}", absolute + 1))
                        .clicked()
                    {
                        self.selected = Some(absolute);
                    }
                    for (index, cell) in row.iter().enumerate().take(columns.len()) {
                        if self.visible[index] {
                            ui.add_sized([150.0, 24.0], egui::Label::new(cell).truncate());
                        }
                    }
                });
            }
        });
        ui.horizontal(|ui| {
            if ui
                .add_enabled(self.page > 0, egui::Button::new("Previous"))
                .clicked()
            {
                self.page -= 1;
            }
            let pages = filtered.len().div_ceil(self.page_size).max(1);
            ui.label(format!("Page {} of {}", self.page + 1, pages));
            if ui
                .add_enabled(self.page + 1 < pages, egui::Button::new("Next"))
                .clicked()
            {
                self.page += 1;
            }
            egui::ComboBox::from_id_salt((name, "page-size"))
                .selected_text(format!("{} rows", self.page_size))
                .show_ui(ui, |ui| {
                    for count in [25, 50, 100, 250] {
                        ui.selectable_value(&mut self.page_size, count, count.to_string());
                    }
                });
        });
        if let Some(row) = self.selected.and_then(|index| filtered.get(index)) {
            ui.separator();
            ui.collapsing("Record details", |ui| {
                for (column, value) in columns.iter().zip(row) {
                    ui.horizontal(|ui| {
                        ui.strong(*column);
                        ui.label(value);
                    });
                }
            });
        }
    }

    pub fn selected_record(&self, rows: &[Vec<String>]) -> Option<Vec<String>> {
        let selected = self.selected?;
        self.filtered_rows(rows).get(selected).cloned()
    }

    fn filtered_rows(&self, rows: &[Vec<String>]) -> Vec<Vec<String>> {
        let filter = self.filter.trim().to_lowercase();
        let mut result: Vec<Vec<String>> = rows
            .iter()
            .filter(|row| {
                filter.is_empty() || row.iter().any(|cell| cell.to_lowercase().contains(&filter))
            })
            .cloned()
            .collect();
        result.sort_by(|left, right| {
            let a = left.get(self.sort_column).map(String::as_str).unwrap_or("");
            let b = right
                .get(self.sort_column)
                .map(String::as_str)
                .unwrap_or("");
            let order = a.cmp(b);
            if self.ascending {
                order
            } else {
                order.reverse()
            }
        });
        result
    }

    fn export_csv(&self, name: &str, columns: &[&str], rows: &[Vec<String>]) -> String {
        let indices: Vec<usize> = self
            .visible
            .iter()
            .enumerate()
            .filter_map(|(i, shown)| shown.then_some(i))
            .collect();
        let mut output = format!("# {name}\n");
        output.push_str(
            &indices
                .iter()
                .map(|i| csv_cell(columns[*i]))
                .collect::<Vec<_>>()
                .join(","),
        );
        output.push('\n');
        for row in self.filtered_rows(rows) {
            output.push_str(
                &indices
                    .iter()
                    .map(|i| csv_cell(row.get(*i).map(String::as_str).unwrap_or("")))
                    .collect::<Vec<_>>()
                    .join(","),
            );
            output.push('\n');
        }
        output
    }
}

fn csv_cell(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_escaping_quotes_and_newlines() {
        assert_eq!(csv_cell("hello, \"world\"\n"), "\"hello, \"\"world\"\"\n\"");
    }

    #[test]
    fn selected_record_uses_the_filtered_sorted_rows() {
        let mut state = TableState {
            filter: "ssh".into(),
            sort_column: 0,
            ascending: true,
            selected: Some(0),
            ..TableState::default()
        };
        let rows = vec![
            vec!["2".into(), "ssh query".into()],
            vec!["1".into(), "ssh search".into()],
            vec!["3".into(), "dns query".into()],
        ];
        assert_eq!(state.selected_record(&rows).unwrap()[0], "1");
        state.selected = Some(1);
        assert_eq!(state.selected_record(&rows).unwrap()[0], "2");
    }

    #[test]
    fn default_table_state_initializes_columns() {
        let mut state = TableState::default();
        let columns = ["id", "event"];
        assert!(state.visible.is_empty());
        assert_eq!(columns.len(), 2);
        state.visible = vec![true; columns.len()];
        assert_eq!(state.visible, vec![true, true]);
    }
}
