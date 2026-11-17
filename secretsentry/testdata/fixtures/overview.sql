INSERT INTO records (id, name, status, severity, note, enabled, is_demo, created_at, updated_at) VALUES
  (1, 'fixture open critical', 'open', 'critical', '', 1, 0, '2026-03-01T08:00:00Z', '2026-03-01T08:00:00Z'),
  (2, 'fixture open critical demo', 'open', 'critical', '', 1, 1, '2026-03-01T08:01:00Z', '2026-03-01T08:01:00Z'),
  (3, 'fixture open low', 'open', 'low', '', 1, 0, '2026-03-01T08:02:00Z', '2026-03-01T08:02:00Z'),
  (4, 'fixture progress high', 'in_progress', 'high', '', 1, 0, '2026-03-01T08:03:00Z', '2026-03-01T08:03:00Z'),
  (5, 'fixture done critical', 'done', 'critical', '', 1, 0, '2026-03-01T08:04:00Z', '2026-03-01T08:04:00Z'),
  (6, 'fixture disabled critical', 'open', 'critical', '', 0, 0, '2026-03-01T08:05:00Z', '2026-03-01T08:05:00Z'),
  (7, 'fixture disabled demo', 'done', 'info', '', 0, 1, '2026-03-01T08:06:00Z', '2026-03-01T08:06:00Z');

INSERT INTO notifications (id, event_type, severity, title, record_kind, record_id, dedup_key, created_at, read_at) VALUES
  (1, 'record.urgent', 'critical', 'fixture unread one', 'record', 1, 'record:1', '2026-03-01T09:00:00Z', NULL),
  (2, 'record.urgent', 'high', 'fixture unread two', 'record', 4, 'record:4', '2026-03-01T09:01:00Z', NULL),
  (3, 'record.completed', 'info', 'fixture read', 'record', 5, 'record:5', '2026-03-01T09:02:00Z', '2026-03-01T10:00:00Z');
