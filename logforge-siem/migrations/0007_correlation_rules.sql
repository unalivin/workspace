CREATE TABLE IF NOT EXISTS correlation_rules (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    detection_rule_id INTEGER NOT NULL UNIQUE REFERENCES detection_rules(id) ON DELETE CASCADE,
    kind TEXT NOT NULL CHECK (kind IN ('threshold', 'sequence')),
    entity_field TEXT NOT NULL DEFAULT 'host' CHECK (entity_field IN ('host', 'service', 'global')),
    window_seconds INTEGER NOT NULL CHECK (window_seconds BETWEEN 1 AND 2592000),
    threshold INTEGER NOT NULL DEFAULT 1 CHECK (threshold BETWEEN 1 AND 100000),
    first_pattern TEXT NOT NULL,
    second_pattern TEXT NOT NULL DEFAULT '',
    enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
    created_at TEXT NOT NULL
) STRICT;

CREATE INDEX IF NOT EXISTS idx_correlation_rules_enabled ON correlation_rules(enabled, kind);
