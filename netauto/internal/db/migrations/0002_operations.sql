CREATE TABLE IF NOT EXISTS schedules (
 id INTEGER PRIMARY KEY AUTOINCREMENT, job_id INTEGER NOT NULL REFERENCES jobs(id) ON DELETE CASCADE,
 device_id INTEGER NOT NULL REFERENCES devices(id) ON DELETE CASCADE, cron TEXT NOT NULL,
 enabled INTEGER NOT NULL DEFAULT 1 CHECK(enabled IN (0,1)), next_run_at TEXT NOT NULL, created_at TEXT NOT NULL
) STRICT;
CREATE TABLE IF NOT EXISTS maintenance_windows (
 id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL, starts_at TEXT NOT NULL, ends_at TEXT NOT NULL,
 created_by TEXT NOT NULL, created_at TEXT NOT NULL, CHECK(ends_at > starts_at)
) STRICT;
CREATE TABLE IF NOT EXISTS device_tags (
 device_id INTEGER NOT NULL REFERENCES devices(id) ON DELETE CASCADE, tag TEXT NOT NULL,
 PRIMARY KEY(device_id,tag)
) STRICT;
CREATE TABLE IF NOT EXISTS run_events (
 id INTEGER PRIMARY KEY AUTOINCREMENT, run_id INTEGER NOT NULL REFERENCES runs(id) ON DELETE CASCADE,
 event TEXT NOT NULL, detail TEXT NOT NULL DEFAULT '', created_at TEXT NOT NULL
) STRICT;
CREATE INDEX IF NOT EXISTS idx_schedules_due ON schedules(enabled,next_run_at);
CREATE INDEX IF NOT EXISTS idx_run_events_run ON run_events(run_id,id);
CREATE INDEX IF NOT EXISTS idx_maintenance_window ON maintenance_windows(starts_at,ends_at);
CREATE INDEX IF NOT EXISTS idx_device_tags_tag ON device_tags(tag,device_id);
