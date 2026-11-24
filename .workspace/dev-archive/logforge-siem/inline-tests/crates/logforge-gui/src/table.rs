#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_sort_and_visible_columns_apply_to_exported_rows() {
        let mut state = TableState {
            filter: "failed".into(),
            sort_column: 0,
            ascending: true,
            visible: vec![true, false, true],
            ..TableState::default()
        };
        let rows = vec![
            vec!["2".into(), "hidden".into(), "failed login".into()],
            vec!["1".into(), "hidden".into(), "failed password".into()],
            vec!["3".into(), "hidden".into(), "accepted".into()],
        ];
        let csv = state.export_csv("Alerts", &["ID", "Private", "Message"], &rows);
        assert!(csv.contains("\"ID\",\"Message\""));
        assert!(csv.contains("\"1\",\"failed password\""));
        assert!(csv.contains("\"2\",\"failed login\""));
        assert!(!csv.contains("hidden"));
        state.filter.clear();
        assert_eq!(state.filtered_rows(&rows).len(), 3);
    }

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
