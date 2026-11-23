use crate::db::{Database, DbError};
use chrono::Utc;
use rusqlite::{params, OptionalExtension};

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct CaseRecord {
    pub id: i64,
    pub title: String,
    pub description: String,
    pub status: String,
    pub priority: String,
    pub assignee: String,
    pub created_at: String,
    pub updated_at: String,
    pub closed_at: Option<String>,
    pub alert_count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct CaseNote {
    pub id: i64,
    pub case_id: i64,
    pub author: String,
    pub body: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CaseError {
    InvalidStatus,
    InvalidPriority,
    EmptyTitle,
}

impl std::fmt::Display for CaseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidStatus => f.write_str("case status must be open, in_progress, or closed"),
            Self::InvalidPriority => {
                f.write_str("case priority must be low, medium, high, or critical")
            }
            Self::EmptyTitle => f.write_str("case title must not be empty"),
        }
    }
}

impl std::error::Error for CaseError {}

pub struct CaseManager<'a> {
    db: &'a Database,
}

impl<'a> CaseManager<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    pub fn create(
        &self,
        title: &str,
        description: &str,
        priority: &str,
        assignee: &str,
    ) -> Result<i64, Box<dyn std::error::Error>> {
        let title = title.trim();
        if title.is_empty() {
            return Err(Box::new(CaseError::EmptyTitle));
        }
        if !matches!(priority, "low" | "medium" | "high" | "critical") {
            return Err(Box::new(CaseError::InvalidPriority));
        }
        let now = Utc::now().to_rfc3339();
        self.db.conn().execute(
            "INSERT INTO cases (title, description, priority, assignee, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
            params![title, description, priority, assignee.trim(), now],
        )?;
        Ok(self.db.conn().last_insert_rowid())
    }

    pub fn get(&self, case_id: i64) -> Result<Option<CaseRecord>, DbError> {
        self.db.conn().query_row(
            "SELECT c.id, c.title, c.description, c.status, c.priority, c.assignee, c.created_at, c.updated_at, c.closed_at, COUNT(ca.alert_id)
             FROM cases c LEFT JOIN case_alerts ca ON ca.case_id = c.id WHERE c.id = ?1 GROUP BY c.id",
            params![case_id], map_case,
        ).optional().map_err(DbError::from)
    }

    pub fn list(&self, status: Option<&str>) -> Result<Vec<CaseRecord>, DbError> {
        let sql = match status {
            Some(_) => "SELECT c.id, c.title, c.description, c.status, c.priority, c.assignee, c.created_at, c.updated_at, c.closed_at, COUNT(ca.alert_id) FROM cases c LEFT JOIN case_alerts ca ON ca.case_id = c.id WHERE c.status = ?1 GROUP BY c.id ORDER BY CASE c.priority WHEN 'critical' THEN 0 WHEN 'high' THEN 1 WHEN 'medium' THEN 2 ELSE 3 END, c.updated_at DESC",
            None => "SELECT c.id, c.title, c.description, c.status, c.priority, c.assignee, c.created_at, c.updated_at, c.closed_at, COUNT(ca.alert_id) FROM cases c LEFT JOIN case_alerts ca ON ca.case_id = c.id GROUP BY c.id ORDER BY CASE c.priority WHEN 'critical' THEN 0 WHEN 'high' THEN 1 WHEN 'medium' THEN 2 ELSE 3 END, c.updated_at DESC",
        };
        let mut statement = self.db.conn().prepare(sql)?;
        let mapper = |row: &rusqlite::Row<'_>| map_case(row);
        let rows = match status {
            Some(value) => statement.query_map(params![value], mapper)?,
            None => statement.query_map([], mapper)?,
        };
        rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
    }

    pub fn set_status(
        &self,
        case_id: i64,
        status: &str,
    ) -> Result<bool, Box<dyn std::error::Error>> {
        if !matches!(status, "open" | "in_progress" | "closed") {
            return Err(Box::new(CaseError::InvalidStatus));
        }
        let now = Utc::now().to_rfc3339();
        let changed = self.db.conn().execute(
            "UPDATE cases SET status = ?1, updated_at = ?2, closed_at = CASE WHEN ?1 = 'closed' THEN ?2 ELSE NULL END WHERE id = ?3",
            params![status, now, case_id],
        )?;
        Ok(changed != 0)
    }

    pub fn assign(&self, case_id: i64, assignee: &str) -> Result<bool, DbError> {
        let changed = self.db.conn().execute(
            "UPDATE cases SET assignee = ?1, updated_at = ?2 WHERE id = ?3",
            params![assignee.trim(), Utc::now().to_rfc3339(), case_id],
        )?;
        Ok(changed != 0)
    }

    pub fn link_alert(&self, case_id: i64, alert_id: i64) -> Result<(), DbError> {
        let tx = self.db.conn().unchecked_transaction()?;
        let now = Utc::now().to_rfc3339();
        tx.execute(
            "INSERT INTO case_alerts (case_id, alert_id, linked_at) VALUES (?1, ?2, ?3)",
            params![case_id, alert_id, now],
        )?;
        tx.execute(
            "UPDATE alerts SET case_ref = CAST(?1 AS TEXT), updated_at = ?2 WHERE id = ?3",
            params![case_id, now, alert_id],
        )?;
        tx.execute(
            "UPDATE cases SET updated_at = ?1 WHERE id = ?2",
            params![now, case_id],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn unlink_alert(&self, case_id: i64, alert_id: i64) -> Result<bool, DbError> {
        let tx = self.db.conn().unchecked_transaction()?;
        let changed = tx.execute(
            "DELETE FROM case_alerts WHERE case_id = ?1 AND alert_id = ?2",
            params![case_id, alert_id],
        )?;
        if changed != 0 {
            tx.execute(
                "UPDATE alerts SET case_ref = '', updated_at = ?1 WHERE id = ?2 AND case_ref = ?3",
                params![Utc::now().to_rfc3339(), alert_id, case_id.to_string()],
            )?;
        }
        tx.commit()?;
        Ok(changed != 0)
    }

    pub fn add_note(&self, case_id: i64, author: &str, body: &str) -> Result<i64, DbError> {
        let body = body.trim();
        if body.is_empty() {
            return Err(DbError::Sqlite(rusqlite::Error::InvalidQuery));
        }
        let now = Utc::now().to_rfc3339();
        self.db.conn().execute(
            "INSERT INTO case_notes (case_id, author, body, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![case_id, author.trim(), body, now],
        )?;
        self.db.conn().execute(
            "UPDATE cases SET updated_at = ?1 WHERE id = ?2",
            params![now, case_id],
        )?;
        Ok(self.db.conn().last_insert_rowid())
    }

    pub fn notes(&self, case_id: i64) -> Result<Vec<CaseNote>, DbError> {
        let mut statement = self.db.conn().prepare("SELECT id, case_id, author, body, created_at FROM case_notes WHERE case_id = ?1 ORDER BY id")?;
        let rows = statement.query_map(params![case_id], |row| {
            Ok(CaseNote {
                id: row.get(0)?,
                case_id: row.get(1)?,
                author: row.get(2)?,
                body: row.get(3)?,
                created_at: row.get(4)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
    }

    pub fn alerts(&self, case_id: i64) -> Result<Vec<crate::model::Alert>, DbError> {
        let mut statement = self.db.conn().prepare("SELECT a.id, a.rule_id, a.event_id, a.severity, a.status, a.title, a.case_ref, a.created_at, a.updated_at FROM alerts a JOIN case_alerts ca ON ca.alert_id = a.id WHERE ca.case_id = ?1 ORDER BY a.created_at DESC, a.id DESC")?;
        let rows = statement.query_map(params![case_id], |r| {
            Ok(crate::model::Alert {
                id: r.get(0)?,
                rule_id: r.get(1)?,
                event_id: r.get(2)?,
                severity: r.get(3)?,
                status: r.get(4)?,
                title: r.get(5)?,
                case_ref: r.get(6)?,
                created_at: r.get(7)?,
                updated_at: r.get(8)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
    }
}

fn map_case(row: &rusqlite::Row<'_>) -> rusqlite::Result<CaseRecord> {
    Ok(CaseRecord {
        id: row.get(0)?,
        title: row.get(1)?,
        description: row.get(2)?,
        status: row.get(3)?,
        priority: row.get(4)?,
        assignee: row.get(5)?,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
        closed_at: row.get(8)?,
        alert_count: row.get(9)?,
    })
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct CaseQueueMetrics {
    pub open_cases: i64,
    pub in_progress_cases: i64,
    pub closed_cases: i64,
    pub unassigned_active: i64,
    pub critical_active: i64,
    pub overdue_active: i64,
    pub oldest_active_at: Option<String>,
    pub average_active_age_hours: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct AssigneeWorkload {
    pub assignee: String,
    pub active_cases: i64,
    pub critical_cases: i64,
    pub oldest_case_at: Option<String>,
}

pub fn case_queue_metrics(
    db: &Database,
    as_of_utc: &str,
    overdue_after_hours: i64,
) -> Result<CaseQueueMetrics, Box<dyn std::error::Error>> {
    if !(0..=24 * 365).contains(&overdue_after_hours) {
        return Err(Box::new(CaseError::InvalidStatus));
    }
    let as_of = chrono::DateTime::parse_from_rfc3339(as_of_utc)?.with_timezone(&Utc);
    let (open_cases, in_progress_cases, closed_cases, unassigned_active, critical_active): (
        i64,
        i64,
        i64,
        i64,
        i64,
    ) = db.conn().query_row(
        "SELECT \
            SUM(CASE WHEN status = 'open' THEN 1 ELSE 0 END), \
            SUM(CASE WHEN status = 'in_progress' THEN 1 ELSE 0 END), \
            SUM(CASE WHEN status = 'closed' THEN 1 ELSE 0 END), \
            SUM(CASE WHEN status <> 'closed' AND trim(assignee) = '' THEN 1 ELSE 0 END), \
            SUM(CASE WHEN status <> 'closed' AND priority = 'critical' THEN 1 ELSE 0 END) \
         FROM cases",
        [],
        |row| {
            Ok((
                row.get::<_, Option<i64>>(0)?.unwrap_or(0),
                row.get::<_, Option<i64>>(1)?.unwrap_or(0),
                row.get::<_, Option<i64>>(2)?.unwrap_or(0),
                row.get::<_, Option<i64>>(3)?.unwrap_or(0),
                row.get::<_, Option<i64>>(4)?.unwrap_or(0),
            ))
        },
    )?;
    let mut statement = db
        .conn()
        .prepare("SELECT created_at FROM cases WHERE status <> 'closed' ORDER BY created_at ASC")?;
    let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
    let mut active_ages = Vec::new();
    let mut overdue_active = 0;
    for row in rows {
        let created = chrono::DateTime::parse_from_rfc3339(&row?)?.with_timezone(&Utc);
        let hours = as_of.signed_duration_since(created).num_hours().max(0);
        if hours >= overdue_after_hours {
            overdue_active += 1;
        }
        active_ages.push((created, hours));
    }
    let oldest_active_at = active_ages.first().map(|(created, _)| created.to_rfc3339());
    let average_active_age_hours = if active_ages.is_empty() {
        0
    } else {
        active_ages.iter().map(|(_, age)| age).sum::<i64>() / active_ages.len() as i64
    };
    Ok(CaseQueueMetrics {
        open_cases,
        in_progress_cases,
        closed_cases,
        unassigned_active,
        critical_active,
        overdue_active,
        oldest_active_at,
        average_active_age_hours,
    })
}

pub fn case_workload_by_assignee(db: &Database) -> Result<Vec<AssigneeWorkload>, DbError> {
    let mut statement = db.conn().prepare(
        "SELECT CASE WHEN trim(assignee) = '' THEN '(unassigned)' ELSE assignee END, \
            COUNT(*), SUM(CASE WHEN priority = 'critical' THEN 1 ELSE 0 END), MIN(created_at) \
         FROM cases WHERE status <> 'closed' \
         GROUP BY CASE WHEN trim(assignee) = '' THEN '(unassigned)' ELSE assignee END \
         ORDER BY COUNT(*) DESC, 1 COLLATE NOCASE",
    )?;
    let rows = statement.query_map([], |row| {
        Ok(AssigneeWorkload {
            assignee: row.get(0)?,
            active_cases: row.get(1)?,
            critical_cases: row.get::<_, Option<i64>>(2)?.unwrap_or(0),
            oldest_case_at: row.get(3)?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
}
