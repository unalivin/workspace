CREATE TABLE IF NOT EXISTS malformed_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    source_id INTEGER NOT NULL REFERENCES sources(id) ON DELETE CASCADE,
    received_at TEXT NOT NULL,
    raw_line TEXT NOT NULL,
    reason TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_malformed_source_received ON malformed_events(source_id, received_at DESC);
