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
    let mut fields = record
        .split(separator)
        .map(str::trim)
        .map(str::to_string)
        .take(6)
        .collect::<Vec<_>>();
    while fields.len() < 6 {
        fields.push(String::new());
    }
    fields
}

pub fn csv_cell(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}
