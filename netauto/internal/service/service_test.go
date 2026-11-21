package service

import (
	"testing"

	"github.com/sonix-devs/netauto/internal/db"
)

func TestDryRunApprovalAndDemoSeed(t *testing.T) {
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
	if c.ActiveDevices != 1 || c.PendingApprovals != 1 || c.TotalBackups != 1 {
		t.Fatalf("unexpected overview: %+v", c)
	}

	if err := svc.ApproveRun(1, "admin", "approved in test"); err != nil {
		t.Fatalf("approve run: %v", err)
	}
	c2, err := svc.Overview()
	if err != nil {
		t.Fatalf("overview after approve: %v", err)
	}
	if c2.PendingApprovals != 0 || c2.ExecutedCount != 1 {
		t.Fatalf("unexpected overview after approve: %+v", c2)
	}
}
