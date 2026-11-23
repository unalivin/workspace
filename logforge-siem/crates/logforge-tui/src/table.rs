pub fn split_record(record: &str) -> Vec<String> {
    let separator = if record.contains(" · ") {
        " · "
    } else if record.contains(" | ") {
        " | "
    } else if record.contains(" — ") {
        " — "
    } else {
        return vec![record.to_string()];
    };
    let mut fields = record.split(separator).map(str::trim).map(str::to_string).take(6).collect::<Vec<_>>();
    while fields.len() < 6 {
        fields.push(String::new());
    }
    fields
}

pub fn csv_cell(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

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
