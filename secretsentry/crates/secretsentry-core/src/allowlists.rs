use rusqlite::{Connection, OptionalExtension, Row, Transaction, TransactionBehavior};

use crate::allow::{self, EntrySpec, Kind};
use crate::clock::{Clock, Timestamp};
use crate::error::{AppError, Result};
use crate::store;

const SQL_LIST: &str = "SELECT id, kind, pattern, rule_id, note, enabled, is_demo, revision, created_at, updated_at FROM allowlist_entries ORDER BY kind, pattern, rule_id, id";
const SQL_GET: &str = "SELECT id, kind, pattern, rule_id, note, enabled, is_demo, revision, created_at, updated_at FROM allowlist_entries WHERE id = ?1";
const SQL_INSERT: &str = "INSERT INTO allowlist_entries (kind, pattern, rule_id, note, enabled, is_demo, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)";
const SQL_UPDATE: &str = "UPDATE allowlist_entries SET kind = ?1, pattern = ?2, rule_id = ?3, note = ?4, enabled = ?5, revision = revision + 1, updated_at = ?6 WHERE id = ?7 AND revision = ?8";
const SQL_SET_ENABLED: &str = "UPDATE allowlist_entries SET enabled = ?1, revision = revision + 1, updated_at = ?2 WHERE id = ?3 AND enabled <> ?1";
const SQL_DELETE: &str = "DELETE FROM allowlist_entries WHERE id = ?1";
const SQL_DELETE_DEMO: &str = "DELETE FROM allowlist_entries WHERE is_demo = 1";
const SQL_COUNT: &str = "SELECT count(*) FROM allowlist_entries";

pub const HOT_QUERIES: &[(&str, &str, usize)] = &[("allowlist entry by identifier", SQL_GET, 1)];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredEntry {
    pub spec: EntrySpec,
    pub demo: bool,
    pub revision: i64,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

type RawEntry = (i64, String, String, String, String, i64, i64, i64, String, String);

fn scan(row: &Row<'_>) -> rusqlite::Result<RawEntry> {
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

fn build(raw: RawEntry) -> Result<StoredEntry> {
    let (id, kind, pattern, rule_id, note, enabled, demo, revision, created, updated) = raw;
    let unreadable = |what: &str| AppError::Storage(format!("allowlist entry {id} has an unreadable {what}"));
    Ok(StoredEntry {
        spec: EntrySpec {
            id,
            kind: Kind::parse(&kind).ok_or_else(|| unreadable("kind"))?,
            pattern,
            rule_id,
            note,
            enabled: enabled == 1,
        },
        demo: demo == 1,
        revision,
        created_at: Timestamp::parse(&created).ok_or_else(|| unreadable("creation time"))?,
        updated_at: Timestamp::parse(&updated).ok_or_else(|| unreadable("update time"))?,
    })
}

fn not_found(id: i64) -> AppError {
    AppError::NotFound(format!("allowlist entry {id} (list the allowlist to see valid identifiers)"))
}

fn duplicate(error: rusqlite::Error, spec: &EntrySpec) -> AppError {
    if store::is_constraint(&error) && error.to_string().contains("allowlist_entries.kind") {
        return AppError::invalid(
            "pattern",
            format!("the same {} already exists", spec.kind.label().to_lowercase()),
            "Edit the existing entry instead of adding it again",
        );
    }
    error.into()
}

pub fn insert(conn: &Connection, now: Timestamp, spec: &EntrySpec, demo: bool) -> Result<i64> {
    let cleaned = allow::validate(spec)?;
    conn.execute(
        SQL_INSERT,
        (
            cleaned.kind.as_str(),
            &cleaned.pattern,
            &cleaned.rule_id,
            &cleaned.note,
            i64::from(cleaned.enabled),
            i64::from(demo),
            now.to_rfc3339(),
        ),
    )
    .map_err(|error| duplicate(error, &cleaned))?;
    Ok(conn.last_insert_rowid())
}

pub fn remove_demo(conn: &Connection) -> Result<usize> {
    Ok(conn.execute(SQL_DELETE_DEMO, [])?)
}

pub struct Allowlists<'a> {
    conn: &'a Connection,
    clock: Clock,
}

impl<'a> Allowlists<'a> {
    pub fn new(conn: &'a Connection, clock: Clock) -> Self {
        Allowlists { conn, clock }
    }

    fn begin(&self) -> Result<Transaction<'a>> {
        Ok(Transaction::new_unchecked(self.conn, TransactionBehavior::Immediate)?)
    }

    fn find(conn: &Connection, id: i64) -> Result<Option<StoredEntry>> {
        conn.query_row(SQL_GET, [id], scan).optional()?.map(build).transpose()
    }

    pub fn list(&self) -> Result<Vec<StoredEntry>> {
        let mut statement = self.conn.prepare(SQL_LIST)?;
        let rows = statement.query_map([], scan)?;
        let mut entries = Vec::new();
        for row in rows {
            entries.push(build(row?)?);
        }
        Ok(entries)
    }

    pub fn count(&self) -> Result<i64> {
        Ok(self.conn.query_row(SQL_COUNT, [], |row| row.get(0))?)
    }

    pub fn get(&self, id: i64) -> Result<StoredEntry> {
        Allowlists::find(self.conn, id)?.ok_or_else(|| not_found(id))
    }

    pub fn enabled_specs(&self) -> Result<Vec<EntrySpec>> {
        Ok(self.list()?.into_iter().map(|entry| entry.spec).filter(|spec| spec.enabled).collect())
    }

    pub fn create(&self, spec: &EntrySpec) -> Result<StoredEntry> {
        let id = insert(self.conn, self.clock.now(), spec, false)?;
        self.get(id)
    }

    pub fn update(&self, id: i64, revision: i64, spec: &EntrySpec) -> Result<StoredEntry> {
        let cleaned = allow::validate(spec)?;
        let transaction = self.begin()?;
        let changed = transaction
            .execute(
                SQL_UPDATE,
                (
                    cleaned.kind.as_str(),
                    &cleaned.pattern,
                    &cleaned.rule_id,
                    &cleaned.note,
                    i64::from(cleaned.enabled),
                    self.clock.now().to_rfc3339(),
                    id,
                    revision,
                ),
            )
            .map_err(|error| duplicate(error, &cleaned))?;
        if changed == 0 {
            return Err(if Allowlists::find(&transaction, id)?.is_some() {
                AppError::Conflict(format!(
                    "allowlist entry {id} was changed by another window: reload it and apply your change again"
                ))
            } else {
                not_found(id)
            });
        }
        transaction.commit()?;
        self.get(id)
    }

    pub fn set_enabled(&self, id: i64, enabled: bool) -> Result<StoredEntry> {
        let transaction = self.begin()?;
        if Allowlists::find(&transaction, id)?.is_none() {
            return Err(not_found(id));
        }
        transaction.execute(SQL_SET_ENABLED, (i64::from(enabled), self.clock.now().to_rfc3339(), id))?;
        transaction.commit()?;
        self.get(id)
    }

    pub fn delete(&self, id: i64) -> Result<()> {
        if self.conn.execute(SQL_DELETE, [id])? == 0 {
            return Err(not_found(id));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::Store;
    use crate::testkit::{TempDir, fixed_time};

    struct Fixture {
        _directory: TempDir,
        store: Store,
    }

    fn fixture(label: &str) -> Fixture {
        let directory = TempDir::new(label);
        let store = Store::open_ready(&directory.path().join("app.db")).unwrap();
        Fixture { _directory: directory, store }
    }

    fn entry(kind: Kind, pattern: &str) -> EntrySpec {
        EntrySpec { kind, pattern: pattern.to_string(), ..EntrySpec::default() }
    }

    #[test]
    fn entries_are_created_listed_edited_disabled_and_removed() {
        let fixture = fixture("allowlists-crud");
        let (clock, hands) = Clock::manual(fixed_time());
        let list = Allowlists::new(fixture.store.conn(), clock);
        assert_eq!(list.count().unwrap(), 0);
        let path = list.create(&EntrySpec { note: " fixtures ".to_string(), ..entry(Kind::Path, " tests/** ") }).unwrap();
        assert_eq!((path.spec.pattern.as_str(), path.spec.note.as_str()), ("tests/**", "fixtures"));
        assert_eq!((path.revision, path.demo, path.created_at), (1, false, fixed_time()));
        let value = list
            .create(&EntrySpec { rule_id: "aws-access-key-id".to_string(), ..entry(Kind::Value, "EXAMPLE$") })
            .unwrap();
        let print = list.create(&entry(Kind::Fingerprint, &"AB".repeat(32))).unwrap();
        assert_eq!(print.spec.pattern, "ab".repeat(32));
        assert_eq!(
            list.list().unwrap().iter().map(|entry| entry.spec.kind).collect::<Vec<_>>(),
            vec![Kind::Fingerprint, Kind::Path, Kind::Value]
        );

        hands.advance(60);
        let edited = list.update(value.spec.id, value.revision, &entry(Kind::Path, "docs/")).unwrap();
        assert_eq!((edited.spec.kind, edited.spec.pattern.as_str(), edited.spec.rule_id.as_str()), (Kind::Path, "docs/", ""));
        assert_eq!(edited.revision, 2);
        assert_eq!(edited.updated_at, fixed_time().plus_seconds(60));
        assert_eq!(edited.created_at, fixed_time());

        let stale = list.update(value.spec.id, 1, &entry(Kind::Path, "other/")).unwrap_err();
        assert!(matches!(stale, AppError::Conflict(_)), "{stale}");

        assert!(!list.set_enabled(path.spec.id, false).unwrap().spec.enabled);
        assert_eq!(list.set_enabled(path.spec.id, false).unwrap().revision, 2);
        assert_eq!(list.enabled_specs().unwrap().len(), 2);

        list.delete(print.spec.id).unwrap();
        for error in [
            list.delete(print.spec.id).unwrap_err(),
            list.get(print.spec.id).unwrap_err(),
            list.set_enabled(print.spec.id, true).unwrap_err(),
            list.update(print.spec.id, 1, &entry(Kind::Path, "x")).unwrap_err(),
        ] {
            assert!(matches!(error, AppError::NotFound(_)), "{error}");
        }
        assert_eq!(list.count().unwrap(), 2);
    }

    #[test]
    fn invalid_and_duplicate_entries_are_refused() {
        let fixture = fixture("allowlists-invalid");
        let list = Allowlists::new(fixture.store.conn(), Clock::fixed(fixed_time()));
        let broken = list.create(&entry(Kind::Value, "(")).unwrap_err().to_string();
        assert!(broken.starts_with("pattern: the pattern is not valid"), "{broken}");
        let first = list.create(&entry(Kind::Path, "tests/**")).unwrap();
        let twice = list.create(&entry(Kind::Path, " tests/** ")).unwrap_err().to_string();
        assert_eq!(twice, "pattern: the same path pattern already exists. Edit the existing entry instead of adding it again");
        let scoped = list.create(&EntrySpec { rule_id: "npm-token".to_string(), ..entry(Kind::Path, "tests/**") }).unwrap();
        let clash = list.update(scoped.spec.id, scoped.revision, &entry(Kind::Path, "tests/**")).unwrap_err().to_string();
        assert!(clash.starts_with("pattern: the same path pattern already exists"), "{clash}");
        assert!(list.update(first.spec.id, first.revision, &entry(Kind::Fingerprint, "short")).is_err());
        assert_eq!(list.count().unwrap(), 2);
        assert_eq!(list.get(first.spec.id).unwrap().revision, 1);
    }

    #[test]
    fn demo_entries_are_labelled_and_removed_alone() {
        let fixture = fixture("allowlists-demo");
        let list = Allowlists::new(fixture.store.conn(), Clock::fixed(fixed_time()));
        list.create(&entry(Kind::Path, "real/**")).unwrap();
        let id = insert(fixture.store.conn(), fixed_time(), &entry(Kind::Path, "demo/**"), true).unwrap();
        assert!(list.get(id).unwrap().demo);
        assert_eq!(remove_demo(fixture.store.conn()).unwrap(), 1);
        assert_eq!(list.list().unwrap().len(), 1);
        assert_eq!(list.list().unwrap()[0].spec.pattern, "real/**");
    }

    #[test]
    fn hot_queries_use_an_index() {
        let fixture = fixture("allowlists-plan");
        for (name, sql, parameters) in HOT_QUERIES {
            let plan = fixture.store.query_plan(sql, *parameters).unwrap().join(" | ");
            assert!(plan.contains("USING") && !plan.contains("SCAN"), "{name}: {plan}");
        }
    }
}
