CREATE TABLE IF NOT EXISTS settings (
    key TEXT PRIMARY KEY NOT NULL,
    value TEXT NOT NULL,
    updated_at TEXT NOT NULL
) STRICT;

CREATE TABLE IF NOT EXISTS devices (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    adapter TEXT NOT NULL CHECK (adapter IN ('mock', 'lab', 'ssh')),
    host TEXT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
    created_at TEXT NOT NULL
) STRICT;

CREATE TABLE IF NOT EXISTS jobs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    description TEXT NOT NULL,
    command_template TEXT NOT NULL,
    created_at TEXT NOT NULL
) STRICT;

CREATE TABLE IF NOT EXISTS runs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    job_id INTEGER NOT NULL REFERENCES jobs(id) ON DELETE CASCADE,
    device_id INTEGER NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
    mode TEXT NOT NULL CHECK (mode IN ('dry-run', 'execute')),
    status TEXT NOT NULL CHECK (status IN ('pending_approval', 'approved', 'rejected', 'succeeded', 'failed')),
    diff_preview TEXT NOT NULL DEFAULT '',
    output_log TEXT NOT NULL DEFAULT '',
    rollback_guidance TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
) STRICT;

CREATE INDEX IF NOT EXISTS idx_runs_status ON runs(status);
CREATE INDEX IF NOT EXISTS idx_runs_device ON runs(device_id);

CREATE TABLE IF NOT EXISTS approvals (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id INTEGER NOT NULL REFERENCES runs(id) ON DELETE CASCADE,
    decision TEXT NOT NULL CHECK (decision IN ('approved', 'rejected')),
    decided_by TEXT NOT NULL,
    note TEXT NOT NULL DEFAULT '',
    decided_at TEXT NOT NULL
) STRICT;

CREATE TABLE IF NOT EXISTS backups (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    device_id INTEGER NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
    config_text TEXT NOT NULL,
    sha256 TEXT NOT NULL,
    created_at TEXT NOT NULL
) STRICT;

CREATE INDEX IF NOT EXISTS idx_backups_device ON backups(device_id, created_at);

CREATE TABLE IF NOT EXISTS credentials (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    username TEXT NOT NULL,
    encrypted_secret TEXT NOT NULL,
    created_at TEXT NOT NULL
) STRICT;

CREATE TABLE IF NOT EXISTS change_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    actor TEXT NOT NULL,
    action TEXT NOT NULL,
    target TEXT NOT NULL,
    created_at TEXT NOT NULL
) STRICT;

CREATE VIEW IF NOT EXISTS v_overview_counts AS
SELECT
    (SELECT COUNT(*) FROM devices WHERE enabled = 1) AS active_devices,
    (SELECT COUNT(*) FROM runs WHERE status = 'pending_approval') AS pending_approvals,
    (SELECT COUNT(*) FROM runs WHERE mode = 'dry-run') AS dry_run_count,
    (SELECT COUNT(*) FROM runs WHERE mode = 'execute') AS executed_count,
    (SELECT COUNT(*) FROM backups) AS total_backups;
