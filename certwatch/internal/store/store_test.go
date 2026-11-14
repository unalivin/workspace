package store_test

import (
	"context"
	"errors"
	"flag"
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"sync"
	"testing"
	"testing/fstest"
	"time"

	"github.com/NexzaDev/certwatch/internal/store"
	"github.com/NexzaDev/certwatch/internal/testkit"
	"github.com/NexzaDev/certwatch/migrations"
)

var updateSnapshot = flag.Bool("update", false, "rewrite the schema snapshot")

func TestOpenAppliesRequiredPragmas(t *testing.T) {
	ctx := context.Background()
	opened := testkit.Open(t)
	mode, err := opened.JournalMode(ctx)
	if err != nil || mode != "wal" {
		t.Fatalf("journal mode = %q, %v; want wal", mode, err)
	}
	enabled, err := opened.ForeignKeysEnabled(ctx)
	if err != nil || !enabled {
		t.Fatalf("foreign keys enabled = %v, %v", enabled, err)
	}
	timeout, err := opened.BusyTimeout(ctx)
	if err != nil || timeout != 2*time.Second {
		t.Fatalf("busy timeout = %v, %v", timeout, err)
	}
	fts, err := opened.HasFullTextSearch(ctx)
	if err != nil || !fts {
		t.Fatalf("FTS5 available = %v, %v; build and test with -tags sqlite_fts5", fts, err)
	}
}

func TestPragmasHoldOnEveryPooledConnection(t *testing.T) {
	ctx := context.Background()
	opened := testkit.Open(t)
	var wg sync.WaitGroup
	failures := make(chan string, 16)
	for worker := 0; worker < 8; worker++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			conn, err := opened.DB().Conn(ctx)
			if err != nil {
				failures <- err.Error()
				return
			}
			defer conn.Close()
			var enabled int
			if err := conn.QueryRowContext(ctx, "PRAGMA foreign_keys").Scan(&enabled); err != nil || enabled != 1 {
				failures <- "foreign keys are off on a pooled connection"
			}
			time.Sleep(10 * time.Millisecond)
		}()
	}
	wg.Wait()
	close(failures)
	for failure := range failures {
		t.Fatal(failure)
	}
}

func TestDatabaseFileIsCreatedWithRestrictivePermissions(t *testing.T) {
	path := filepath.Join(t.TempDir(), "nested", "dir", "app.db")
	testkit.OpenAt(t, path)
	if runtime.GOOS == "windows" {
		return
	}
	info, err := os.Stat(path)
	if err != nil {
		t.Fatal(err)
	}
	if info.Mode().Perm() != 0o600 {
		t.Fatalf("database mode = %v, want 0600", info.Mode().Perm())
	}
	parent, err := os.Stat(filepath.Dir(path))
	if err != nil || parent.Mode().Perm() != 0o700 {
		t.Fatalf("directory mode = %v, %v; want 0700", parent.Mode().Perm(), err)
	}
}

func TestOpenRejectsUnusablePaths(t *testing.T) {
	ctx := context.Background()
	if _, err := store.Open(ctx, "relative.db", store.Options{}); err == nil {
		t.Fatal("expected an error for a relative path")
	}
	if _, err := store.Open(ctx, t.TempDir(), store.Options{}); err == nil || !strings.Contains(err.Error(), "directory") {
		t.Fatalf("expected a directory error, got %v", err)
	}
	if _, err := store.Open(ctx, filepath.Join(t.TempDir(), "missing.db"), store.Options{ReadOnly: true}); err == nil {
		t.Fatal("expected an error for a missing read-only database")
	}
	garbage := filepath.Join(t.TempDir(), "garbage.db")
	if err := os.WriteFile(garbage, []byte(strings.Repeat("not a database ", 100)), 0o600); err != nil {
		t.Fatal(err)
	}
	if _, err := store.Open(ctx, garbage, store.Options{}); err == nil || !strings.Contains(err.Error(), "not a usable SQLite database") {
		t.Fatalf("expected a corrupt-file error, got %v", err)
	}
}

func TestOpenLeavesForeignFilesUntouched(t *testing.T) {
	ctx := context.Background()
	for name, content := range map[string]string{"one byte": "x", "short text": "notes", "almost a header": "SQLite format 3", "text of header length": "0123456789abcdef"} {
		path := filepath.Join(t.TempDir(), "foreign.db")
		if err := os.WriteFile(path, []byte(content), 0o600); err != nil {
			t.Fatal(err)
		}
		for _, readOnly := range []bool{false, true} {
			if _, err := store.Open(ctx, path, store.Options{ReadOnly: readOnly}); err == nil || !strings.Contains(err.Error(), "left untouched") {
				t.Fatalf("%s (read-only %v): got %v", name, readOnly, err)
			}
		}
		after, err := os.ReadFile(path)
		if err != nil || string(after) != content {
			t.Fatalf("%s: the file was changed to %q", name, after)
		}
	}
	empty := filepath.Join(t.TempDir(), "empty.db")
	if err := os.WriteFile(empty, nil, 0o600); err != nil {
		t.Fatal(err)
	}
	opened, err := store.Open(ctx, empty, store.Options{})
	if err != nil {
		t.Fatalf("an empty file must be usable as a new database: %v", err)
	}
	_ = opened.Close()
}

func TestPathsWithSpecialCharacters(t *testing.T) {
	path := filepath.Join(t.TempDir(), "dir with space", "50% #1 app.db")
	opened := testkit.OpenAt(t, path)
	if opened.Path() != path {
		t.Fatalf("path = %q, want %q", opened.Path(), path)
	}
	if _, err := os.Stat(path); err != nil {
		t.Fatalf("database was not created at the requested path: %v", err)
	}
}

func TestMigrationsApplyOnceAndTrackVersion(t *testing.T) {
	ctx := context.Background()
	path := testkit.DatabasePath(t)
	opened, err := store.Open(ctx, path, store.Options{})
	if err != nil {
		t.Fatal(err)
	}
	defer opened.Close()
	all := testkit.Migrations(t)
	applied, err := opened.Migrate(ctx, all)
	if err != nil || applied != len(all) {
		t.Fatalf("first run applied %d, %v; want %d", applied, err, len(all))
	}
	applied, err = opened.Migrate(ctx, all)
	if err != nil || applied != 0 {
		t.Fatalf("second run applied %d, %v; want 0", applied, err)
	}
	version, err := opened.SchemaVersion(ctx)
	if err != nil || version != len(all) {
		t.Fatalf("schema version = %d, %v; want %d", version, err, len(all))
	}
	if err := opened.IntegrityCheck(ctx); err != nil {
		t.Fatal(err)
	}
	if err := opened.ForeignKeyCheck(ctx); err != nil {
		t.Fatal(err)
	}
}

func TestFailedMigrationRollsBackCompletely(t *testing.T) {
	ctx := context.Background()
	opened, err := store.Open(ctx, testkit.DatabasePath(t), store.Options{})
	if err != nil {
		t.Fatal(err)
	}
	defer opened.Close()
	broken := []store.Migration{
		{Number: 1, Name: "0001_ok.sql", SQL: "CREATE TABLE a (id INTEGER PRIMARY KEY) STRICT; PRAGMA user_version = 1;"},
		{Number: 2, Name: "0002_broken.sql", SQL: "CREATE TABLE b (id INTEGER PRIMARY KEY) STRICT; INSERT INTO missing VALUES (1); PRAGMA user_version = 2;"},
	}
	applied, err := opened.Migrate(ctx, broken)
	if err == nil || applied != 1 || !strings.Contains(err.Error(), "0002_broken.sql") {
		t.Fatalf("applied %d, err %v; want 1 and an error naming the migration", applied, err)
	}
	version, _ := opened.SchemaVersion(ctx)
	if version != 1 {
		t.Fatalf("schema version = %d, want 1", version)
	}
	var count int
	if err := opened.DB().QueryRowContext(ctx, "SELECT count(*) FROM sqlite_master WHERE name = 'b'").Scan(&count); err != nil || count != 0 {
		t.Fatalf("table from the failed migration exists (%d, %v)", count, err)
	}
}

func TestMigrateRefusesNewerDatabaseAndReadOnly(t *testing.T) {
	ctx := context.Background()
	path := testkit.DatabasePath(t)
	opened := testkit.OpenAt(t, path)
	if _, err := opened.DB().ExecContext(ctx, "PRAGMA user_version = 999"); err != nil {
		t.Fatal(err)
	}
	if _, err := opened.Migrate(ctx, testkit.Migrations(t)); err == nil || !strings.Contains(err.Error(), "newer build") {
		t.Fatalf("expected a newer-build error, got %v", err)
	}
	readOnly, err := store.Open(ctx, path, store.Options{ReadOnly: true})
	if err != nil {
		t.Fatal(err)
	}
	defer readOnly.Close()
	if _, err := readOnly.Migrate(ctx, testkit.Migrations(t)); err == nil {
		t.Fatal("expected read-only migration to fail")
	}
}

func TestReadOnlyStoreCannotWrite(t *testing.T) {
	ctx := context.Background()
	path := testkit.DatabasePath(t)
	testkit.OpenAt(t, path)
	readOnly, err := store.Open(ctx, path, store.Options{ReadOnly: true})
	if err != nil {
		t.Fatal(err)
	}
	defer readOnly.Close()
	if !readOnly.ReadOnly() {
		t.Fatal("store does not report read-only")
	}
	_, err = readOnly.DB().ExecContext(ctx, "INSERT INTO settings (key, value, updated_at) VALUES ('a', 'b', 'c')")
	if err == nil {
		t.Fatal("write succeeded on a read-only store")
	}
	var count int
	if err := readOnly.DB().QueryRowContext(ctx, "SELECT count(*) FROM settings").Scan(&count); err != nil || count != 0 {
		t.Fatalf("read failed or data changed: %d, %v", count, err)
	}
}

func TestConcurrentMigrationFromTwoConnections(t *testing.T) {
	ctx := context.Background()
	path := testkit.DatabasePath(t)
	all := testkit.Migrations(t)
	var wg sync.WaitGroup
	errs := make(chan error, 4)
	for worker := 0; worker < 4; worker++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			opened, err := store.Open(ctx, path, store.Options{})
			if err != nil {
				errs <- err
				return
			}
			defer opened.Close()
			_, err = opened.Migrate(ctx, all)
			errs <- err
		}()
	}
	wg.Wait()
	close(errs)
	for err := range errs {
		if err != nil {
			t.Fatalf("concurrent migration failed: %v", err)
		}
	}
	opened := testkit.OpenAt(t, path)
	if err := opened.IntegrityCheck(ctx); err != nil {
		t.Fatal(err)
	}
}

func TestDataVersionChangesOnlyForOtherConnections(t *testing.T) {
	ctx := context.Background()
	path := testkit.DatabasePath(t)
	first := testkit.OpenAt(t, path)
	second := testkit.OpenAt(t, path)
	before, err := first.DataVersion(ctx)
	if err != nil {
		t.Fatal(err)
	}
	again, _ := first.DataVersion(ctx)
	if again != before {
		t.Fatalf("data version changed without a write: %d -> %d", before, again)
	}
	if _, err := second.DB().ExecContext(ctx, "INSERT INTO settings (key, value, updated_at) VALUES ('k', 'v', 'now')"); err != nil {
		t.Fatal(err)
	}
	after, err := first.DataVersion(ctx)
	if err != nil || after == before {
		t.Fatalf("data version did not change after an external write: %d -> %d (%v)", before, after, err)
	}
}

func TestSchemaSnapshotMatchesGoldenFile(t *testing.T) {
	ctx := context.Background()
	opened := testkit.Open(t)
	snapshot, err := opened.SchemaSnapshot(ctx)
	if err != nil {
		t.Fatal(err)
	}
	golden := filepath.Join("..", "..", "testdata", "schema.snapshot")
	if *updateSnapshot {
		if err := os.WriteFile(golden, []byte(snapshot), 0o644); err != nil {
			t.Fatal(err)
		}
	}
	want, err := os.ReadFile(golden)
	if err != nil {
		t.Fatalf("read golden snapshot (create it with -update): %v", err)
	}
	if strings.ReplaceAll(string(want), "\r\n", "\n") != snapshot {
		t.Fatalf("schema differs from testdata/schema.snapshot; add a new migration and run the test with -update\n%s", snapshot)
	}
}

func TestLoadMigrationsValidatesFiles(t *testing.T) {
	valid := "CREATE TABLE a (id INTEGER PRIMARY KEY) STRICT;\nPRAGMA user_version = 1;\n"
	cases := []struct {
		name  string
		files fstest.MapFS
		want  string
	}{
		{"empty", fstest.MapFS{}, "no migrations"},
		{"bad name", fstest.MapFS{"init.sql": {Data: []byte(valid)}}, "NNNN_name.sql"},
		{"gap", fstest.MapFS{"0001_a.sql": {Data: []byte(valid)}, "0003_c.sql": {Data: []byte("SELECT 1;\nPRAGMA user_version = 3;")}}, "without gaps"},
		{"missing marker", fstest.MapFS{"0001_a.sql": {Data: []byte("CREATE TABLE a (id INTEGER) STRICT;")}}, "must end with"},
		{"wrong marker", fstest.MapFS{"0001_a.sql": {Data: []byte("SELECT 1;\nPRAGMA user_version = 2;")}}, "must end with"},
		{"starts at two", fstest.MapFS{"0002_a.sql": {Data: []byte("SELECT 1;\nPRAGMA user_version = 2;")}}, "without gaps"},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			_, err := store.LoadMigrations(tc.files)
			if err == nil || !strings.Contains(err.Error(), tc.want) {
				t.Fatalf("error = %v, want it to contain %q", err, tc.want)
			}
		})
	}
	loaded, err := store.LoadMigrations(fstest.MapFS{"0001_a.sql": {Data: []byte(valid)}, "embed.go": {Data: []byte("package x")}})
	if err != nil || len(loaded) != 1 || loaded[0].Number != 1 {
		t.Fatalf("valid set rejected: %v %v", loaded, err)
	}
}

func TestEmbeddedMigrationsAreValid(t *testing.T) {
	loaded, err := store.LoadMigrations(migrations.Files)
	if err != nil || len(loaded) == 0 {
		t.Fatalf("embedded migrations invalid: %v", err)
	}
}

func TestRetryOnlyRepeatsBusyErrors(t *testing.T) {
	ctx := context.Background()
	calls := 0
	plain := errors.New("plain failure")
	err := store.Retry(ctx, 3, time.Millisecond, func() error { calls++; return plain })
	if !errors.Is(err, plain) || calls != 1 {
		t.Fatalf("non-busy error retried: calls=%d err=%v", calls, err)
	}
	calls = 0
	err = store.Retry(ctx, 3, time.Millisecond, func() error { calls++; return nil })
	if err != nil || calls != 1 {
		t.Fatalf("success repeated: calls=%d err=%v", calls, err)
	}
}

func TestBusyDatabaseIsReportedAndRetried(t *testing.T) {
	ctx := context.Background()
	path := testkit.DatabasePath(t)
	testkit.OpenAt(t, path)
	holder, err := store.Open(ctx, path, store.Options{BusyTimeout: 20 * time.Millisecond})
	if err != nil {
		t.Fatal(err)
	}
	defer holder.Close()
	writer, err := store.Open(ctx, path, store.Options{BusyTimeout: 20 * time.Millisecond})
	if err != nil {
		t.Fatal(err)
	}
	defer writer.Close()
	tx, err := holder.DB().BeginTx(ctx, nil)
	if err != nil {
		t.Fatal(err)
	}
	insert := func() error {
		_, err := writer.DB().ExecContext(ctx, "INSERT INTO settings (key, value, updated_at) VALUES ('busy', 'v', 'now')")
		return err
	}
	if err := insert(); !store.IsBusy(err) {
		t.Fatalf("expected a busy error while another writer holds the lock, got %v", err)
	}
	if err := store.Retry(ctx, 2, time.Millisecond, insert); err == nil || !strings.Contains(err.Error(), "stayed busy") {
		t.Fatalf("expected the retry to give up, got %v", err)
	}
	go func() {
		time.Sleep(60 * time.Millisecond)
		_ = tx.Rollback()
	}()
	if err := store.Retry(ctx, 50, 10*time.Millisecond, insert); err != nil {
		t.Fatalf("retry did not succeed after the lock was released: %v", err)
	}
	if err := insert(); !store.IsConstraint(err) {
		t.Fatalf("expected a constraint error for the duplicate key, got %v", err)
	}
}
