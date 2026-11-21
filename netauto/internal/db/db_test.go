package db

import "testing"

func TestMigrationsAndIntegrity(t *testing.T) {
	d, err := Open(":memory:")
	if err != nil {
		t.Fatalf("open db: %v", err)
	}
	defer d.Close()

	var integrity string
	if err := d.Conn.QueryRow("PRAGMA integrity_check").Scan(&integrity); err != nil {
		t.Fatalf("integrity_check: %v", err)
	}
	if integrity != "ok" {
		t.Fatalf("expected ok, got %q", integrity)
	}

	var plan string
	if err := d.Conn.QueryRow("EXPLAIN QUERY PLAN SELECT id FROM runs WHERE status = 'pending_approval'").Scan(new(int), new(int), new(int), &plan); err != nil {
		t.Fatalf("explain: %v", err)
	}
	if plan == "" {
		t.Fatalf("expected query plan")
	}
}
