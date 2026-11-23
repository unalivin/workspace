package db

import "testing"

func TestOperationsMigrationApplied(t *testing.T) {
 d, err := Open(":memory:")
 if err != nil { t.Fatal(err) }
 defer d.Close()
 var version int
 if err := d.Conn.QueryRow("PRAGMA user_version").Scan(&version); err != nil { t.Fatal(err) }
 if version != 2 { t.Fatalf("schema version = %d, want 2", version) }
 for _, table := range []string{"schedules", "maintenance_windows", "device_tags", "run_events"} {
  var name string
  if err := d.Conn.QueryRow("SELECT name FROM sqlite_master WHERE type='table' AND name=?", table).Scan(&name); err != nil { t.Fatalf("missing table %s: %v", table, err) }
 }
}
