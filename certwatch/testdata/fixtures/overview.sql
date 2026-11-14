INSERT INTO certificates (id, fingerprint, display_name, subject, issuer, serial, not_before, not_after, pem, source, is_demo, first_seen_at, last_seen_at) VALUES
  (1, '0101010101010101010101010101010101010101010101010101010101010101', 'expired.fixture.test', 'CN=expired.fixture.test', 'CN=Fixture Root', '01', '2025-12-01T00:00:00Z', '2026-02-20T00:00:00Z', 'fixture', 'observed', 0, '2026-03-01T08:00:00Z', '2026-03-01T08:00:00Z'),
  (2, '0202020202020202020202020202020202020202020202020202020202020202', 'week.fixture.test', 'CN=week.fixture.test', 'CN=Fixture Root', '02', '2025-12-01T00:00:00Z', '2026-03-05T00:00:00Z', 'fixture', 'observed', 0, '2026-03-01T08:00:00Z', '2026-03-01T08:00:00Z'),
  (3, '0303030303030303030303030303030303030303030303030303030303030303', 'month.fixture.test', 'CN=month.fixture.test', 'CN=Fixture Root', '03', '2025-12-01T00:00:00Z', '2026-03-25T00:00:00Z', 'fixture', 'imported', 0, '2026-03-01T08:00:00Z', '2026-03-01T08:00:00Z'),
  (4, '0404040404040404040404040404040404040404040404040404040404040404', 'disabled.fixture.test', 'CN=disabled.fixture.test', 'CN=Fixture Root', '04', '2025-12-01T00:00:00Z', '2026-05-01T00:00:00Z', 'fixture', 'observed', 0, '2026-03-01T08:00:00Z', '2026-03-01T08:00:00Z'),
  (5, '0505050505050505050505050505050505050505050505050505050505050505', 'far.demo.invalid', 'CN=far.demo.invalid', 'CN=Fixture Root', '05', '2025-12-01T00:00:00Z', '2027-01-01T00:00:00Z', 'fixture', 'imported', 1, '2026-03-01T08:00:00Z', '2026-03-01T08:00:00Z'),
  (6, '0606060606060606060606060606060606060606060606060606060606060606', 'quarter.demo.invalid', 'CN=quarter.demo.invalid', 'CN=Fixture Root', '06', '2025-12-01T00:00:00Z', '2026-05-15T00:00:00Z', 'fixture', 'observed', 1, '2026-03-01T08:00:00Z', '2026-03-01T08:00:00Z'),
  (7, '0707070707070707070707070707070707070707070707070707070707070707', 'replaced.fixture.test', 'CN=replaced.fixture.test', 'CN=Fixture Root', '07', '2025-12-01T00:00:00Z', '2026-02-01T00:00:00Z', 'fixture', 'observed', 0, '2026-03-01T08:00:00Z', '2026-03-01T08:00:00Z');

INSERT INTO endpoints (id, host, port, enabled, is_demo, authorized_at, created_at, updated_at) VALUES
  (1, 'a.fixture.test', 443, 1, 0, '2026-03-01T08:00:00Z', '2026-03-01T08:00:00Z', '2026-03-01T08:00:00Z'),
  (2, 'b.fixture.test', 443, 1, 0, '2026-03-01T08:00:00Z', '2026-03-01T08:00:00Z', '2026-03-01T08:00:00Z'),
  (3, 'c.fixture.test', 443, 0, 0, '2026-03-01T08:00:00Z', '2026-03-01T08:00:00Z', '2026-03-01T08:00:00Z'),
  (4, 'd.demo.invalid', 443, 1, 1, '2026-03-01T08:00:00Z', '2026-03-01T08:00:00Z', '2026-03-01T08:00:00Z'),
  (5, 'e.fixture.test', 443, 1, 0, '2026-03-01T08:00:00Z', '2026-03-01T08:00:00Z', '2026-03-01T08:00:00Z'),
  (6, 'f.fixture.test', 443, 1, 0, '2026-03-01T08:00:00Z', '2026-03-01T08:00:00Z', '2026-03-01T08:00:00Z'),
  (7, 'g.fixture.test', 443, 1, 0, '2026-03-01T08:00:00Z', '2026-03-01T08:00:00Z', '2026-03-01T08:00:00Z');

INSERT INTO observations (id, endpoint_id, observed_at, outcome, leaf_certificate_id, chain_status, hostname_status) VALUES
  (1, 1, '2026-03-02T01:00:00Z', 'ok', 7, 'valid', 'match'),
  (2, 1, '2026-03-02T02:00:00Z', 'ok', 1, 'valid', 'match'),
  (3, 2, '2026-03-02T03:00:00Z', 'ok', 2, 'invalid', 'match'),
  (4, 3, '2026-03-02T04:00:00Z', 'ok', 4, 'invalid', 'mismatch'),
  (5, 4, '2026-03-02T05:00:00Z', 'ok', 6, 'invalid', 'mismatch'),
  (6, 5, '2026-03-02T06:00:00Z', 'unreachable', NULL, 'unknown', 'unknown'),
  (7, 7, '2026-03-02T07:00:00Z', 'handshake_failed', NULL, 'unknown', 'unknown');

UPDATE endpoints SET last_observation_id = 2 WHERE id = 1;
UPDATE endpoints SET last_observation_id = 3 WHERE id = 2;
UPDATE endpoints SET last_observation_id = 4 WHERE id = 3;
UPDATE endpoints SET last_observation_id = 5 WHERE id = 4;
UPDATE endpoints SET last_observation_id = 6 WHERE id = 5;
UPDATE endpoints SET last_observation_id = 7 WHERE id = 7;

INSERT INTO findings (observation_id, code, severity, detail) VALUES
  (1, 'expired', 'critical', 'fixture history'),
  (2, 'expired', 'critical', 'fixture'),
  (3, 'chain_untrusted', 'high', 'fixture'),
  (4, 'chain_untrusted', 'high', 'fixture disabled'),
  (4, 'name_mismatch', 'high', 'fixture disabled'),
  (5, 'chain_untrusted', 'high', 'fixture demo'),
  (5, 'name_mismatch', 'high', 'fixture demo');

INSERT INTO notifications (id, event_type, severity, title, record_kind, record_id, dedup_key, created_at, read_at) VALUES
  (1, 'certificate.expiring', 'critical', 'fixture unread one', 'certificate', 1, 'expiry:1:0', '2026-03-01T09:00:00Z', NULL),
  (2, 'validation.failed', 'high', 'fixture unread two', 'endpoint', 2, 'validation:2', '2026-03-01T09:01:00Z', NULL),
  (3, 'certificate.changed', 'info', 'fixture read', 'endpoint', 1, 'changed:1', '2026-03-01T09:02:00Z', '2026-03-01T10:00:00Z');
