CREATE TABLE IF NOT EXISTS category_colors (
    kind TEXT NOT NULL,
    value TEXT NOT NULL,
    colour TEXT NOT NULL,
    PRIMARY KEY (kind, value)
) STRICT;

CREATE TABLE IF NOT EXISTS retention_policies (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    max_days INTEGER NOT NULL CHECK (max_days > 0),
    max_events INTEGER NOT NULL CHECK (max_events > 0),
    enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
    updated_at TEXT NOT NULL
) STRICT;

INSERT OR IGNORE INTO retention_policies (id, name, max_days, max_events, enabled, updated_at)
VALUES (1, 'default', 90, 1000000, 1, '2026-11-21T00:00:00Z');

CREATE TABLE IF NOT EXISTS case_links (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    alert_id INTEGER NOT NULL REFERENCES alerts(id) ON DELETE CASCADE,
    case_ref TEXT NOT NULL,
    note TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL
) STRICT;
