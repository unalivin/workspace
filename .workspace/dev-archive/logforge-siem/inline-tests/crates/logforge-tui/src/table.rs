#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_record_keeps_columns_and_pads_for_toggle_state() {
        let fields = split_record("time · error · host-1");
        assert_eq!(fields[0], "time");
        assert_eq!(fields[1], "error");
        assert_eq!(fields[2], "host-1");
        assert_eq!(fields.len(), 6);
    }

    #[test]
    fn csv_cell_quotes_delimiters_quotes_and_newlines() {
        assert_eq!(csv_cell("a, \"b\"\nc"), "\"a, \"\"b\"\"\nc\"");
    }
}
