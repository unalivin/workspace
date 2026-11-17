CREATE TABLE settings (
  key TEXT PRIMARY KEY CHECK (length(key) BETWEEN 1 AND 120),
  value TEXT NOT NULL CHECK (length(value) <= 4096),
  updated_at TEXT NOT NULL
) STRICT, WITHOUT ROWID;

CREATE TABLE records (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  name TEXT NOT NULL UNIQUE CHECK (length(trim(name)) BETWEEN 1 AND 200),
  status TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'in_progress', 'done')),
  severity TEXT NOT NULL DEFAULT 'info' CHECK (severity IN ('info', 'low', 'medium', 'high', 'critical')),
  note TEXT NOT NULL DEFAULT '' CHECK (length(note) <= 4000),
  enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
  is_demo INTEGER NOT NULL DEFAULT 0 CHECK (is_demo IN (0, 1)),
  revision INTEGER NOT NULL DEFAULT 1 CHECK (revision >= 1),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
) STRICT;

CREATE INDEX records_status_severity ON records (status, severity);
CREATE INDEX records_updated_at ON records (updated_at DESC);

CREATE VIRTUAL TABLE records_fts USING fts5 (name, note, content = 'records', content_rowid = 'id');

CREATE TRIGGER records_fts_insert AFTER INSERT ON records BEGIN
  INSERT INTO records_fts (rowid, name, note) VALUES (new.id, new.name, new.note);
END;

CREATE TRIGGER records_fts_delete AFTER DELETE ON records BEGIN
  INSERT INTO records_fts (records_fts, rowid, name, note) VALUES ('delete', old.id, old.name, old.note);
END;

CREATE TRIGGER records_fts_update AFTER UPDATE ON records BEGIN
  INSERT INTO records_fts (records_fts, rowid, name, note) VALUES ('delete', old.id, old.name, old.note);
  INSERT INTO records_fts (rowid, name, note) VALUES (new.id, new.name, new.note);
END;

CREATE TABLE notification_rules (
  event_type TEXT PRIMARY KEY CHECK (length(event_type) BETWEEN 1 AND 80),
  enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
  min_severity TEXT NOT NULL DEFAULT 'info' CHECK (min_severity IN ('info', 'low', 'medium', 'high', 'critical')),
  quiet_start TEXT NOT NULL DEFAULT '' CHECK (quiet_start = '' OR quiet_start GLOB '[0-2][0-9]:[0-5][0-9]'),
  quiet_end TEXT NOT NULL DEFAULT '' CHECK (quiet_end = '' OR quiet_end GLOB '[0-2][0-9]:[0-5][0-9]'),
  dedup_minutes INTEGER NOT NULL DEFAULT 60 CHECK (dedup_minutes BETWEEN 0 AND 10080),
  updated_at TEXT NOT NULL
) STRICT, WITHOUT ROWID;

CREATE TABLE notifications (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  event_type TEXT NOT NULL CHECK (length(event_type) BETWEEN 1 AND 80),
  severity TEXT NOT NULL CHECK (severity IN ('info', 'low', 'medium', 'high', 'critical')),
  title TEXT NOT NULL CHECK (length(title) BETWEEN 1 AND 200),
  record_kind TEXT NOT NULL DEFAULT '',
  record_id INTEGER NOT NULL DEFAULT 0,
  dedup_key TEXT NOT NULL,
  created_at TEXT NOT NULL,
  read_at TEXT
) STRICT;

CREATE INDEX notifications_unread ON notifications (created_at DESC) WHERE read_at IS NULL;
CREATE INDEX notifications_dedup ON notifications (event_type, dedup_key, created_at DESC);
CREATE INDEX notifications_created_at ON notifications (created_at DESC);

CREATE VIEW overview_record_counts AS
SELECT status, severity, count(*) AS total, sum(is_demo) AS demo_total
FROM records
WHERE enabled = 1
GROUP BY status, severity;

CREATE VIEW overview_attention AS
SELECT
  (SELECT count(*) FROM records WHERE enabled = 1 AND status <> 'done') AS open_records,
  (SELECT count(*) FROM records WHERE enabled = 1 AND status <> 'done' AND severity IN ('high', 'critical')) AS urgent_records,
  (SELECT count(*) FROM records WHERE enabled = 0) AS disabled_records,
  (SELECT count(*) FROM records WHERE is_demo = 1) AS demo_records,
  (SELECT count(*) FROM notifications WHERE read_at IS NULL) AS unread_notifications;

PRAGMA user_version = 1;
