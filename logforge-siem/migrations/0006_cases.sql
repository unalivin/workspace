CREATE TABLE IF NOT EXISTS cases (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    title TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'in_progress', 'closed')),
    priority TEXT NOT NULL DEFAULT 'medium' CHECK (priority IN ('low', 'medium', 'high', 'critical')),
    assignee TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    closed_at TEXT
) STRICT;

CREATE TABLE IF NOT EXISTS case_alerts (
    case_id INTEGER NOT NULL REFERENCES cases(id) ON DELETE CASCADE,
    alert_id INTEGER NOT NULL UNIQUE REFERENCES alerts(id) ON DELETE CASCADE,
    linked_at TEXT NOT NULL,
    PRIMARY KEY (case_id, alert_id)
) STRICT;

CREATE TABLE IF NOT EXISTS case_notes (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    case_id INTEGER NOT NULL REFERENCES cases(id) ON DELETE CASCADE,
    author TEXT NOT NULL,
    body TEXT NOT NULL,
    created_at TEXT NOT NULL
) STRICT;

CREATE INDEX IF NOT EXISTS idx_cases_status_priority ON cases(status, priority, updated_at);
CREATE INDEX IF NOT EXISTS idx_case_notes_case ON case_notes(case_id, created_at);
