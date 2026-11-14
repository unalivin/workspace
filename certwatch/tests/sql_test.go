package tests

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"testing"

	"github.com/NexzaDev/certwatch/internal/inventory"
	"github.com/NexzaDev/certwatch/internal/notify"
	"github.com/NexzaDev/certwatch/internal/sqlscan"
	"github.com/NexzaDev/certwatch/internal/testkit"
)

const (
	sqlExplainPrefix   = "EXPLAIN QUERY PLAN "
	sqlAttentionView   = "SELECT endpoints, disabled_endpoints, never_checked, failed_checks, chain_failures, hostname_mismatches, certificates, findings, demo_rows, unread_notifications FROM overview_attention"
	sqlTrackedView     = "SELECT group_concat(certificate_id, ' ') FROM (SELECT certificate_id FROM tracked_certificates ORDER BY certificate_id)"
	sqlStatusView      = "SELECT group_concat(endpoint_id || '=' || outcome, ' ') FROM (SELECT endpoint_id, outcome FROM endpoint_status ORDER BY endpoint_id)"
	sqlLatestFindings  = "SELECT group_concat(observation_id || ':' || code, ' ') FROM (SELECT observation_id, code FROM latest_findings ORDER BY observation_id, code)"
	sqlUsedByView      = "SELECT endpoint_count, used_by FROM tracked_certificates WHERE certificate_id = ?"
	sqlFixtureOrphans  = "SELECT count(*) FROM observations WHERE endpoint_id NOT IN (SELECT id FROM endpoints)"
	sqlFixtureEndpoint = "DELETE FROM endpoints WHERE id = ?"
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
	for name, query := range inventory.HotQueries {
		all[name] = query
	}
	for name, query := range notify.HotQueries {
		all[name] = query
	}
	return all
}

type planRule struct {
	index string
	sorts bool
}

var inventoryTables = map[string]bool{"e": true, "c": true, "endpoints": true, "certificates": true, "CONSTANT ROW": true, "overview_attention": true}

func TestHotQueriesUseTheirIndexes(t *testing.T) {
	ctx := context.Background()
	db := testkit.Open(t).DB()
	expected := map[string]planRule{
		"endpoint list":                      {index: "SEARCH o USING INTEGER PRIMARY KEY", sorts: true},
		"endpoints due for a check":          {index: "endpoints_last_observation", sorts: true},
		"certificate list":                   {index: "certificates_not_after"},
		"expiry buckets":                     {index: "SEARCH o USING INTEGER PRIMARY KEY"},
		"overview attention":                 {index: "notifications_unread"},
		"endpoint history":                   {index: "observations_endpoint"},
		"latest findings":                    {index: "findings_observation", sorts: true},
		"certificate change history":         {index: "observations_previous", sorts: true},
		"previous leaf of endpoint":          {index: "observations_endpoint"},
		"prune observation history":          {index: "observations_endpoint"},
		"expiry threshold candidates":        {index: "certificates_not_after (not_after<?)"},
		"notification de-duplication lookup": {index: "notifications_dedup"},
		"notifications after cursor":         {index: "INTEGER PRIMARY KEY"},
		"mark notification read":             {index: "INTEGER PRIMARY KEY"},
	}
	queries := hotQueries()
	if len(queries) != len(expected) {
		t.Fatalf("%d hot queries are declared but %d have an expected plan", len(queries), len(expected))
	}
	names := make([]string, 0, len(queries))
	for name := range queries {
		names = append(names, name)
	}
	sort.Strings(names)
	for _, name := range names {
		t.Run(name, func(t *testing.T) {
			query := queries[name]
			rule, known := expected[name]
			if !known {
				t.Fatalf("no expected plan for %q", name)
			}
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
			if !strings.Contains(text, rule.index) {
				t.Fatalf("plan does not use %s: %s", rule.index, text)
			}
			for _, step := range plan {
				if strings.HasPrefix(step, "SCAN ") && !strings.Contains(step, "USING") && !inventoryTables[strings.TrimPrefix(step, "SCAN ")] {
					t.Fatalf("plan scans a table that grows with the check history (%s): %s", step, text)
				}
				if strings.Contains(step, "TEMP B-TREE") && !rule.sorts {
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
	counts := make([]int, 10)
	targets := make([]any, len(counts))
	for i := range counts {
		targets[i] = &counts[i]
	}
	if err := db.QueryRowContext(ctx, sqlAttentionView).Scan(targets...); err != nil {
		t.Fatal(err)
	}
	if got, want := fmt.Sprint(counts), "[7 1 1 2 2 1 5 4 3 2]"; got != want {
		t.Fatalf("overview_attention = %s, want %s (endpoints, disabled, never checked, failed checks, chain failures, name mismatches, certificates, findings, demo rows, unread)", got, want)
	}
	for query, want := range map[string]string{
		sqlTrackedView:    "1 2 3 5 6",
		sqlStatusView:     "1=ok 2=ok 3=ok 4=ok 5=unreachable 6=never 7=handshake_failed",
		sqlLatestFindings: "2:expired 3:chain_untrusted 5:chain_untrusted 5:name_mismatch",
	} {
		var got string
		if err := db.QueryRowContext(ctx, query).Scan(&got); err != nil || got != want {
			t.Fatalf("%s = %q, want %q: %v", query, got, want, err)
		}
	}
	var endpoints int
	var usedBy string
	if err := db.QueryRowContext(ctx, sqlUsedByView, 2).Scan(&endpoints, &usedBy); err != nil || endpoints != 1 || usedBy != "b.fixture.test:443" {
		t.Fatalf("tracked_certificates for certificate 2: %d %q %v", endpoints, usedBy, err)
	}
	clock := testkit.NewClock()
	overview, err := inventory.New(db, clock.Now).Overview(ctx)
	if err != nil {
		t.Fatal(err)
	}
	want := inventory.Overview{Expired: 1, Within7: 1, Within30: 2, Within90: 3, Certificates: 5, Endpoints: 7, DisabledEndpoints: 1, NeverChecked: 1, FailedChecks: 2, ChainFailures: 2, HostnameMismatches: 1, Findings: 4, DemoRows: 3, UnreadAlerts: 2}
	if overview != want {
		t.Fatalf("the service reads different numbers than the views:\n got %+v\nwant %+v", overview, want)
	}
	count, err := notify.New(db, clock.Now).UnreadCount(ctx)
	if err != nil || count != counts[9] {
		t.Fatalf("unread count %d differs from the view %d: %v", count, counts[9], err)
	}
	if _, err := db.ExecContext(ctx, sqlFixtureEndpoint, 1); err != nil {
		t.Fatal(err)
	}
	var orphans int
	if err := db.QueryRowContext(ctx, sqlFixtureOrphans).Scan(&orphans); err != nil || orphans != 0 {
		t.Fatalf("removing an endpoint left %d observations behind: %v", orphans, err)
	}
	if err := opened.IntegrityCheck(ctx); err != nil {
		t.Fatal(err)
	}
	if err := opened.ForeignKeyCheck(ctx); err != nil {
		t.Fatal(err)
	}
}
