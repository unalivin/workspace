CREATE TABLE IF NOT EXISTS settings (
    key TEXT PRIMARY KEY NOT NULL,
    value TEXT NOT NULL,
    updated_at TEXT NOT NULL
) STRICT;

CREATE TABLE IF NOT EXISTS sources (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    kind TEXT NOT NULL CHECK (kind IN ('file', 'api', 'synthetic')),
    location TEXT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
    created_at TEXT NOT NULL
) STRICT;

CREATE TABLE IF NOT EXISTS events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    source_id INTEGER NOT NULL REFERENCES sources(id) ON DELETE CASCADE,
    timestamp_utc TEXT NOT NULL,
    received_at TEXT NOT NULL,
    severity TEXT NOT NULL CHECK (severity IN ('debug', 'info', 'warn', 'error', 'critical')),
    host TEXT NOT NULL,
    service TEXT NOT NULL,
    message TEXT NOT NULL,
    raw_line TEXT NOT NULL,
    dedup_hash TEXT NOT NULL,
    redacted INTEGER NOT NULL DEFAULT 0 CHECK (redacted IN (0, 1)),
    fields_json TEXT NOT NULL DEFAULT '{}'
) STRICT;

CREATE INDEX IF NOT EXISTS idx_events_timestamp ON events(timestamp_utc);
CREATE INDEX IF NOT EXISTS idx_events_source_id ON events(source_id);
CREATE INDEX IF NOT EXISTS idx_events_dedup_hash ON events(dedup_hash);
CREATE INDEX IF NOT EXISTS idx_events_severity ON events(severity);

CREATE VIRTUAL TABLE IF NOT EXISTS events_fts USING fts5(
    message,
    host,
    service,
    content='events',
    content_rowid='id'
);

CREATE TRIGGER IF NOT EXISTS trg_events_ai AFTER INSERT ON events BEGIN
    INSERT INTO events_fts(rowid, message, host, service)
    VALUES (new.id, new.message, new.host, new.service);
END;

CREATE TRIGGER IF NOT EXISTS trg_events_ad AFTER DELETE ON events BEGIN
    INSERT INTO events_fts(events_fts, rowid, message, host, service)
    VALUES ('delete', old.id, old.message, old.host, old.service);
END;

CREATE TABLE IF NOT EXISTS saved_queries (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    query TEXT NOT NULL,
    created_at TEXT NOT NULL
) STRICT;

CREATE TABLE IF NOT EXISTS detection_rules (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    severity TEXT NOT NULL CHECK (severity IN ('debug', 'info', 'warn', 'error', 'critical')),
    pattern TEXT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
    created_at TEXT NOT NULL
) STRICT;

CREATE TABLE IF NOT EXISTS alerts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    rule_id INTEGER NOT NULL REFERENCES detection_rules(id) ON DELETE CASCADE,
    event_id INTEGER NOT NULL REFERENCES events(id) ON DELETE CASCADE,
    severity TEXT NOT NULL CHECK (severity IN ('debug', 'info', 'warn', 'error', 'critical')),
    status TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'acknowledged', 'resolved')),
    title TEXT NOT NULL,
    case_ref TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
) STRICT;

CREATE INDEX IF NOT EXISTS idx_alerts_status_severity ON alerts(status, severity);

CREATE TABLE IF NOT EXISTS notification_rules (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    event_type TEXT NOT NULL UNIQUE,
    min_severity TEXT NOT NULL CHECK (min_severity IN ('debug', 'info', 'warn', 'error', 'critical')),
    enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
    quiet_start_hour INTEGER NOT NULL DEFAULT 0 CHECK (quiet_start_hour BETWEEN 0 AND 23),
    quiet_end_hour INTEGER NOT NULL DEFAULT 0 CHECK (quiet_end_hour BETWEEN 0 AND 23)
) STRICT;

CREATE TABLE IF NOT EXISTS audit_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    actor TEXT NOT NULL,
    action TEXT NOT NULL,
    target TEXT NOT NULL,
    created_at TEXT NOT NULL,
    prev_hash TEXT NOT NULL,
    entry_hash TEXT NOT NULL
) STRICT;

CREATE TABLE IF NOT EXISTS ingestion_stats (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    received INTEGER NOT NULL DEFAULT 0,
    deduplicated INTEGER NOT NULL DEFAULT 0,
    dropped INTEGER NOT NULL DEFAULT 0,
    malformed INTEGER NOT NULL DEFAULT 0
) STRICT;

INSERT OR IGNORE INTO ingestion_stats (id, received, deduplicated, dropped, malformed)
VALUES (1, 0, 0, 0, 0);

CREATE VIEW IF NOT EXISTS v_overview_counts AS
SELECT
    (SELECT COUNT(*) FROM events) AS total_events,
    (SELECT COUNT(*) FROM sources WHERE enabled = 1) AS active_sources,
    (SELECT COUNT(*) FROM alerts WHERE status = 'open') AS open_alerts,
    (SELECT received FROM ingestion_stats WHERE id = 1) AS stat_received,
    (SELECT deduplicated FROM ingestion_stats WHERE id = 1) AS stat_deduplicated,
    (SELECT dropped FROM ingestion_stats WHERE id = 1) AS stat_dropped,
    (SELECT malformed FROM ingestion_stats WHERE id = 1) AS stat_malformed;

CREATE VIEW IF NOT EXISTS v_top_sources AS
SELECT
    s.id,
    s.name,
    s.kind,
    COUNT(e.id) AS event_count
FROM sources s
LEFT JOIN events e ON e.source_id = s.id
GROUP BY s.id, s.name, s.kind
ORDER BY event_count DESC, s.name ASC;

CREATE VIEW IF NOT EXISTS v_alerts_by_severity AS
SELECT
    severity,
    COUNT(*) AS alert_count
FROM alerts
WHERE status = 'open'
GROUP BY severity;
