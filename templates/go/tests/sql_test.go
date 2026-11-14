package tests

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"testing"

	"github.com/NexzaDev/templateapp/internal/notify"
	"github.com/NexzaDev/templateapp/internal/records"
	"github.com/NexzaDev/templateapp/internal/sqlscan"
	"github.com/NexzaDev/templateapp/internal/testkit"
)

const (
	sqlExplainPrefix    = "EXPLAIN QUERY PLAN "
	sqlAttentionView    = "SELECT open_records, urgent_records, disabled_records, demo_records, unread_notifications FROM overview_attention"
	sqlRecordCountsView = "SELECT status, severity, total, demo_total FROM overview_record_counts ORDER BY status, severity"
	sqlSearchFixture    = "SELECT count(*) FROM records_fts WHERE records_fts MATCH ?"
)

var allowedDynamicSQL = map[string][]string{
	"internal/store/store.go": {"query", "migration.SQL", `"PRAGMA "+marker`},
}

func TestNoSQLIsBuiltFromStrings(t *testing.T) {
	findings, err := sqlscan.Scan("..", sqlscan.Options{Allowed: allowedDynamicSQL})
	if err != nil {
		t.Fatal(err)
	}
	for _, finding := range findings {
		t.Errorf("string-built SQL: %s", finding)
	}
}

func explain(query string) string {
	return sqlExplainPrefix + query
}

func fixtureSQL(content []byte) string {
	return string(content)
}

func hotQueries() map[string]string {
	all := map[string]string{}
	for name, query := range records.HotQueries {
		all[name] = query
	}
	for name, query := range notify.HotQueries {
		all[name] = query
	}
	return all
}

func TestHotQueriesUseTheirIndexes(t *testing.T) {
	ctx := context.Background()
	db := testkit.Open(t).DB()
	expected := map[string]string{
		"records by status and severity":     "records_status_severity",
		"recently updated records":           "records_updated_at",
		"record by name":                     "sqlite_autoindex_records_1",
		"record by identifier":               "INTEGER PRIMARY KEY",
		"notification de-duplication lookup": "notifications_dedup",
		"notifications after cursor":         "INTEGER PRIMARY KEY",
		"mark notification read":             "INTEGER PRIMARY KEY",
	}
	queries := hotQueries()
	if len(queries) != len(expected) {
		t.Fatalf("%d hot queries are declared but %d have an expected index", len(queries), len(expected))
	}
	names := make([]string, 0, len(queries))
	for name := range queries {
		names = append(names, name)
	}
	sort.Strings(names)
	for _, name := range names {
		t.Run(name, func(t *testing.T) {
			query := queries[name]
			arguments := make([]any, strings.Count(query, "?"))
			rows, err := db.QueryContext(ctx, explain(query), arguments...)
			if err != nil {
				t.Fatal(err)
			}
			defer rows.Close()
			var plan []string
			for rows.Next() {
				var id, parent, unused int
				var detail string
				if err := rows.Scan(&id, &parent, &unused, &detail); err != nil {
					t.Fatal(err)
				}
				plan = append(plan, detail)
			}
			text := strings.Join(plan, " | ")
			if !strings.Contains(text, expected[name]) {
				t.Fatalf("plan does not use %s: %s", expected[name], text)
			}
			for _, step := range plan {
				if strings.HasPrefix(step, "SCAN") && !strings.Contains(step, "USING") {
					t.Fatalf("plan scans a whole table: %s", text)
				}
				if strings.Contains(step, "TEMP B-TREE") {
					t.Fatalf("plan sorts without an index: %s", text)
				}
			}
		})
	}
}

func TestViewsMatchTheFixture(t *testing.T) {
	ctx := context.Background()
	opened := testkit.Open(t)
	db := opened.DB()
	fixture, err := os.ReadFile(filepath.Join("..", "testdata", "fixtures", "overview.sql"))
	if err != nil {
		t.Fatal(err)
	}
	if _, err := db.ExecContext(ctx, fixtureSQL(fixture)); err != nil {
		t.Fatalf("load fixture: %v", err)
	}
	var open, urgent, disabled, demo, unread int
	if err := db.QueryRowContext(ctx, sqlAttentionView).Scan(&open, &urgent, &disabled, &demo, &unread); err != nil {
		t.Fatal(err)
	}
	if got, want := fmt.Sprint(open, urgent, disabled, demo, unread), "4 3 2 2 2"; got != want {
		t.Fatalf("overview_attention = %s, want %s", got, want)
	}
	rows, err := db.QueryContext(ctx, sqlRecordCountsView)
	if err != nil {
		t.Fatal(err)
	}
	defer rows.Close()
	var got []string
	for rows.Next() {
		var status, severity string
		var total, demoTotal int
		if err := rows.Scan(&status, &severity, &total, &demoTotal); err != nil {
			t.Fatal(err)
		}
		got = append(got, fmt.Sprintf("%s/%s=%d(%d)", status, severity, total, demoTotal))
	}
	want := "done/critical=1(0) in_progress/high=1(0) open/critical=2(1) open/low=1(0)"
	if strings.Join(got, " ") != want {
		t.Fatalf("overview_record_counts = %s, want %s", strings.Join(got, " "), want)
	}
	var matches int
	if err := db.QueryRowContext(ctx, sqlSearchFixture, `"fixture"* "critical"*`).Scan(&matches); err != nil || matches != 4 {
		t.Fatalf("full-text search over the fixture found %d, want 4: %v", matches, err)
	}
	clock := testkit.NewClock()
	overview, err := records.New(db, clock.Now).Overview(ctx)
	if err != nil {
		t.Fatal(err)
	}
	if overview.Open != open || overview.Urgent != urgent || overview.Disabled != disabled || overview.Demo != demo || overview.UnreadNotifications != unread || overview.Total() != 5 {
		t.Fatalf("the service reads different numbers than the views: %+v", overview)
	}
	count, err := notify.New(db, clock.Now).UnreadCount(ctx)
	if err != nil || count != unread {
		t.Fatalf("unread count %d differs from the view %d: %v", count, unread, err)
	}
	if err := opened.IntegrityCheck(ctx); err != nil {
		t.Fatal(err)
	}
	if err := opened.ForeignKeyCheck(ctx); err != nil {
		t.Fatal(err)
	}
}
