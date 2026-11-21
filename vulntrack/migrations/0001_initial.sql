CREATE TABLE IF NOT EXISTS settings (
    key TEXT PRIMARY KEY NOT NULL,
    value TEXT NOT NULL,
    updated_at TEXT NOT NULL
) STRICT;

CREATE TABLE IF NOT EXISTS assets (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    kind TEXT NOT NULL,
    owner TEXT NOT NULL DEFAULT 'unassigned',
    created_at TEXT NOT NULL
) STRICT;

CREATE TABLE IF NOT EXISTS vulnerabilities (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    asset_id INTEGER NOT NULL REFERENCES assets(id) ON DELETE CASCADE,
    cve_id TEXT NOT NULL,
    title TEXT NOT NULL,
    severity TEXT NOT NULL CHECK (severity IN ('low', 'medium', 'high', 'critical')),
    exploitability TEXT NOT NULL DEFAULT 'unknown' CHECK (exploitability IN ('unknown', 'poc', 'active')),
    risk_score REAL NOT NULL,
    status TEXT NOT NULL DEFAULT 'reported' CHECK (status IN ('reported', 'confirmed', 'in_remediation', 'retested', 'resolved', 'accepted')),
    owner TEXT NOT NULL DEFAULT 'unassigned',
    sla_due_at TEXT NOT NULL,
    dedup_key TEXT NOT NULL UNIQUE,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
) STRICT;

CREATE INDEX IF NOT EXISTS idx_vulns_severity_status ON vulnerabilities(severity, status);
CREATE INDEX IF NOT EXISTS idx_vulns_asset ON vulnerabilities(asset_id);

CREATE TABLE IF NOT EXISTS exceptions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    vuln_id INTEGER NOT NULL REFERENCES vulnerabilities(id) ON DELETE CASCADE,
    reason TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    created_at TEXT NOT NULL
) STRICT;

CREATE VIEW IF NOT EXISTS v_overview_counts AS
SELECT
    (SELECT COUNT(*) FROM vulnerabilities WHERE status NOT IN ('resolved', 'accepted')) AS open_vulns,
    (SELECT COUNT(*) FROM vulnerabilities WHERE severity = 'critical' AND status NOT IN ('resolved', 'accepted')) AS critical_open,
    (SELECT COUNT(*) FROM assets) AS total_assets,
    (SELECT COUNT(*) FROM exceptions) AS active_exceptions;
