use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::csv;
use crate::error::{AppError, Result};
use crate::records::{self, Input, Record, Records};
use crate::words;

pub const MAX_IMPORT_BYTES: usize = 5 << 20;
pub const MAX_IMPORT_ROWS: usize = 10000;
const CSV_HEADER: [&str; 5] = ["name", "status", "severity", "note", "enabled"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Csv,
    Json,
}

impl Format {
    pub fn parse(text: &str) -> Result<Format> {
        match text.trim().to_lowercase().as_str() {
            "csv" => Ok(Format::Csv),
            "json" => Ok(Format::Json),
            other => Err(AppError::Usage(format!("unknown format \"{other}\": use csv or json"))),
        }
    }

    pub fn for_file(name: &str, explicit: &str) -> Result<Format> {
        if !explicit.is_empty() {
            return Format::parse(explicit);
        }
        let lowered = name.to_lowercase();
        if lowered.ends_with(".csv") {
            Ok(Format::Csv)
        } else if lowered.ends_with(".json") {
            Ok(Format::Json)
        } else {
            Err(AppError::Usage(format!(
                "the format of \"{name}\" is not clear from its name: use a .csv or .json file name, or pass --format csv or --format json"
            )))
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Format::Csv => "csv",
            Format::Json => "json",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Exported {
    name: String,
    #[serde(default)]
    status: String,
    #[serde(default)]
    severity: String,
    #[serde(default)]
    note: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    enabled: Option<bool>,
}

pub fn export(format: Format, list: &[Record]) -> Result<String> {
    match format {
        Format::Csv => {
            let mut output = String::new();
            csv::write_row(&mut output, &CSV_HEADER);
            for record in list {
                csv::write_row(
                    &mut output,
                    &[
                        &csv::guard_cell(&record.name),
                        record.status.as_str(),
                        record.severity.as_str(),
                        &csv::guard_cell(&record.note),
                        if record.enabled { "true" } else { "false" },
                    ],
                );
            }
            Ok(output)
        }
        Format::Json => {
            let rows: Vec<Exported> = list
                .iter()
                .map(|record| Exported {
                    name: record.name.clone(),
                    status: record.status.as_str().to_string(),
                    severity: record.severity.as_str().to_string(),
                    note: record.note.clone(),
                    enabled: Some(record.enabled),
                })
                .collect();
            Ok(serde_json::to_string_pretty(&rows)? + "\n")
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RowError {
    pub row: usize,
    pub message: String,
}

impl std::fmt::Display for RowError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "row {}: {}", self.row, self.message)
    }
}

fn parse_bool(value: &str) -> Result<bool> {
    match value.trim().to_lowercase().as_str() {
        "" | "true" | "yes" | "1" | "on" => Ok(true),
        "false" | "no" | "0" | "off" => Ok(false),
        _ => Err(AppError::invalid("enabled", format!("\"{value}\" is not a yes or no value"), "Use true or false")),
    }
}

struct Candidate {
    row: usize,
    item: Exported,
    rejected: Option<AppError>,
}

fn csv_candidates(text: &str) -> Result<Vec<Candidate>> {
    let mut rows = csv::parse(text, MAX_IMPORT_ROWS + 1)?.into_iter();
    let header = rows.next().unwrap_or_default();
    let position: HashMap<String, usize> =
        header.iter().enumerate().map(|(index, name)| (name.trim().to_lowercase(), index)).collect();
    if !position.contains_key("name") {
        return Err(AppError::Usage(
            "the CSV header has no \"name\" column: the first line must be name,status,severity,note,enabled"
                .to_string(),
        ));
    }
    let mut candidates = Vec::new();
    for (index, fields) in rows.enumerate() {
        let cell = |name: &str| position.get(name).and_then(|at| fields.get(*at)).map(String::as_str).unwrap_or("");
        let (enabled, rejected) = if position.contains_key("enabled") {
            match parse_bool(cell("enabled")) {
                Ok(value) => (Some(value), None),
                Err(error) => (None, Some(error)),
            }
        } else {
            (None, None)
        };
        candidates.push(Candidate {
            row: index + 2,
            item: Exported {
                name: csv::unguard_cell(cell("name")).to_string(),
                status: cell("status").to_string(),
                severity: cell("severity").to_string(),
                note: csv::unguard_cell(cell("note")).to_string(),
                enabled,
            },
            rejected,
        });
    }
    Ok(candidates)
}

fn json_candidates(text: &str) -> Result<Vec<Candidate>> {
    let rows: Vec<Exported> = serde_json::from_str(text).map_err(|error| {
        AppError::Usage(format!(
            "the JSON cannot be read: {error}: the file must hold a list of objects with name, status, severity, note and enabled"
        ))
    })?;
    if rows.len() > MAX_IMPORT_ROWS {
        return Err(AppError::Usage(format!(
            "the import has more than {MAX_IMPORT_ROWS} rows: split it into smaller files"
        )));
    }
    Ok(rows.into_iter().enumerate().map(|(index, item)| Candidate { row: index + 1, item, rejected: None }).collect())
}

fn to_input(item: &Exported) -> Result<Input> {
    let status =
        if item.status.trim().is_empty() { records::Status::Open } else { records::parse_status(&item.status)? };
    let severity = if item.severity.trim().is_empty() {
        crate::theme::Severity::Info
    } else {
        records::parse_severity(&item.severity)?
    };
    records::normalize(&Input {
        name: item.name.clone(),
        status,
        severity,
        note: item.note.clone(),
        enabled: item.enabled.unwrap_or(true),
    })
}

pub fn parse(raw: &[u8], format: Format) -> Result<(Vec<Input>, Vec<RowError>)> {
    if raw.len() > MAX_IMPORT_BYTES {
        return Err(AppError::Usage(format!(
            "the import is larger than {} MB: split it into smaller files",
            MAX_IMPORT_BYTES >> 20
        )));
    }
    let text = std::str::from_utf8(raw).map_err(|_| {
        AppError::Usage("the import is not valid UTF-8 text: save it as UTF-8 and try again".to_string())
    })?;
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    if text.trim().is_empty() {
        return Err(AppError::Usage("the import is empty: export a list first to see the expected layout".to_string()));
    }
    let candidates = match format {
        Format::Csv => csv_candidates(text)?,
        Format::Json => json_candidates(text)?,
    };
    let mut inputs = Vec::new();
    let mut problems = Vec::new();
    let mut seen: HashMap<String, usize> = HashMap::new();
    for candidate in candidates {
        let outcome = match candidate.rejected {
            Some(error) => Err(error),
            None => to_input(&candidate.item),
        };
        match outcome {
            Err(error) => problems.push(RowError { row: candidate.row, message: error.to_string() }),
            Ok(input) => {
                let key = input.name.to_lowercase();
                if let Some(first) = seen.get(&key) {
                    problems.push(RowError {
                        row: candidate.row,
                        message: format!("name: the name repeats row {first}. Keep one row per name"),
                    });
                } else {
                    seen.insert(key, candidate.row);
                    inputs.push(input);
                }
            }
        }
    }
    Ok((inputs, problems))
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ImportResult {
    pub created: usize,
    pub updated: usize,
    pub skipped: usize,
    pub problems: Vec<RowError>,
    pub dry_run: bool,
}

impl ImportResult {
    pub fn summary(&self) -> String {
        let prefix = if self.dry_run { "Dry run: would have " } else { "" };
        format!(
            "{prefix}created {}, updated {}, unchanged {}, rejected {}",
            self.created,
            self.updated,
            self.skipped,
            words::count(self.problems.len() as i64, "row", "rows")
        )
    }
}

pub fn import(records: &Records<'_>, raw: &[u8], format: Format, dry_run: bool) -> Result<ImportResult> {
    let (inputs, problems) = parse(raw, format)?;
    let transaction = records.begin()?;
    let (_, now, zone) = records.parts();
    let (created, updated, skipped) = records::apply_import(&transaction, now, zone, &inputs)?;
    if dry_run {
        transaction.rollback()?;
    } else {
        transaction.commit()?;
    }
    Ok(ImportResult { created, updated, skipped, problems, dry_run })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clock::{Clock, Zone};
    use crate::records::{Query, Status};
    use crate::store::Store;
    use crate::testkit::{TempDir, fixed_time};
    use crate::theme::Severity;
    use proptest::prelude::*;

    fn open(directory: &TempDir) -> Store {
        Store::open_ready(&directory.path().join("app.db")).unwrap()
    }

    fn service(store: &Store) -> Records<'_> {
        Records::new(store.conn(), Clock::fixed(fixed_time()), Zone::Utc)
    }

    fn everything(records: &Records<'_>) -> Vec<Record> {
        records.list(&Query { include_disabled: true, ..Query::default() }).unwrap()
    }

    #[test]
    fn an_export_can_be_imported_into_an_empty_database_in_both_formats() {
        let directory = TempDir::new("transfer-roundtrip");
        let source = open(&directory);
        let records = service(&source);
        records
            .create(&Input {
                name: "=SUM(A1), \"quoted\"".to_string(),
                note: "two\nlines, and a comma".to_string(),
                status: Status::InProgress,
                severity: Severity::High,
                enabled: false,
            })
            .unwrap();
        records.create(&Input { name: "plain".to_string(), ..Input::default() }).unwrap();
        let original: Vec<Input> = everything(&records).iter().map(Record::input).collect();
        for format in [Format::Csv, Format::Json] {
            let text = export(format, &everything(&records)).unwrap();
            if format == Format::Csv {
                assert!(text.starts_with("name,status,severity,note,enabled\n"), "{text}");
                assert!(text.contains("\"'=SUM(A1), \"\"quoted\"\"\""), "{text}");
            }
            let other = TempDir::new("transfer-target");
            let target = open(&other);
            let copy = service(&target);
            let result = import(&copy, text.as_bytes(), format, false).unwrap();
            assert_eq!(
                (result.created, result.updated, result.skipped, result.problems.len()),
                (2, 0, 0, 0),
                "{}",
                format.label()
            );
            assert_eq!(everything(&copy).iter().map(Record::input).collect::<Vec<_>>(), original);
            let again = import(&copy, text.as_bytes(), format, false).unwrap();
            assert_eq!((again.created, again.updated, again.skipped), (0, 0, 2));
        }
    }

    #[test]
    fn a_dry_run_reports_and_changes_nothing() {
        let directory = TempDir::new("transfer-dry");
        let store = open(&directory);
        let records = service(&store);
        records.create(&Input { name: "kept".to_string(), ..Input::default() }).unwrap();
        let text = "name,status,severity\nkept,done,low\nfresh,open,info\n,open,info\n";
        let result = import(&records, text.as_bytes(), Format::Csv, true).unwrap();
        assert_eq!(result.summary(), "Dry run: would have created 1, updated 1, unchanged 0, rejected 1 row");
        assert_eq!(everything(&records).len(), 1);
        assert_eq!(everything(&records)[0].status, Status::Open);
        let real = import(&records, text.as_bytes(), Format::Csv, false).unwrap();
        assert_eq!(real.summary(), "created 1, updated 1, unchanged 0, rejected 1 row");
        assert_eq!(everything(&records).len(), 2);
    }

    #[test]
    fn bad_rows_are_listed_with_their_number_and_good_rows_still_load() {
        let text = "Name,Status,Severity,Note,Enabled\nfirst,open,low,,yes\nsecond,closed,low,,\nthird,open,urgent,,\nfourth,open,low,,maybe\nFIRST,open,low,,\n";
        let (inputs, problems) = parse(text.as_bytes(), Format::Csv).unwrap();
        assert_eq!(inputs.iter().map(|input| input.name.as_str()).collect::<Vec<_>>(), vec!["first"]);
        let listed: Vec<String> = problems.iter().map(RowError::to_string).collect();
        assert_eq!(listed.len(), 4, "{listed:?}");
        assert!(listed[0].starts_with("row 3: status: unknown status \"closed\""), "{}", listed[0]);
        assert!(listed[1].starts_with("row 4: severity: unknown severity"), "{}", listed[1]);
        assert!(listed[2].starts_with("row 5: enabled:"), "{}", listed[2]);
        assert_eq!(listed[3], "row 6: name: the name repeats row 2. Keep one row per name");
    }

    #[test]
    fn unreadable_imports_are_refused_whole() {
        let cases: [(&[u8], Format, &str); 7] = [
            (b"", Format::Csv, "the import is empty"),
            (b"\xef\xbb\xbf  \n", Format::Csv, "the import is empty"),
            (b"status,severity\nopen,low\n", Format::Csv, "no \"name\" column"),
            (b"name\n\"open", Format::Csv, "never closed"),
            (b"{\"name\":\"x\"}", Format::Json, "the JSON cannot be read"),
            (b"[{\"name\":\"x\",\"owner\":\"y\"}]", Format::Json, "the JSON cannot be read"),
            (b"\xff\xfe", Format::Json, "not valid UTF-8"),
        ];
        for (raw, format, expected) in cases {
            let error = parse(raw, format).unwrap_err().to_string();
            assert!(error.contains(expected), "{error}");
        }
        let large = vec![b'a'; MAX_IMPORT_BYTES + 1];
        assert!(parse(&large, Format::Csv).unwrap_err().to_string().contains("larger than 5 MB"));
        let (inputs, _) = parse(b"\xef\xbb\xbfname\nwith bom\n", Format::Csv).unwrap();
        assert_eq!(inputs[0].name, "with bom");
        let (inputs, _) = parse(b"[{\"name\":\"minimal\"}]", Format::Json).unwrap();
        assert_eq!((inputs[0].status, inputs[0].severity, inputs[0].enabled), (Status::Open, Severity::Info, true));
    }

    #[test]
    fn the_format_comes_from_the_flag_or_the_file_name() {
        assert_eq!(Format::for_file("list.CSV", "").unwrap(), Format::Csv);
        assert_eq!(Format::for_file("list.json", "").unwrap(), Format::Json);
        assert_eq!(Format::for_file("list.txt", "json").unwrap(), Format::Json);
        assert!(Format::for_file("list.txt", "").unwrap_err().to_string().contains("--format csv"));
        assert!(Format::for_file("list.csv", "xml").unwrap_err().to_string().contains("use csv or json"));
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]

        #[test]
        fn parsing_arbitrary_bytes_never_panics(raw in proptest::collection::vec(proptest::num::u8::ANY, 0..300)) {
            let _ = parse(&raw, Format::Csv);
            let _ = parse(&raw, Format::Json);
        }

        #[test]
        fn any_valid_record_survives_a_csv_round_trip(name in "[^\\p{C}]{1,40}", note in "[^\\p{C}]{0,60}", enabled in proptest::bool::ANY) {
            let Ok(input) = records::normalize(&Input { name, note, enabled, ..Input::default() }) else { return Ok(()); };
            let record = Record { id: 1, name: input.name.clone(), status: input.status, severity: input.severity, note: input.note.clone(), enabled: input.enabled, demo: false, revision: 1, created_at: fixed_time(), updated_at: fixed_time() };
            let text = export(Format::Csv, std::slice::from_ref(&record)).unwrap();
            let (inputs, problems) = parse(text.as_bytes(), Format::Csv).unwrap();
            prop_assert!(problems.is_empty(), "{problems:?}");
            prop_assert_eq!(inputs, vec![input]);
        }
    }
}
