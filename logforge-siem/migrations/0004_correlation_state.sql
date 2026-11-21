CREATE TABLE IF NOT EXISTS correlation_state (
    rule_key TEXT NOT NULL,
    correlation_key TEXT NOT NULL,
    pending_events TEXT NOT NULL,
    last_emitted_at TEXT,
    updated_at TEXT NOT NULL,
    PRIMARY KEY (rule_key, correlation_key)
);
CREATE INDEX IF NOT EXISTS idx_correlation_state_updated ON correlation_state(updated_at);
