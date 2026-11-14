package testkit

import (
	"context"
	"path/filepath"
	"sync"
	"testing"
	"time"

	"github.com/NexzaDev/certwatch/internal/store"
	"github.com/NexzaDev/certwatch/migrations"
)

type Clock struct {
	mu  sync.Mutex
	now time.Time
}

func NewClock() *Clock {
	return &Clock{now: time.Date(2026, time.March, 2, 9, 30, 0, 0, time.UTC)}
}

func (c *Clock) Now() time.Time {
	c.mu.Lock()
	defer c.mu.Unlock()
	return c.now
}

func (c *Clock) Advance(d time.Duration) {
	c.mu.Lock()
	defer c.mu.Unlock()
	c.now = c.now.Add(d)
}

func (c *Clock) Set(t time.Time) {
	c.mu.Lock()
	defer c.mu.Unlock()
	c.now = t
}

func DatabasePath(t testing.TB) string {
	t.Helper()
	return filepath.Join(t.TempDir(), "test.db")
}

func Migrations(t testing.TB) []store.Migration {
	t.Helper()
	loaded, err := store.LoadMigrations(migrations.Files)
	if err != nil {
		t.Fatalf("load migrations: %v", err)
	}
	return loaded
}

func OpenAt(t testing.TB, path string) *store.Store {
	t.Helper()
	ctx := context.Background()
	opened, err := store.Open(ctx, path, store.Options{BusyTimeout: 2 * time.Second})
	if err != nil {
		t.Fatalf("open database: %v", err)
	}
	t.Cleanup(func() { _ = opened.Close() })
	if _, err := opened.Migrate(ctx, Migrations(t)); err != nil {
		t.Fatalf("migrate: %v", err)
	}
	return opened
}

func Open(t testing.TB) *store.Store {
	t.Helper()
	return OpenAt(t, DatabasePath(t))
}
