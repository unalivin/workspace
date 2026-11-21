package service

import (
	"testing"

	"github.com/sonix-devs/vulntrack/internal/db"
)

func TestImportDeduplicatesAndScores(t *testing.T) {
	d, err := db.Open(":memory:")
	if err != nil {
		t.Fatalf("open db: %v", err)
	}
	defer d.Close()

	svc := New(d)
	if err := svc.SeedDemo(); err != nil {
		t.Fatalf("seed demo: %v", err)
	}
	if err := svc.SeedDemo(); err != nil {
		t.Fatalf("seed demo idempotent: %v", err)
	}

	c, err := svc.Overview()
	if err != nil {
		t.Fatalf("overview: %v", err)
	}
	if c.OpenVulns != 2 || c.CriticalOpen != 1 || c.TotalAssets != 1 {
		t.Fatalf("unexpected overview: %+v", c)
	}
}
