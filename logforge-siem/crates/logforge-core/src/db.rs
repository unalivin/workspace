use rusqlite::{params, Connection};
use std::path::Path;
use std::time::Duration;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DbError {
    #[error("database error: {0}")]
    Sqlite(#[from] rusqlite::Error),
}

pub struct Migration {
    pub version: u32,
    pub sql: &'static str,
}

const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        sql: include_str!("../../../migrations/0001_initial.sql"),
    },
    Migration {
        version: 2,
        sql: include_str!("../../../migrations/0002_category_colors_and_retention.sql"),
    },
    Migration {
        version: 3,
        sql: include_str!("../../../migrations/0003_malformed_events.sql"),
    },
    Migration {
        version: 4,
        sql: include_str!("../../../migrations/0004_correlation_state.sql"),
    },
    Migration {
        version: 5,
        sql: include_str!("../../../migrations/0005_retention_bytes.sql"),
    },
    Migration {
        version: 6,
        sql: include_str!("../../../migrations/0006_cases.sql"),
    },
    Migration {
        version: 7,
        sql: include_str!("../../../migrations/0007_correlation_rules.sql"),
    },
    Migration {
        version: 8,
        sql: include_str!("../../../migrations/0008_notification_outbox.sql"),
    },
];

pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn open(path: &Path) -> Result<Self, DbError> {
        let conn = Connection::open(path)?;
        Self::configure_and_migrate(conn)
    }

    pub fn open_in_memory() -> Result<Self, DbError> {
        let conn = Connection::open_in_memory()?;
        Self::configure_and_migrate(conn)
    }

    fn configure_and_migrate(conn: Connection) -> Result<Self, DbError> {
        conn.busy_timeout(Duration::from_secs(5))?;
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA foreign_keys = ON;",
        )?;

        let current_version: u32 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;

        for migration in MIGRATIONS {
            if migration.version > current_version {
                let tx = conn.unchecked_transaction()?;
                tx.execute_batch(migration.sql)?;
                tx.pragma_update(None, "user_version", migration.version)?;
                tx.commit()?;
            }
        }

        Ok(Self { conn })
    }

    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    pub fn data_version(&self) -> Result<i64, DbError> {
        Ok(self
            .conn
            .query_row("PRAGMA data_version", [], |row| row.get(0))?)
    }

    pub fn get_setting(&self, key: &str) -> Result<Option<String>, DbError> {
        let mut stmt = self
            .conn
            .prepare("SELECT value FROM settings WHERE key = ?1")?;
        let mut rows = stmt.query(params![key])?;
        if let Some(row) = rows.next()? {
            Ok(Some(row.get(0)?))
        } else {
            Ok(None)
        }
    }

    pub fn set_setting(&self, key: &str, value: &str, now_utc: &str) -> Result<(), DbError> {
        self.conn.execute(
            "INSERT INTO settings (key, value, updated_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
            params![key, value, now_utc],
        )?;
        Ok(())
    }
}
