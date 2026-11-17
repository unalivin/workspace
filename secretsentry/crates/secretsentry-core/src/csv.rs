use crate::error::{AppError, Result};

fn finish_row(rows: &mut Vec<Vec<String>>, row: &mut Vec<String>, quoted: bool, max_rows: usize) -> Result<()> {
    if row.len() == 1 && row[0].is_empty() && !quoted {
        row.clear();
        return Ok(());
    }
    if rows.len() == max_rows {
        return Err(AppError::Usage(format!(
            "the import has more than {} rows: split it into smaller files",
            max_rows.saturating_sub(1)
        )));
    }
    rows.push(std::mem::take(row));
    Ok(())
}

pub fn parse(text: &str, max_rows: usize) -> Result<Vec<Vec<String>>> {
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut row: Vec<String> = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut was_quoted = false;
    let mut row_quoted = false;
    let mut letters = text.chars().peekable();
    while let Some(letter) = letters.next() {
        if quoted {
            if letter != '"' {
                field.push(letter);
            } else if letters.peek() == Some(&'"') {
                letters.next();
                field.push('"');
            } else {
                quoted = false;
            }
            continue;
        }
        match letter {
            '"' if field.is_empty() && !was_quoted => {
                quoted = true;
                was_quoted = true;
                row_quoted = true;
            }
            '"' => {
                return Err(AppError::Usage(format!(
                    "the CSV cannot be read near row {}: a quote appears inside a field, write it as two quotes inside a quoted field",
                    rows.len() + 1
                )));
            }
            ',' => {
                row.push(std::mem::take(&mut field));
                was_quoted = false;
            }
            '\r' if letters.peek() == Some(&'\n') => {}
            '\n' => {
                row.push(std::mem::take(&mut field));
                was_quoted = false;
                finish_row(&mut rows, &mut row, row_quoted, max_rows)?;
                row_quoted = false;
            }
            _ if was_quoted => {
                return Err(AppError::Usage(format!(
                    "the CSV cannot be read near row {}: text follows a closing quote",
                    rows.len() + 1
                )));
            }
            other => field.push(other),
        }
    }
    if quoted {
        return Err(AppError::Usage(format!(
            "the CSV cannot be read near row {}: a quoted field is never closed",
            rows.len() + 1
        )));
    }
    if !field.is_empty() || was_quoted || !row.is_empty() {
        row.push(field);
        finish_row(&mut rows, &mut row, row_quoted, max_rows)?;
    }
    Ok(rows)
}

pub fn write_row(output: &mut String, fields: &[&str]) {
    for (index, field) in fields.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        let needs_quotes = field.is_empty() && fields.len() == 1
            || field.contains([',', '"', '\n', '\r'])
            || field.starts_with(' ')
            || field.ends_with(' ');
        if needs_quotes {
            output.push('"');
            output.push_str(&field.replace('"', "\"\""));
            output.push('"');
        } else {
            output.push_str(field);
        }
    }
    output.push('\n');
}

const GUARDED: [char; 7] = ['=', '+', '-', '@', '\t', '\r', '\''];

pub fn guard_cell(value: &str) -> String {
    if value.starts_with(GUARDED) { format!("'{value}") } else { value.to_string() }
}

pub fn unguard_cell(value: &str) -> &str {
    match value.strip_prefix('\'') {
        Some(rest) if rest.starts_with(GUARDED) => rest,
        _ => value,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn rows_fields_quotes_and_line_endings_are_read() {
        let rows = parse("a,b,c\r\n\"x,1\",\"say \"\"hi\"\"\",\"two\nlines\"\n\nlast,,\n", 100).unwrap();
        assert_eq!(rows, vec![vec!["a", "b", "c"], vec!["x,1", "say \"hi\"", "two\nlines"], vec!["last", "", ""]]);
        assert_eq!(parse("only", 100).unwrap(), vec![vec!["only"]]);
        assert_eq!(parse("a,\"\"", 100).unwrap(), vec![vec!["a", ""]]);
        assert_eq!(parse("\"\"\nnext\n", 100).unwrap(), vec![vec![""], vec!["next"]]);
        assert_eq!(parse("", 100).unwrap(), Vec::<Vec<String>>::new());
        assert_eq!(parse("a\rb\n", 100).unwrap(), vec![vec!["a\rb"]]);
    }

    #[test]
    fn malformed_input_is_rejected_with_the_row() {
        let cases = [
            ("a,b\nc,\"open", "row 2: a quoted field is never closed"),
            ("a\"b,c\n", "row 1: a quote appears inside a field"),
            ("\"a\"b,c\n", "row 1: text follows a closing quote"),
        ];
        for (text, expected) in cases {
            let error = parse(text, 100).unwrap_err().to_string();
            assert!(error.contains(expected), "{error}");
        }
        assert!(parse("1\n2\n3\n", 2).unwrap_err().to_string().contains("more than 1 rows"));
        assert_eq!(parse("1\n2\n", 2).unwrap().len(), 2);
    }

    #[test]
    fn formula_cells_are_neutralised_and_restored() {
        for risky in ["=SUM(A1)", "+1", "-1", "@cmd", "\tx", "\rx"] {
            let guarded = guard_cell(risky);
            assert!(guarded.starts_with('\''), "{risky}");
            assert_eq!(unguard_cell(&guarded), risky);
        }
        assert_eq!(guard_cell("plain"), "plain");
        assert_eq!(unguard_cell("'written by hand"), "'written by hand");
        assert_eq!(unguard_cell(&guard_cell("'=kept as typed")), "'=kept as typed");
        assert_eq!(unguard_cell(&guard_cell("'")), "'");
    }

    proptest! {
        #[test]
        fn written_rows_are_read_back_unchanged(rows in proptest::collection::vec(proptest::collection::vec(".{0,12}", 1..5), 0..6)) {
            let mut text = String::new();
            for row in &rows {
                write_row(&mut text, &row.iter().map(String::as_str).collect::<Vec<_>>());
            }
            prop_assert_eq!(parse(&text, 100).unwrap(), rows);
        }

        #[test]
        fn parsing_never_panics(text in ".{0,200}") {
            let _ = parse(&text, 50);
        }
    }
}
