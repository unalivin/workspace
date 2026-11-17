CREATE TABLE identity (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  fingerprint_key TEXT NOT NULL CHECK (length(fingerprint_key) = 64 AND fingerprint_key NOT GLOB '*[^0-9a-f]*'),
  created_at TEXT NOT NULL
) STRICT;

CREATE TABLE rules (
  id TEXT PRIMARY KEY CHECK (length(id) BETWEEN 1 AND 60 AND id NOT GLOB '*[^a-z0-9-]*'),
  name TEXT NOT NULL CHECK (length(trim(name)) BETWEEN 1 AND 80),
  description TEXT NOT NULL DEFAULT '' CHECK (length(description) <= 400),
  pattern TEXT NOT NULL CHECK (length(pattern) BETWEEN 1 AND 1000),
  secret_group INTEGER NOT NULL DEFAULT 0 CHECK (secret_group BETWEEN 0 AND 999),
  keywords TEXT NOT NULL DEFAULT '' CHECK (length(keywords) <= 700),
  entropy_tenths INTEGER NOT NULL DEFAULT 0 CHECK (entropy_tenths BETWEEN 0 AND 80),
  severity TEXT NOT NULL DEFAULT 'medium' CHECK (severity IN ('info', 'low', 'medium', 'high', 'critical')),
  confidence INTEGER NOT NULL DEFAULT 60 CHECK (confidence BETWEEN 1 AND 100),
  enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
  revision INTEGER NOT NULL DEFAULT 1 CHECK (revision >= 1),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
) STRICT, WITHOUT ROWID;

CREATE TABLE rule_states (
  rule_id TEXT PRIMARY KEY CHECK (length(rule_id) BETWEEN 1 AND 60),
  enabled INTEGER NOT NULL CHECK (enabled IN (0, 1)),
  updated_at TEXT NOT NULL
) STRICT, WITHOUT ROWID;

CREATE TABLE allowlist_entries (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  kind TEXT NOT NULL CHECK (kind IN ('path', 'value', 'fingerprint')),
  pattern TEXT NOT NULL CHECK (length(pattern) BETWEEN 1 AND 1000),
  rule_id TEXT NOT NULL DEFAULT '' CHECK (length(rule_id) <= 60),
  note TEXT NOT NULL DEFAULT '' CHECK (length(note) <= 400),
  enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
  is_demo INTEGER NOT NULL DEFAULT 0 CHECK (is_demo IN (0, 1)),
  revision INTEGER NOT NULL DEFAULT 1 CHECK (revision >= 1),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  UNIQUE (kind, pattern, rule_id)
) STRICT;

CREATE TABLE scans (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  kind TEXT NOT NULL CHECK (kind IN ('tree', 'history')),
  root TEXT NOT NULL CHECK (length(root) BETWEEN 1 AND 4096),
  status TEXT NOT NULL DEFAULT 'running' CHECK (status IN ('running', 'completed', 'failed', 'cancelled')),
  started_at TEXT NOT NULL,
  finished_at TEXT,
  duration_ms INTEGER NOT NULL DEFAULT 0 CHECK (duration_ms >= 0),
  files_scanned INTEGER NOT NULL DEFAULT 0 CHECK (files_scanned >= 0),
  files_skipped INTEGER NOT NULL DEFAULT 0 CHECK (files_skipped >= 0),
  bytes_scanned INTEGER NOT NULL DEFAULT 0 CHECK (bytes_scanned >= 0),
  commits_scanned INTEGER NOT NULL DEFAULT 0 CHECK (commits_scanned >= 0),
  rules_used INTEGER NOT NULL DEFAULT 0 CHECK (rules_used >= 0),
  findings_seen INTEGER NOT NULL DEFAULT 0 CHECK (findings_seen >= 0),
  findings_new INTEGER NOT NULL DEFAULT 0 CHECK (findings_new >= 0),
  problem TEXT NOT NULL DEFAULT '' CHECK (length(problem) <= 1000),
  is_demo INTEGER NOT NULL DEFAULT 0 CHECK (is_demo IN (0, 1))
) STRICT;

CREATE INDEX scans_started_at ON scans (started_at DESC);
CREATE INDEX scans_root ON scans (root, kind, id DESC);

CREATE TABLE findings (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  root TEXT NOT NULL CHECK (length(root) BETWEEN 1 AND 4096),
  fingerprint TEXT NOT NULL CHECK (length(fingerprint) = 64 AND fingerprint NOT GLOB '*[^0-9a-f]*'),
  value_digest TEXT NOT NULL CHECK (length(value_digest) = 64 AND value_digest NOT GLOB '*[^0-9a-f]*'),
  rule_id TEXT NOT NULL CHECK (length(rule_id) BETWEEN 1 AND 60),
  rule_name TEXT NOT NULL CHECK (length(rule_name) BETWEEN 1 AND 80),
  severity TEXT NOT NULL CHECK (severity IN ('info', 'low', 'medium', 'high', 'critical')),
  path TEXT NOT NULL CHECK (length(path) BETWEEN 1 AND 4096),
  preview TEXT NOT NULL CHECK (length(preview) BETWEEN 8 AND 12),
  value_length INTEGER NOT NULL CHECK (value_length >= 1),
  entropy_tenths INTEGER NOT NULL CHECK (entropy_tenths BETWEEN 0 AND 80),
  score INTEGER NOT NULL CHECK (score BETWEEN 1 AND 100),
  confidence TEXT NOT NULL CHECK (confidence IN ('low', 'medium', 'high')),
  reasons TEXT NOT NULL CHECK (length(reasons) <= 4000),
  status TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'confirmed', 'false_positive', 'accepted_risk', 'resolved')),
  note TEXT NOT NULL DEFAULT '' CHECK (length(note) <= 2000),
  allow_kind TEXT NOT NULL DEFAULT '' CHECK (allow_kind IN ('', 'marker', 'entry')),
  allow_entry_id INTEGER NOT NULL DEFAULT 0 CHECK (allow_entry_id >= 0),
  baseline INTEGER NOT NULL DEFAULT 0 CHECK (baseline IN (0, 1)),
  in_tree INTEGER NOT NULL DEFAULT 0 CHECK (in_tree IN (0, 1)),
  in_history INTEGER NOT NULL DEFAULT 0 CHECK (in_history IN (0, 1)),
  is_demo INTEGER NOT NULL DEFAULT 0 CHECK (is_demo IN (0, 1)),
  revision INTEGER NOT NULL DEFAULT 1 CHECK (revision >= 1),
  last_scan_id INTEGER REFERENCES scans (id) ON DELETE SET NULL,
  first_seen_at TEXT NOT NULL,
  last_seen_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  UNIQUE (root, fingerprint)
) STRICT;

CREATE INDEX findings_status ON findings (status, confidence);
CREATE INDEX findings_rule ON findings (rule_id);
CREATE INDEX findings_path ON findings (path);
CREATE INDEX findings_last_scan ON findings (last_scan_id);
CREATE INDEX findings_value ON findings (value_digest);

CREATE TABLE commits (
  root TEXT NOT NULL CHECK (length(root) BETWEEN 1 AND 4096),
  id TEXT NOT NULL CHECK (length(id) IN (40, 64) AND id NOT GLOB '*[^0-9a-f]*'),
  author TEXT NOT NULL DEFAULT '' CHECK (length(author) <= 200),
  authored_at TEXT NOT NULL,
  subject TEXT NOT NULL DEFAULT '' CHECK (length(subject) <= 200),
  PRIMARY KEY (root, id)
) STRICT, WITHOUT ROWID;

CREATE TABLE occurrences (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  finding_id INTEGER NOT NULL REFERENCES findings (id) ON DELETE CASCADE,
  scan_id INTEGER NOT NULL REFERENCES scans (id) ON DELETE CASCADE,
  line INTEGER NOT NULL CHECK (line >= 1),
  col INTEGER NOT NULL CHECK (col >= 1),
  context TEXT NOT NULL CHECK (length(context) <= 300),
  commit_id TEXT NOT NULL DEFAULT '' CHECK (length(commit_id) IN (0, 40, 64))
) STRICT;

CREATE INDEX occurrences_finding ON occurrences (finding_id, commit_id, line);
CREATE INDEX occurrences_scan ON occurrences (scan_id);
CREATE INDEX occurrences_commit ON occurrences (commit_id) WHERE commit_id <> '';

CREATE VIEW attention_findings AS
SELECT id, root, rule_id, rule_name, severity, path, score, confidence, status, baseline, in_tree, in_history, is_demo
FROM findings
WHERE allow_kind = '' AND status IN ('open', 'confirmed') AND (in_tree = 1 OR in_history = 1);

CREATE VIEW overview_findings_by_rule AS
SELECT rule_id, rule_name, confidence, count(*) AS total
FROM attention_findings
GROUP BY rule_id, rule_name, confidence;

CREATE VIEW overview_findings_by_file AS
SELECT root, path, count(*) AS total, max(score) AS top_score
FROM attention_findings
GROUP BY root, path;

CREATE VIEW overview_scanning AS
SELECT
  (SELECT count(*) FROM attention_findings) AS attention_total,
  (SELECT count(*) FROM attention_findings WHERE baseline = 0) AS new_total,
  (SELECT count(*) FROM attention_findings WHERE baseline = 1) AS baseline_total,
  (SELECT count(*) FROM findings WHERE allow_kind <> '') AS allowlisted_total,
  (SELECT count(*) FROM findings WHERE status IN ('false_positive', 'accepted_risk', 'resolved')) AS decided_total,
  (SELECT count(*) FROM findings WHERE in_tree = 0 AND in_history = 0) AS gone_total,
  (SELECT count(*) FROM findings WHERE is_demo = 1) AS demo_total,
  (SELECT count(*) FROM scans) AS scan_total,
  (SELECT count(*) FROM scans WHERE status = 'failed') AS failed_scan_total,
  (SELECT count(*) FROM notifications WHERE read_at IS NULL) AS unread_notifications;

PRAGMA user_version = 2;
