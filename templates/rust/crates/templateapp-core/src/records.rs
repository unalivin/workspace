use rusqlite::{Connection, OptionalExtension, Row, Transaction, TransactionBehavior};

use crate::clock::{Clock, Timestamp, Zone};
use crate::error::{AppError, Result};
use crate::notify::{self, Event};
use crate::store;
use crate::theme::Severity;

pub const MAX_NAME_LENGTH: usize = 200;
pub const MAX_NOTE_LENGTH: usize = 4000;
pub const KIND: &str = "record";

const SQL_INSERT: &str = "INSERT INTO records (name, status, severity, note, enabled, is_demo, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)";
const SQL_GET: &str = "SELECT id, name, status, severity, note, enabled, is_demo, revision, created_at, updated_at FROM records WHERE id = ?1";
const SQL_GET_BY_NAME: &str = "SELECT id, name, status, severity, note, enabled, is_demo, revision, created_at, updated_at FROM records WHERE name = ?1";
const SQL_UPDATE: &str = "UPDATE records SET name = ?1, status = ?2, severity = ?3, note = ?4, enabled = ?5, revision = revision + 1, updated_at = ?6 WHERE id = ?7 AND revision = ?8";
const SQL_SET_ENABLED: &str =
    "UPDATE records SET enabled = ?1, revision = revision + 1, updated_at = ?2 WHERE id = ?3 AND enabled <> ?1";
const SQL_DELETE: &str = "DELETE FROM records WHERE id = ?1";
const SQL_DELETE_DEMO: &str = "DELETE FROM records WHERE is_demo = 1";
const SQL_COUNT_DEMO: &str = "SELECT count(*) FROM records WHERE is_demo = 1";
const SQL_COUNT: &str = "SELECT count(*) FROM records";
const SQL_LIST: &str = "SELECT id, name, status, severity, note, enabled, is_demo, revision, created_at, updated_at
FROM records
WHERE (?1 = '' OR status = ?1)
  AND (?2 = '' OR severity = ?2)
  AND (?3 = 1 OR enabled = 1)
  AND (?4 = '' OR id IN (SELECT rowid FROM records_fts WHERE records_fts MATCH ?4))
ORDER BY
  CASE WHEN ?5 = 'name' AND ?6 = 0 THEN name END COLLATE NOCASE ASC,
  CASE WHEN ?5 = 'name' AND ?6 = 1 THEN name END COLLATE NOCASE DESC,
  CASE WHEN ?5 = 'status' AND ?6 = 0 THEN status END ASC,
  CASE WHEN ?5 = 'status' AND ?6 = 1 THEN status END DESC,
  CASE WHEN ?5 = 'severity' AND ?6 = 0 THEN CASE severity WHEN 'critical' THEN 4 WHEN 'high' THEN 3 WHEN 'medium' THEN 2 WHEN 'low' THEN 1 ELSE 0 END END ASC,
  CASE WHEN ?5 = 'severity' AND ?6 = 1 THEN CASE severity WHEN 'critical' THEN 4 WHEN 'high' THEN 3 WHEN 'medium' THEN 2 WHEN 'low' THEN 1 ELSE 0 END END DESC,
  CASE WHEN ?5 = 'updated' AND ?6 = 0 THEN updated_at END ASC,
  CASE WHEN ?5 = 'updated' AND ?6 = 1 THEN updated_at END DESC,
  id ASC
LIMIT ?7 OFFSET ?8";
const SQL_BY_STATUS: &str = "SELECT id FROM records WHERE status = ?1 AND severity = ?2";
const SQL_RECENT: &str = "SELECT id, name, status, severity, note, enabled, is_demo, revision, created_at, updated_at FROM records ORDER BY updated_at DESC LIMIT ?1";
const SQL_OVERVIEW: &str =
    "SELECT open_records, urgent_records, disabled_records, demo_records, unread_notifications FROM overview_attention";
const SQL_OVERVIEW_ROWS: &str = "SELECT status, severity, total, demo_total FROM overview_record_counts";

pub const HOT_QUERIES: &[(&str, &str, usize)] = &[
    ("records by status and severity", SQL_BY_STATUS, 2),
    ("recently updated records", SQL_RECENT, 1),
    ("record by name", SQL_GET_BY_NAME, 1),
    ("record by identifier", SQL_GET, 1),
];

pub const SORT_KEYS: &[&str] = &["name", "status", "severity", "updated"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Status {
    Open,
    InProgress,
    Done,
}

impl Status {
    pub const ALL: [Status; 3] = [Status::Open, Status::InProgress, Status::Done];

    pub fn parse(text: &str) -> Option<Status> {
        let wanted = text.trim().to_lowercase();
        Status::ALL.into_iter().find(|status| status.as_str() == wanted)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Status::Open => "open",
            Status::InProgress => "in_progress",
            Status::Done => "done",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Status::Open => "Open",
            Status::InProgress => "In progress",
            Status::Done => "Done",
        }
    }

    pub fn glyph(self) -> &'static str {
        match self {
            Status::Open => "○",
            Status::InProgress => "◐",
            Status::Done => "●",
        }
    }
}

pub fn column_label(column: &str) -> &str {
    match column {
        "id" => "ID",
        "name" => "Name",
        "status" => "Status",
        "severity" => "Severity",
        "updated" => "Updated",
        "state" => "State",
        other => other,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    pub id: i64,
    pub name: String,
    pub status: Status,
    pub severity: Severity,
    pub note: String,
    pub enabled: bool,
    pub demo: bool,
    pub revision: i64,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Input {
    pub name: String,
    pub status: Status,
    pub severity: Severity,
    pub note: String,
    pub enabled: bool,
}

impl Default for Input {
    fn default() -> Self {
        Input {
            name: String::new(),
            status: Status::Open,
            severity: Severity::Info,
            note: String::new(),
            enabled: true,
        }
    }
}

impl Record {
    pub fn input(&self) -> Input {
        Input {
            name: self.name.clone(),
            status: self.status,
            severity: self.severity,
            note: self.note.clone(),
            enabled: self.enabled,
        }
    }

    pub fn state_label(&self) -> &'static str {
        match (self.enabled, self.demo) {
            (true, false) => "Active",
            (true, true) => "Active, demo",
            (false, false) => "Disabled",
            (false, true) => "Disabled, demo",
        }
    }
}

pub fn parse_status(text: &str) -> Result<Status> {
    Status::parse(text).ok_or_else(|| {
        AppError::invalid("status", format!("unknown status \"{text}\""), "Use open, in_progress or done")
    })
}

pub fn parse_severity(text: &str) -> Result<Severity> {
    Severity::parse(text).ok_or_else(|| {
        AppError::invalid("severity", format!("unknown severity \"{text}\""), "Use info, low, medium, high or critical")
    })
}

pub fn normalize(input: &Input) -> Result<Input> {
    let name = input.name.split_whitespace().collect::<Vec<_>>().join(" ");
    if name.chars().any(char::is_control) {
        return Err(AppError::invalid(
            "name",
            "the name contains a control character",
            "Use letters, digits, spaces and punctuation",
        ));
    }
    if name.is_empty() {
        return Err(AppError::invalid("name", "the name is empty", "Enter a name of 1 to 200 characters"));
    }
    if name.chars().count() > MAX_NAME_LENGTH {
        return Err(AppError::invalid(
            "name",
            format!("the name has more than {MAX_NAME_LENGTH} characters"),
            "Shorten the name",
        ));
    }
    let note = input.note.replace("\r\n", "\n").trim().to_string();
    if note.chars().any(|letter| letter.is_control() && letter != '\n' && letter != '\t') {
        return Err(AppError::invalid("note", "the note contains a control character", "Remove the control character"));
    }
    if note.chars().count() > MAX_NOTE_LENGTH {
        return Err(AppError::invalid(
            "note",
            format!("the note has more than {MAX_NOTE_LENGTH} characters"),
            "Shorten the note",
        ));
    }
    Ok(Input { name, status: input.status, severity: input.severity, note, enabled: input.enabled })
}

pub fn search_expression(text: &str) -> String {
    text.split(|letter: char| !letter.is_alphanumeric())
        .filter(|field| !field.is_empty())
        .take(16)
        .map(|field| format!("\"{}\"*", field.chars().take(64).collect::<String>()))
        .collect::<Vec<_>>()
        .join(" ")
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Query {
    pub search: String,
    pub status: Option<Status>,
    pub severity: Option<Severity>,
    pub include_disabled: bool,
    pub sort: String,
    pub descending: bool,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Breakdown {
    pub status: Status,
    pub severity: Severity,
    pub total: i64,
    pub demo: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Overview {
    pub open: i64,
    pub urgent: i64,
    pub disabled: i64,
    pub demo: i64,
    pub unread_notifications: i64,
    pub breakdown: Vec<Breakdown>,
}

impl Overview {
    pub fn total(&self) -> i64 {
        self.breakdown.iter().map(|row| row.total).sum()
    }

    pub fn by_status(&self, status: Status) -> i64 {
        self.breakdown.iter().filter(|row| row.status == status).map(|row| row.total).sum()
    }

    pub fn by_severity(&self, severity: Severity) -> i64 {
        self.breakdown
            .iter()
            .filter(|row| row.severity == severity && row.status != Status::Done)
            .map(|row| row.total)
            .sum()
    }
}

type RawRecord = (i64, String, String, String, String, i64, i64, i64, String, String);

fn scan(row: &Row<'_>) -> rusqlite::Result<RawRecord> {
    Ok((
        row.get(0)?,
        row.get(1)?,
        row.get(2)?,
        row.get(3)?,
        row.get(4)?,
        row.get(5)?,
        row.get(6)?,
        row.get(7)?,
        row.get(8)?,
        row.get(9)?,
    ))
}

fn build(raw: RawRecord) -> Result<Record> {
    let (id, name, status, severity, note, enabled, demo, revision, created, updated) = raw;
    let unreadable = |what: &str| AppError::Storage(format!("record {id} has an unreadable {what}"));
    Ok(Record {
        id,
        name,
        status: Status::parse(&status).ok_or_else(|| unreadable("status"))?,
        severity: Severity::parse(&severity).ok_or_else(|| unreadable("severity"))?,
        note,
        enabled: enabled == 1,
        demo: demo == 1,
        revision,
        created_at: Timestamp::parse(&created).ok_or_else(|| unreadable("creation time"))?,
        updated_at: Timestamp::parse(&updated).ok_or_else(|| unreadable("update time"))?,
    })
}

fn find(conn: &Connection, id: i64) -> Result<Option<Record>> {
    conn.query_row(SQL_GET, [id], scan).optional()?.map(build).transpose()
}

fn not_found(id: i64) -> AppError {
    AppError::NotFound(format!("record {id} (list the records to see valid identifiers)"))
}

fn urgent(status: Status, severity: Severity) -> bool {
    status != Status::Done && severity >= Severity::High
}

fn duplicate_name(error: rusqlite::Error, name: &str) -> AppError {
    if store::is_constraint(&error) && error.to_string().contains("records.name") {
        return AppError::invalid(
            "name",
            format!("a record named \"{name}\" already exists"),
            "Choose another name or edit the existing record",
        );
    }
    error.into()
}

fn announce(conn: &Connection, now: Timestamp, zone: Zone, before: Option<&Record>, after: &Record) -> Result<()> {
    if !after.enabled {
        return Ok(());
    }
    let event = |event_type: &str, severity: Severity, title: &str| Event {
        event_type: event_type.to_string(),
        severity,
        title: title.to_string(),
        record_kind: KIND.to_string(),
        record_id: after.id,
        dedup_key: String::new(),
    };
    let was_urgent = before.is_some_and(|record| record.enabled && urgent(record.status, record.severity));
    if urgent(after.status, after.severity) && !was_urgent {
        notify::publish_with(
            conn,
            now,
            zone,
            &event(notify::EVENT_RECORD_URGENT, after.severity, "A record needs attention"),
        )?;
    }
    if after.status == Status::Done && before.is_some_and(|record| record.status != Status::Done) {
        notify::publish_with(
            conn,
            now,
            zone,
            &event(notify::EVENT_RECORD_COMPLETED, Severity::Info, "A record was completed"),
        )?;
    }
    Ok(())
}

fn create(conn: &Connection, now: Timestamp, zone: Zone, input: &Input, demo: bool) -> Result<Record> {
    conn.execute(
        SQL_INSERT,
        (
            &input.name,
            input.status.as_str(),
            input.severity.as_str(),
            &input.note,
            i64::from(input.enabled),
            i64::from(demo),
            now.to_rfc3339(),
        ),
    )
    .map_err(|error| duplicate_name(error, &input.name))?;
    let id = conn.last_insert_rowid();
    let created = find(conn, id)?.ok_or_else(|| not_found(id))?;
    announce(conn, now, zone, None, &created)?;
    Ok(created)
}

pub struct DemoRecord {
    pub name: &'static str,
    pub status: Status,
    pub severity: Severity,
    pub note: &'static str,
    pub enabled: bool,
}

pub const DEMO_RECORDS: &[DemoRecord] = &[
    DemoRecord {
        name: "Demo: edge router firmware review",
        status: Status::Open,
        severity: Severity::High,
        note: "Sample entry. Compare the installed firmware with the vendor advisory list.",
        enabled: true,
    },
    DemoRecord {
        name: "Demo: rotate backup credentials",
        status: Status::InProgress,
        severity: Severity::Critical,
        note: "Sample entry. Rotation is scheduled for the next maintenance window.",
        enabled: true,
    },
    DemoRecord {
        name: "Demo: document the lab network",
        status: Status::Open,
        severity: Severity::Low,
        note: "Sample entry. Diagram the lab VLANs and their gateways.",
        enabled: true,
    },
    DemoRecord {
        name: "Demo: review firewall change log",
        status: Status::Done,
        severity: Severity::Medium,
        note: "Sample entry. No unexpected rules were found.",
        enabled: true,
    },
    DemoRecord {
        name: "Demo: archive old inventory export",
        status: Status::Open,
        severity: Severity::Info,
        note: "Sample entry. Disabled to show how hidden entries look.",
        enabled: false,
    },
    DemoRecord {
        name: "Demo: check time synchronisation",
        status: Status::Open,
        severity: Severity::Medium,
        note: "Sample entry. Two hosts drifted by more than a second.",
        enabled: true,
    },
];

pub struct Records<'a> {
    conn: &'a Connection,
    clock: Clock,
    zone: Zone,
}

impl<'a> Records<'a> {
    pub fn new(conn: &'a Connection, clock: Clock, zone: Zone) -> Self {
        Records { conn, clock, zone }
    }

    pub(crate) fn parts(&self) -> (&'a Connection, Timestamp, Zone) {
        (self.conn, self.clock.now(), self.zone)
    }

    pub(crate) fn begin(&self) -> Result<Transaction<'a>> {
        Ok(Transaction::new_unchecked(self.conn, TransactionBehavior::Immediate)?)
    }

    pub fn create(&self, input: &Input) -> Result<Record> {
        let normalized = normalize(input)?;
        let transaction = self.begin()?;
        let created = create(&transaction, self.clock.now(), self.zone, &normalized, false)?;
        transaction.commit()?;
        Ok(created)
    }

    pub fn create_all(&self, inputs: &[Input]) -> Result<Vec<Record>> {
        let transaction = self.begin()?;
        let now = self.clock.now();
        let mut created = Vec::with_capacity(inputs.len());
        for input in inputs {
            created.push(create(&transaction, now, self.zone, &normalize(input)?, false)?);
        }
        transaction.commit()?;
        Ok(created)
    }

    pub fn get(&self, id: i64) -> Result<Record> {
        find(self.conn, id)?.ok_or_else(|| not_found(id))
    }

    pub fn by_name(&self, name: &str) -> Result<Option<Record>> {
        self.conn.query_row(SQL_GET_BY_NAME, [name], scan).optional()?.map(build).transpose()
    }

    pub fn update(&self, id: i64, revision: i64, input: &Input) -> Result<Record> {
        let normalized = normalize(input)?;
        let transaction = self.begin()?;
        let Some(before) = find(&transaction, id)? else {
            return Err(AppError::Conflict(format!("record {id} was removed by another window; reload the list")));
        };
        let changed_elsewhere = || {
            AppError::Conflict(format!(
                "record {id} was changed by another window (now revision {}); reload it and apply your change again",
                before.revision
            ))
        };
        if before.revision != revision {
            return Err(changed_elsewhere());
        }
        let now = self.clock.now();
        let changed = transaction
            .execute(
                SQL_UPDATE,
                (
                    &normalized.name,
                    normalized.status.as_str(),
                    normalized.severity.as_str(),
                    &normalized.note,
                    i64::from(normalized.enabled),
                    now.to_rfc3339(),
                    id,
                    revision,
                ),
            )
            .map_err(|error| duplicate_name(error, &normalized.name))?;
        if changed != 1 {
            return Err(changed_elsewhere());
        }
        let updated = find(&transaction, id)?.ok_or_else(|| not_found(id))?;
        announce(&transaction, now, self.zone, Some(&before), &updated)?;
        transaction.commit()?;
        Ok(updated)
    }

    pub fn set_enabled(&self, id: i64, enabled: bool) -> Result<Record> {
        let transaction = self.begin()?;
        let before = find(&transaction, id)?.ok_or_else(|| not_found(id))?;
        let now = self.clock.now();
        transaction.execute(SQL_SET_ENABLED, (i64::from(enabled), now.to_rfc3339(), id))?;
        let updated = find(&transaction, id)?.ok_or_else(|| not_found(id))?;
        announce(&transaction, now, self.zone, Some(&before), &updated)?;
        transaction.commit()?;
        Ok(updated)
    }

    pub fn delete(&self, id: i64) -> Result<()> {
        if self.conn.execute(SQL_DELETE, [id])? == 0 {
            return Err(not_found(id));
        }
        Ok(())
    }

    pub fn list(&self, query: &Query) -> Result<Vec<Record>> {
        let sort = if query.sort.is_empty() { "name" } else { query.sort.as_str() };
        if !SORT_KEYS.contains(&sort) {
            return Err(AppError::invalid(
                "sort",
                format!("unknown sort column \"{}\"", query.sort),
                "Use name, status, severity or updated",
            ));
        }
        let limit = if query.limit <= 0 || query.limit > 10000 { 500 } else { query.limit };
        let search = search_expression(&query.search);
        if !query.search.trim().is_empty() && search.is_empty() {
            return Ok(Vec::new());
        }
        let mut statement = self.conn.prepare(SQL_LIST)?;
        let rows = statement.query_map(
            (
                query.status.map(Status::as_str).unwrap_or(""),
                query.severity.map(Severity::as_str).unwrap_or(""),
                i64::from(query.include_disabled),
                search,
                sort,
                i64::from(query.descending),
                limit,
                query.offset.max(0),
            ),
            scan,
        )?;
        let mut list = Vec::new();
        for row in rows {
            list.push(build(row?)?);
        }
        Ok(list)
    }

    pub fn count(&self) -> Result<i64> {
        Ok(self.conn.query_row(SQL_COUNT, [], |row| row.get(0))?)
    }

    pub fn overview(&self) -> Result<Overview> {
        let mut overview = self.conn.query_row(SQL_OVERVIEW, [], |row| {
            Ok(Overview {
                open: row.get(0)?,
                urgent: row.get(1)?,
                disabled: row.get(2)?,
                demo: row.get(3)?,
                unread_notifications: row.get(4)?,
                breakdown: Vec::new(),
            })
        })?;
        let mut statement = self.conn.prepare(SQL_OVERVIEW_ROWS)?;
        let rows = statement.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, i64>(2)?, row.get::<_, i64>(3)?))
        })?;
        for row in rows {
            let (status, severity, total, demo) = row?;
            if let (Some(status), Some(severity)) = (Status::parse(&status), Severity::parse(&severity)) {
                overview.breakdown.push(Breakdown { status, severity, total, demo });
            }
        }
        Ok(overview)
    }

    pub fn load_demo(&self) -> Result<usize> {
        let transaction = self.begin()?;
        let now = self.clock.now();
        let mut added = 0;
        for demo in DEMO_RECORDS {
            let exists: Option<i64> =
                transaction.query_row(SQL_GET_BY_NAME, [demo.name], |row| row.get(0)).optional()?;
            if exists.is_some() {
                continue;
            }
            let input = Input {
                name: demo.name.to_string(),
                status: demo.status,
                severity: demo.severity,
                note: demo.note.to_string(),
                enabled: demo.enabled,
            };
            create(&transaction, now, self.zone, &input, true)?;
            added += 1;
        }
        transaction.commit()?;
        Ok(added)
    }

    pub fn demo_count(&self) -> Result<i64> {
        Ok(self.conn.query_row(SQL_COUNT_DEMO, [], |row| row.get(0))?)
    }

    pub fn remove_demo(&self, dry_run: bool) -> Result<i64> {
        if dry_run {
            return self.demo_count();
        }
        Ok(self.conn.execute(SQL_DELETE_DEMO, [])? as i64)
    }
}

pub(crate) fn apply_import(
    conn: &Connection,
    now: Timestamp,
    zone: Zone,
    inputs: &[Input],
) -> Result<(usize, usize, usize)> {
    let (mut created, mut updated, mut skipped) = (0, 0, 0);
    for input in inputs {
        let existing = conn.query_row(SQL_GET_BY_NAME, [&input.name], scan).optional()?.map(build).transpose()?;
        let Some(before) = existing else {
            create(conn, now, zone, input, false)?;
            created += 1;
            continue;
        };
        if before.input() == *input {
            skipped += 1;
            continue;
        }
        conn.execute(
            SQL_UPDATE,
            (
                &input.name,
                input.status.as_str(),
                input.severity.as_str(),
                &input.note,
                i64::from(input.enabled),
                now.to_rfc3339(),
                before.id,
                before.revision,
            ),
        )
        .map_err(|error| duplicate_name(error, &input.name))?;
        let after = find(conn, before.id)?.ok_or_else(|| not_found(before.id))?;
        announce(conn, now, zone, Some(&before), &after)?;
        updated += 1;
    }
    Ok((created, updated, skipped))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::notify::{Filter, Notifications};
    use crate::store::Store;
    use crate::testkit::{TempDir, fixed_time};
    use proptest::prelude::*;

    struct Fixture {
        _directory: TempDir,
        store: Store,
        clock: Clock,
        hand: crate::clock::ManualClock,
    }

    fn fixture(label: &str) -> Fixture {
        let directory = TempDir::new(label);
        let store = Store::open_ready(&directory.path().join("app.db")).unwrap();
        let (clock, hand) = Clock::manual(fixed_time());
        Fixture { _directory: directory, store, clock, hand }
    }

    impl Fixture {
        fn records(&self) -> Records<'_> {
            Records::new(self.store.conn(), self.clock.clone(), Zone::Utc)
        }

        fn notifications(&self) -> Notifications<'_> {
            Notifications::new(self.store.conn(), self.clock.clone(), Zone::Utc)
        }
    }

    fn input(name: &str, status: Status, severity: Severity) -> Input {
        Input { name: name.to_string(), status, severity, note: String::new(), enabled: true }
    }

    #[test]
    fn records_are_created_read_updated_and_deleted() {
        let fixture = fixture("records-crud");
        let records = fixture.records();
        let created = records
            .create(&Input {
                name: "  Review   the  router ".to_string(),
                note: " line\r\nnext ".to_string(),
                ..Input::default()
            })
            .unwrap();
        assert_eq!(
            (created.name.as_str(), created.note.as_str(), created.revision),
            ("Review the router", "line\nnext", 1)
        );
        assert_eq!(created.created_at, fixed_time());
        fixture.hand.advance(60);
        let mut change = created.input();
        change.status = Status::InProgress;
        let updated = records.update(created.id, created.revision, &change).unwrap();
        assert_eq!((updated.status, updated.revision), (Status::InProgress, 2));
        assert_eq!(updated.updated_at, fixed_time().plus_seconds(60));
        assert_eq!(records.get(created.id).unwrap(), updated);
        assert_eq!(records.by_name("Review the router").unwrap().unwrap().id, created.id);
        assert_eq!(records.count().unwrap(), 1);
        records.delete(created.id).unwrap();
        assert!(records.get(created.id).unwrap_err().to_string().contains("was not found"));
        assert!(records.delete(created.id).is_err());
        assert_eq!(records.count().unwrap(), 0);
    }

    #[test]
    fn invalid_input_names_the_field_and_the_fix() {
        let cases = [
            (Input { name: "   ".to_string(), ..Input::default() }, "name: the name is empty"),
            (Input { name: "a".repeat(201), ..Input::default() }, "more than 200 characters"),
            (Input { name: "bell\u{7}".to_string(), ..Input::default() }, "control character"),
            (Input { name: "ok".to_string(), note: "n".repeat(4001), ..Input::default() }, "more than 4000 characters"),
            (
                Input { name: "ok".to_string(), note: "esc\u{1b}[0m".to_string(), ..Input::default() },
                "note: the note contains a control character",
            ),
        ];
        for (candidate, expected) in cases {
            let error = normalize(&candidate).unwrap_err().to_string();
            assert!(error.contains(expected), "{error}");
        }
        assert!(parse_status("closed").unwrap_err().to_string().contains("Use open, in_progress or done"));
        assert!(parse_severity("urgent").unwrap_err().to_string().contains("Use info, low, medium, high or critical"));
        assert_eq!(parse_status(" DONE ").unwrap(), Status::Done);
        assert_eq!(
            normalize(&Input { name: "tab\there".to_string(), note: "keep\ttab".to_string(), ..Input::default() })
                .unwrap()
                .note,
            "keep\ttab"
        );
    }

    #[test]
    fn duplicate_names_are_refused_with_a_clear_message() {
        let fixture = fixture("records-duplicate");
        let records = fixture.records();
        records.create(&input("Same", Status::Open, Severity::Info)).unwrap();
        let error = records.create(&input("Same", Status::Open, Severity::Info)).unwrap_err();
        assert!(error.to_string().contains("a record named \"Same\" already exists"), "{error}");
        assert!(error.is_usage());
        let other = records.create(&input("Other", Status::Open, Severity::Info)).unwrap();
        let error = records.update(other.id, other.revision, &input("Same", Status::Open, Severity::Info)).unwrap_err();
        assert!(error.to_string().contains("already exists"), "{error}");
        assert_eq!(records.count().unwrap(), 2);
    }

    #[test]
    fn a_stale_revision_or_a_removed_record_is_a_conflict() {
        let fixture = fixture("records-conflict");
        let records = fixture.records();
        let created = records.create(&input("Shared", Status::Open, Severity::Info)).unwrap();
        let first =
            records.update(created.id, created.revision, &input("Shared", Status::InProgress, Severity::Info)).unwrap();
        let stale =
            records.update(created.id, created.revision, &input("Shared", Status::Done, Severity::Info)).unwrap_err();
        assert!(stale.to_string().contains("changed by another window (now revision 2)"), "{stale}");
        assert_eq!(records.get(created.id).unwrap(), first);
        records.delete(created.id).unwrap();
        let removed = records.update(created.id, first.revision, &created.input()).unwrap_err();
        assert!(removed.to_string().contains("removed by another window"), "{removed}");
    }

    #[test]
    fn listing_filters_sorts_pages_and_searches() {
        let fixture = fixture("records-list");
        let records = fixture.records();
        let alpha = records.create(&input("alpha router", Status::Open, Severity::Critical)).unwrap();
        fixture.hand.advance(10);
        let bravo = records
            .create(&Input { note: "firewall notes".to_string(), ..input("Bravo switch", Status::Done, Severity::Low) })
            .unwrap();
        fixture.hand.advance(10);
        let charlie = records.create(&input("charlie router", Status::InProgress, Severity::Medium)).unwrap();
        records.set_enabled(charlie.id, false).unwrap();
        let ids = |query: Query| records.list(&query).unwrap().iter().map(|record| record.id).collect::<Vec<_>>();
        assert_eq!(ids(Query::default()), vec![alpha.id, bravo.id]);
        assert_eq!(ids(Query { include_disabled: true, ..Query::default() }), vec![alpha.id, bravo.id, charlie.id]);
        assert_eq!(
            ids(Query { include_disabled: true, descending: true, ..Query::default() }),
            vec![charlie.id, bravo.id, alpha.id]
        );
        assert_eq!(
            ids(Query { sort: "severity".to_string(), descending: true, ..Query::default() }),
            vec![alpha.id, bravo.id]
        );
        assert_eq!(
            ids(Query { sort: "updated".to_string(), descending: true, include_disabled: true, ..Query::default() })[0],
            charlie.id
        );
        assert_eq!(ids(Query { status: Some(Status::Done), ..Query::default() }), vec![bravo.id]);
        assert_eq!(ids(Query { severity: Some(Severity::Critical), ..Query::default() }), vec![alpha.id]);
        assert_eq!(
            ids(Query { search: "rout".to_string(), include_disabled: true, ..Query::default() }),
            vec![alpha.id, charlie.id]
        );
        assert_eq!(ids(Query { search: "FIREWALL".to_string(), ..Query::default() }), vec![bravo.id]);
        assert_eq!(ids(Query { search: "\"*()".to_string(), ..Query::default() }), Vec::<i64>::new());
        assert_eq!(ids(Query { limit: 1, offset: 1, ..Query::default() }), vec![bravo.id]);
        assert!(records.list(&Query { sort: "owner".to_string(), ..Query::default() }).is_err());
    }

    #[test]
    fn search_follows_edits_and_deletions() {
        let fixture = fixture("records-fts");
        let records = fixture.records();
        let created = records.create(&input("printer queue", Status::Open, Severity::Info)).unwrap();
        let found = |text: &str| records.list(&Query { search: text.to_string(), ..Query::default() }).unwrap().len();
        assert_eq!(found("printer"), 1);
        records.update(created.id, created.revision, &input("scanner queue", Status::Open, Severity::Info)).unwrap();
        assert_eq!((found("printer"), found("scanner")), (0, 1));
        records.delete(created.id).unwrap();
        assert_eq!(found("scanner"), 0);
    }

    #[test]
    fn urgent_and_completed_records_publish_notifications_once() {
        let fixture = fixture("records-notify");
        let records = fixture.records();
        let notifications = fixture.notifications();
        let calm = records.create(&input("calm", Status::Open, Severity::Low)).unwrap();
        assert_eq!(notifications.unread_count().unwrap(), 0);
        let raised = records.update(calm.id, calm.revision, &input("calm", Status::Open, Severity::High)).unwrap();
        assert_eq!(notifications.unread_count().unwrap(), 1);
        let still =
            records.update(raised.id, raised.revision, &input("calm", Status::InProgress, Severity::Critical)).unwrap();
        assert_eq!(notifications.unread_count().unwrap(), 1);
        records.update(still.id, still.revision, &input("calm", Status::Done, Severity::Critical)).unwrap();
        let listed = notifications.list(&Filter::default()).unwrap();
        assert_eq!(listed.len(), 2);
        assert!(
            listed
                .iter()
                .any(|item| item.event_type == notify::EVENT_RECORD_COMPLETED && item.severity == Severity::Info)
        );
        assert!(listed.iter().all(|item| item.record_id == calm.id && !item.title.contains("calm")));
        let hidden =
            records.create(&Input { enabled: false, ..input("hidden", Status::Open, Severity::Critical) }).unwrap();
        assert_eq!(notifications.list(&Filter::default()).unwrap().len(), 2);
        fixture.hand.advance(2 * 60 * 60);
        records.set_enabled(hidden.id, true).unwrap();
        assert_eq!(notifications.list(&Filter::default()).unwrap().len(), 3);
    }

    #[test]
    fn the_overview_counts_follow_the_fixture() {
        let fixture = fixture("records-overview");
        let sql = include_str!("../../../testdata/fixtures/overview.sql");
        fixture.store.conn().execute_batch(sql).unwrap();
        let overview = fixture.records().overview().unwrap();
        assert_eq!(
            (overview.open, overview.urgent, overview.disabled, overview.demo, overview.unread_notifications),
            (4, 3, 2, 2, 2)
        );
        assert_eq!(overview.total(), 5);
        assert_eq!(overview.by_status(Status::Open), 3);
        assert_eq!(overview.by_status(Status::Done), 1);
        assert_eq!(overview.by_severity(Severity::Critical), 2);
        assert_eq!(overview.by_severity(Severity::High), 1);
        assert_eq!(overview.breakdown.iter().map(|row| row.demo).sum::<i64>(), 1);
    }

    #[test]
    fn demo_data_is_labelled_loaded_once_and_removed_alone() {
        let fixture = fixture("records-demo");
        let records = fixture.records();
        let own = records.create(&input("my own record", Status::Open, Severity::Info)).unwrap();
        assert_eq!(records.load_demo().unwrap(), DEMO_RECORDS.len());
        assert_eq!(records.load_demo().unwrap(), 0);
        assert_eq!(records.demo_count().unwrap(), DEMO_RECORDS.len() as i64);
        let listed = records.list(&Query { include_disabled: true, ..Query::default() }).unwrap();
        assert!(listed.iter().filter(|record| record.demo).all(|record| record.name.starts_with("Demo: ")));
        assert!(listed.iter().any(|record| record.state_label() == "Disabled, demo"));
        assert_eq!(records.remove_demo(true).unwrap(), DEMO_RECORDS.len() as i64);
        assert_eq!(records.count().unwrap(), DEMO_RECORDS.len() as i64 + 1);
        assert_eq!(records.remove_demo(false).unwrap(), DEMO_RECORDS.len() as i64);
        assert_eq!(records.list(&Query { include_disabled: true, ..Query::default() }).unwrap(), vec![own]);
    }

    #[test]
    fn hot_queries_use_an_index() {
        let fixture = fixture("records-plans");
        for (name, sql, parameters) in HOT_QUERIES.iter().chain(notify::HOT_QUERIES) {
            let plan = fixture.store.query_plan(sql, *parameters).unwrap();
            let indexed = |step: &String| step.contains("USING INDEX") || step.contains("USING COVERING INDEX");
            let full_scan = |step: &String| step.starts_with("SCAN") && !indexed(step);
            assert!(plan.iter().any(indexed) && !plan.iter().any(full_scan), "{name}: {}", plan.join(" | "));
        }
    }

    #[test]
    fn a_batch_is_created_completely_or_not_at_all() {
        let fixture = fixture("records-batch");
        let records = fixture.records();
        let good = vec![input("one", Status::Open, Severity::Info), input("two", Status::Open, Severity::Info)];
        assert_eq!(records.create_all(&good).unwrap().len(), 2);
        let bad = vec![input("three", Status::Open, Severity::Info), input("one", Status::Open, Severity::Info)];
        assert!(records.create_all(&bad).is_err());
        assert_eq!(records.count().unwrap(), 2);
        assert!(records.by_name("three").unwrap().is_none());
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]

        #[test]
        fn search_expressions_are_always_valid_fts_queries(text in ".{0,80}") {
            let expression = search_expression(&text);
            prop_assert!(expression.split(' ').filter(|term| !term.is_empty()).count() <= 16);
            let conn = rusqlite::Connection::open_in_memory().unwrap();
            conn.execute_batch("CREATE VIRTUAL TABLE probe USING fts5 (body); INSERT INTO probe (body) VALUES ('sample text');").unwrap();
            if !expression.is_empty() {
                let matched: rusqlite::Result<i64> = conn.query_row("SELECT count(*) FROM probe WHERE probe MATCH ?1", [&expression], |row| row.get(0));
                prop_assert!(matched.is_ok(), "{expression}: {matched:?}");
            }
        }

        #[test]
        fn normalized_input_is_stable_and_within_limits(name in ".{0,220}", note in ".{0,200}") {
            let candidate = Input { name, note, ..Input::default() };
            if let Ok(once) = normalize(&candidate) {
                prop_assert!(!once.name.is_empty() && once.name.chars().count() <= MAX_NAME_LENGTH);
                prop_assert_eq!(normalize(&once).unwrap(), once);
            }
        }
    }
}
