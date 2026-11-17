use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;

use rusqlite::{Connection, OpenFlags, Transaction, TransactionBehavior};

use crate::error::{AppError, Result};
use crate::files;

pub const DEFAULT_BUSY_TIMEOUT: Duration = Duration::from_secs(5);
const FILE_HEADER: &[u8] = b"SQLite format 3\0";

const SQL_SCHEMA_SNAPSHOT: &str = "SELECT type, name, coalesce(sql, '') FROM sqlite_master WHERE name NOT LIKE 'sqlite_%' AND name NOT LIKE 'records_fts_%' ORDER BY type, name";
const SQL_COMPILE_OPTIONS: &str = "PRAGMA compile_options";
const SQL_FOREIGN_KEY_CHECK: &str = "PRAGMA foreign_key_check";

pub const EMBEDDED_MIGRATIONS: &[(&str, &str)] = &[
    ("0001_init.sql", include_str!("../../../migrations/0001_init.sql")),
    ("0002_scanning.sql", include_str!("../../../migrations/0002_scanning.sql")),
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Migration {
    pub number: i64,
    pub name: String,
    pub sql: String,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Options {
    pub read_only: bool,
    pub busy_timeout: Option<Duration>,
}

pub struct Store {
    conn: Connection,
    path: PathBuf,
    read_only: bool,
}

fn migration_number(name: &str) -> Option<i64> {
    let stem = name.strip_suffix(".sql")?;
    let (number, title) = stem.split_once('_')?;
    let valid_title = !title.is_empty()
        && title.bytes().all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_');
    if number.len() != 4 || !number.bytes().all(|byte| byte.is_ascii_digit()) || !valid_title {
        return None;
    }
    number.parse().ok()
}

pub fn load_migrations(files: &[(&str, &str)]) -> Result<Vec<Migration>> {
    let mut migrations = Vec::new();
    for (name, content) in files {
        let number = migration_number(name)
            .ok_or_else(|| AppError::Storage(format!("migration \"{name}\" must be named NNNN_name.sql")))?;
        let text = content.trim();
        let marker = format!("user_version = {number};");
        if !text.ends_with(&format!("PRAGMA {marker}")) {
            return Err(AppError::Storage(format!("migration {name} must end with \"{marker}\"")));
        }
        migrations.push(Migration { number, name: (*name).to_string(), sql: text.to_string() });
    }
    migrations.sort_by_key(|migration| migration.number);
    for (index, migration) in migrations.iter().enumerate() {
        if migration.number != index as i64 + 1 {
            return Err(AppError::Storage(format!(
                "migrations must be numbered without gaps from 0001: found {} at position {}",
                migration.name,
                index + 1
            )));
        }
    }
    if migrations.is_empty() {
        return Err(AppError::Storage("no migrations found".to_string()));
    }
    Ok(migrations)
}

pub fn embedded_migrations() -> Result<Vec<Migration>> {
    load_migrations(EMBEDDED_MIGRATIONS)
}

fn check_header(path: &Path) -> Result<()> {
    let mut file = match fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(AppError::Storage(format!("the database file \"{}\" cannot be read: {error}", path.display())));
        }
    };
    let mut header = Vec::with_capacity(FILE_HEADER.len());
    let read = file.by_ref().take(FILE_HEADER.len() as u64).read_to_end(&mut header);
    match read {
        Ok(0) => Ok(()),
        Ok(_) if header == FILE_HEADER => Ok(()),
        _ => Err(unusable(path, "it does not start with the SQLite file header")),
    }
}

fn unusable(path: &Path, reason: impl std::fmt::Display) -> AppError {
    AppError::Storage(format!(
        "\"{}\" is not a usable SQLite database and was left untouched; choose another file ({reason})",
        path.display()
    ))
}

impl Store {
    pub fn open(path: &Path, options: Options) -> Result<Store> {
        if !path.is_absolute() {
            return Err(AppError::Usage(format!("database path \"{}\" must be absolute", path.display())));
        }
        if path.is_dir() {
            return Err(AppError::Usage(format!(
                "database path \"{}\" is a directory: choose a file path",
                path.display()
            )));
        }
        check_header(path)?;
        let flags = if options.read_only {
            if !path.exists() {
                return Err(AppError::Storage(format!(
                    "database \"{}\" cannot be opened read-only: the file does not exist",
                    path.display()
                )));
            }
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX
        } else {
            if let Some(parent) = path.parent() {
                files::create_private_dir(parent)?;
            }
            if !path.exists() {
                files::write_private(path, b"", false)?;
            }
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE | OpenFlags::SQLITE_OPEN_NO_MUTEX
        };
        let conn = Connection::open_with_flags(path, flags).map_err(|error| unusable(path, error))?;
        conn.busy_timeout(options.busy_timeout.unwrap_or(DEFAULT_BUSY_TIMEOUT))?;
        conn.pragma_update(None, "foreign_keys", true).map_err(|error| unusable(path, error))?;
        if options.read_only {
            conn.pragma_update(None, "query_only", true).map_err(|error| unusable(path, error))?;
        } else {
            let mode: String = conn
                .pragma_update_and_check(None, "journal_mode", "WAL", |row| row.get(0))
                .map_err(|error| unusable(path, error))?;
            if !mode.eq_ignore_ascii_case("wal") {
                return Err(AppError::Storage(format!(
                    "\"{}\" could not be switched to write-ahead logging (journal mode {mode})",
                    path.display()
                )));
            }
            conn.pragma_update(None, "synchronous", "NORMAL")?;
        }
        let store = Store { conn, path: path.to_path_buf(), read_only: options.read_only };
        store.schema_version().map_err(|error| unusable(path, error))?;
        Ok(store)
    }

    pub fn open_ready(path: &Path) -> Result<Store> {
        let store = Store::open(path, Options::default())?;
        store.migrate(&embedded_migrations()?)?;
        Ok(store)
    }

    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn read_only(&self) -> bool {
        self.read_only
    }

    pub fn begin(&self) -> Result<Transaction<'_>> {
        Ok(Transaction::new_unchecked(&self.conn, TransactionBehavior::Immediate)?)
    }

    fn pragma_i64(&self, name: &str) -> Result<i64> {
        Ok(self.conn.pragma_query_value(None, name, |row| row.get(0))?)
    }

    pub fn schema_version(&self) -> Result<i64> {
        self.pragma_i64("user_version")
    }

    pub fn data_version(&self) -> Result<i64> {
        self.pragma_i64("data_version")
    }

    pub fn journal_mode(&self) -> Result<String> {
        Ok(self.conn.pragma_query_value(None, "journal_mode", |row| row.get(0))?)
    }

    pub fn foreign_keys_enabled(&self) -> Result<bool> {
        Ok(self.pragma_i64("foreign_keys")? == 1)
    }

    pub fn busy_timeout(&self) -> Result<Duration> {
        Ok(Duration::from_millis(self.pragma_i64("busy_timeout")?.max(0) as u64))
    }

    pub fn migrate(&self, migrations: &[Migration]) -> Result<usize> {
        if self.read_only {
            return Err(AppError::Storage("a read-only database cannot be migrated".to_string()));
        }
        let current = self.schema_version()?;
        if current > migrations.len() as i64 {
            return Err(AppError::Storage(format!(
                "the database has schema version {current} but this build knows only {}: use a newer build",
                migrations.len()
            )));
        }
        let mut applied = 0;
        for migration in migrations.iter().filter(|migration| migration.number > current) {
            let done = self.apply_migration(migration).map_err(|error| {
                AppError::Storage(format!("migration {} failed and was rolled back: {error}", migration.name))
            })?;
            if done {
                applied += 1;
            }
        }
        Ok(applied)
    }

    fn apply_migration(&self, migration: &Migration) -> Result<bool> {
        let transaction = self.begin()?;
        let current: i64 = transaction.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if current >= migration.number {
            return Ok(false);
        }
        if current != migration.number - 1 {
            return Err(AppError::Storage(format!(
                "expected schema version {}, found {current}",
                migration.number - 1
            )));
        }
        transaction.execute_batch(&migration.sql)?;
        transaction.commit()?;
        Ok(true)
    }

    pub fn integrity_check(&self) -> Result<()> {
        let result: String = self.conn.pragma_query_value(None, "integrity_check", |row| row.get(0))?;
        if result == "ok" { Ok(()) } else { Err(AppError::Storage(format!("integrity check reported: {result}"))) }
    }

    pub fn foreign_key_check(&self) -> Result<()> {
        let mut statement = self.conn.prepare(SQL_FOREIGN_KEY_CHECK)?;
        if statement.exists([])? {
            return Err(AppError::Storage("foreign key check reported at least one violation".to_string()));
        }
        Ok(())
    }

    pub fn schema_snapshot(&self) -> Result<String> {
        let mut statement = self.conn.prepare(SQL_SCHEMA_SNAPSHOT)?;
        let rows = statement
            .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?)))?;
        let mut snapshot = String::new();
        for row in rows {
            let (kind, name, sql) = row?;
            snapshot.push_str(&format!("[{kind} {name}]\n{}\n\n", sql.trim()));
        }
        Ok(snapshot)
    }

    pub fn has_full_text_search(&self) -> Result<bool> {
        let mut statement = self.conn.prepare(SQL_COMPILE_OPTIONS)?;
        let options = statement.query_map([], |row| row.get::<_, String>(0))?;
        for option in options {
            if option? == "ENABLE_FTS5" {
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub fn query_plan(&self, sql: &str, parameters: usize) -> Result<Vec<String>> {
        let mut statement = self.conn.prepare(&format!("EXPLAIN QUERY PLAN {sql}"))?;
        let values: Vec<rusqlite::types::Value> = (0..parameters).map(|_| rusqlite::types::Value::Integer(1)).collect();
        let rows = statement.query_map(rusqlite::params_from_iter(values), |row| row.get::<_, String>(3))?;
        let mut plan = Vec::new();
        for row in rows {
            plan.push(row?);
        }
        Ok(plan)
    }
}

pub fn is_constraint(error: &rusqlite::Error) -> bool {
    matches!(error, rusqlite::Error::SqliteFailure(code, _) if code.code == rusqlite::ErrorCode::ConstraintViolation)
}

pub fn is_busy(error: &rusqlite::Error) -> bool {
    matches!(error, rusqlite::Error::SqliteFailure(code, _) if matches!(code.code, rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::TempDir;

    fn ready(directory: &TempDir) -> Store {
        Store::open_ready(&directory.path().join("app.db")).unwrap()
    }

    #[test]
    fn a_new_database_is_created_private_migrated_and_configured() {
        let directory = TempDir::new("store-new");
        let path = directory.path().join("nested").join("app.db");
        let store = Store::open(&path, Options::default()).unwrap();
        assert_eq!(store.schema_version().unwrap(), 0);
        assert_eq!(store.migrate(&embedded_migrations().unwrap()).unwrap(), EMBEDDED_MIGRATIONS.len());
        assert_eq!(store.migrate(&embedded_migrations().unwrap()).unwrap(), 0);
        assert_eq!(store.schema_version().unwrap(), EMBEDDED_MIGRATIONS.len() as i64);
        assert_eq!(store.journal_mode().unwrap().to_lowercase(), "wal");
        assert!(store.foreign_keys_enabled().unwrap());
        assert_eq!(store.busy_timeout().unwrap(), DEFAULT_BUSY_TIMEOUT);
        assert!(store.has_full_text_search().unwrap());
        store.integrity_check().unwrap();
        store.foreign_key_check().unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
            assert_eq!(fs::metadata(path.parent().unwrap()).unwrap().permissions().mode() & 0o777, 0o700);
        }
    }

    #[test]
    fn schema_matches_the_committed_snapshot() {
        let directory = TempDir::new("store-snapshot");
        let snapshot = ready(&directory).schema_snapshot().unwrap();
        let golden = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata/schema.snapshot");
        if std::env::var("UPDATE_SNAPSHOT").is_ok() {
            fs::write(&golden, &snapshot).unwrap();
        }
        let expected = fs::read_to_string(&golden).unwrap().replace("\r\n", "\n");
        assert_eq!(
            snapshot, expected,
            "run UPDATE_SNAPSHOT=1 cargo test -p secretsentry-core schema_matches to accept the change"
        );
    }

    #[test]
    fn files_that_are_not_databases_are_refused_and_left_untouched() {
        let directory = TempDir::new("store-garbage");
        let path = directory.path().join("notes.txt");
        fs::write(&path, "these are my notes, not a database").unwrap();
        let error = Store::open(&path, Options::default()).err().unwrap();
        assert!(error.to_string().contains("left untouched"), "{error}");
        assert_eq!(fs::read_to_string(&path).unwrap(), "these are my notes, not a database");
        let error = Store::open(directory.path(), Options::default()).err().unwrap();
        assert!(error.to_string().contains("is a directory"), "{error}");
        let error = Store::open(Path::new("relative.db"), Options::default()).err().unwrap();
        assert!(error.to_string().contains("must be absolute"), "{error}");
    }

    #[test]
    fn read_only_stores_reject_writes_and_missing_files() {
        let directory = TempDir::new("store-readonly");
        let path = directory.path().join("app.db");
        let missing = Store::open(&path, Options { read_only: true, busy_timeout: None }).err().unwrap();
        assert!(missing.to_string().contains("read-only"), "{missing}");
        drop(Store::open_ready(&path).unwrap());
        let store = Store::open(&path, Options { read_only: true, busy_timeout: None }).unwrap();
        assert!(store.read_only());
        assert!(
            store.conn().execute("INSERT INTO settings (key, value, updated_at) VALUES ('a', 'b', 'c')", []).is_err()
        );
        assert!(store.migrate(&embedded_migrations().unwrap()).is_err());
    }

    #[test]
    fn a_failing_migration_is_rolled_back_completely() {
        let directory = TempDir::new("store-rollback");
        let store = ready(&directory);
        let mut migrations = embedded_migrations().unwrap();
        let current = migrations.len() as i64;
        migrations.push(Migration {
            number: current + 1,
            name: "9999_broken.sql".to_string(),
            sql: "CREATE TABLE half_done (id INTEGER PRIMARY KEY); INSERT INTO missing_table VALUES (1); PRAGMA user_version = 99;"
                .to_string(),
        });
        let error = store.migrate(&migrations).unwrap_err();
        assert!(error.to_string().contains("9999_broken.sql failed and was rolled back"), "{error}");
        assert_eq!(store.schema_version().unwrap(), current);
        assert!(!store.schema_snapshot().unwrap().contains("half_done"));
    }

    #[test]
    fn a_database_from_a_newer_build_is_refused() {
        let directory = TempDir::new("store-newer");
        let store = ready(&directory);
        store.conn().pragma_update(None, "user_version", 99).unwrap();
        let error = store.migrate(&embedded_migrations().unwrap()).unwrap_err();
        assert!(error.to_string().contains("use a newer build"), "{error}");
    }

    #[test]
    fn migration_files_must_be_named_numbered_and_terminated() {
        assert!(load_migrations(&[]).is_err());
        assert!(load_migrations(&[("init.sql", "PRAGMA user_version = 1;")]).is_err());
        assert!(load_migrations(&[("0001_init.sql", "CREATE TABLE a (b);")]).is_err());
        assert!(load_migrations(&[("0002_second.sql", "PRAGMA user_version = 2;")]).is_err());
        let loaded =
            load_migrations(&[("0002_b.sql", "PRAGMA user_version = 2;"), ("0001_a.sql", "PRAGMA user_version = 1;")])
                .unwrap();
        assert_eq!(loaded.iter().map(|migration| migration.number).collect::<Vec<_>>(), vec![1, 2]);
    }

    #[test]
    fn a_second_writer_waits_then_reports_a_busy_database() {
        let directory = TempDir::new("store-busy");
        let path = directory.path().join("app.db");
        let first = Store::open_ready(&path).unwrap();
        let second =
            Store::open(&path, Options { read_only: false, busy_timeout: Some(Duration::from_millis(50)) }).unwrap();
        let held = first.begin().unwrap();
        held.execute("INSERT INTO settings (key, value, updated_at) VALUES ('a', 'b', 'c')", []).unwrap();
        let error = second.begin().err().unwrap();
        assert!(error.to_string().contains("database is busy"), "{error}");
        held.commit().unwrap();
        second.begin().unwrap().commit().unwrap();
        let version_before = second.data_version().unwrap();
        first.conn().execute("INSERT INTO settings (key, value, updated_at) VALUES ('d', 'e', 'f')", []).unwrap();
        assert_ne!(second.data_version().unwrap(), version_before);
    }
}
