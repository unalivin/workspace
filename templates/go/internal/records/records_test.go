package records

import (
	"bytes"
	"context"
	"errors"
	"strings"
	"sync"
	"testing"
	"time"

	"github.com/NexzaDev/templateapp/internal/notify"
	"github.com/NexzaDev/templateapp/internal/testkit"
	"github.com/NexzaDev/templateapp/internal/theme"
)

type env struct {
	records *Service
	notices *notify.Service
	clock   *testkit.Clock
}

func newEnv(t *testing.T) env {
	t.Helper()
	clock := testkit.NewClock()
	db := testkit.Open(t).DB()
	return env{records: New(db, clock.Now), notices: notify.New(db, clock.Now), clock: clock}
}

func mustCreate(t *testing.T, e env, in Input) Record {
	t.Helper()
	created, err := e.records.Create(context.Background(), in)
	if err != nil {
		t.Fatalf("create %q: %v", in.Name, err)
	}
	return created
}

func TestNormalize(t *testing.T) {
	good, err := Normalize(Input{Name: "  core \t switch  ", Note: " line one\r\nline two ", Severity: "HIGH", Status: " Done "})
	if err != nil {
		t.Fatal(err)
	}
	if good.Name != "core switch" || good.Note != "line one\nline two" || good.Severity != theme.SeverityHigh || good.Status != StatusDone {
		t.Fatalf("unexpected result %+v", good)
	}
	defaults, err := Normalize(Input{Name: "x"})
	if err != nil || defaults.Status != StatusOpen || defaults.Severity != theme.SeverityInfo {
		t.Fatalf("defaults: %+v, %v", defaults, err)
	}
	cases := []struct {
		name  string
		in    Input
		field string
	}{
		{"empty name", Input{Name: "   "}, "name"},
		{"control character in name", Input{Name: "a\x1bb"}, "name"},
		{"long name", Input{Name: strings.Repeat("n", MaxNameLength+1)}, "name"},
		{"invalid UTF-8", Input{Name: "a\xffb"}, "name"},
		{"long note", Input{Name: "a", Note: strings.Repeat("n", MaxNoteLength+1)}, "note"},
		{"control character in note", Input{Name: "a", Note: "x\x00y"}, "note"},
		{"unknown status", Input{Name: "a", Status: "closed"}, "status"},
		{"unknown severity", Input{Name: "a", Severity: "urgent"}, "severity"},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			_, err := Normalize(tc.in)
			var invalid *ValidationError
			if !errors.As(err, &invalid) || invalid.Field != tc.field || invalid.Fix == "" {
				t.Fatalf("got %v, want a validation error on %s with a fix", err, tc.field)
			}
		})
	}
	if _, err := Normalize(Input{Name: strings.Repeat("é", MaxNameLength)}); err != nil {
		t.Fatalf("a name of exactly the maximum length in characters was rejected: %v", err)
	}
}

func TestSearchExpression(t *testing.T) {
	cases := map[string]string{
		"":                        "",
		"router":                  `"router"*`,
		"edge  router":            `"edge"* "router"*`,
		`"quoted" AND NOT x*`:     `"quoted"* "AND"* "NOT"* "x"*`,
		"name:secret OR note:^a":  `"name"* "secret"* "OR"* "note"* "a"*`,
		"');DROP TABLE records--": `"DROP"* "TABLE"* "records"*`,
		"!!! ***":                 "",
		"zürich-büro":             `"zürich"* "büro"*`,
	}
	for in, want := range cases {
		if got := SearchExpression(in); got != want {
			t.Errorf("SearchExpression(%q) = %q, want %q", in, got, want)
		}
	}
	if got := SearchExpression(strings.Repeat("word ", 100)); strings.Count(got, "*") != 16 {
		t.Fatalf("term count is not bounded: %d", strings.Count(got, "*"))
	}
	if got := SearchExpression(strings.Repeat("x", 1000)); len(got) > 80 {
		t.Fatalf("term length is not bounded: %d", len(got))
	}
}

func FuzzSearchExpressionIsAlwaysAValidQuery(f *testing.F) {
	for _, seed := range []string{"router", `"`, "a OR b", "NEAR(a b)", "x* -y", "\x00", "col:val", "a\"b\"c", "(((", "^start"} {
		f.Add(seed)
	}
	clock := testkit.NewClock()
	service := New(testkit.Open(f).DB(), clock.Now)
	if _, err := service.Create(context.Background(), Input{Name: "edge router", Note: "firmware"}); err != nil {
		f.Fatal(err)
	}
	f.Fuzz(func(t *testing.T, text string) {
		if _, err := service.List(context.Background(), Query{Search: text}); err != nil {
			t.Fatalf("search for %q failed: %v", text, err)
		}
	})
}

func TestCreateGetUpdateDelete(t *testing.T) {
	ctx := context.Background()
	e := newEnv(t)
	created := mustCreate(t, e, Input{Name: "Core switch", Severity: theme.SeverityLow, Note: "rack 4", Enabled: true})
	if created.ID == 0 || created.Revision != 1 || created.Status != StatusOpen || created.Demo || !created.CreatedAt.Equal(e.clock.Now()) {
		t.Fatalf("unexpected record %+v", created)
	}
	if _, err := e.records.Create(ctx, Input{Name: "Core switch"}); err == nil || !strings.Contains(err.Error(), "already exists") {
		t.Fatalf("duplicate name: %v", err)
	}
	if _, err := e.records.Create(ctx, Input{Name: ""}); err == nil {
		t.Fatal("an empty name was stored")
	}
	e.clock.Advance(time.Hour)
	change := created.Input()
	change.Note = "rack 5"
	updated, err := e.records.Update(ctx, created.ID, created.Revision, change)
	if err != nil || updated.Revision != 2 || updated.Note != "rack 5" || !updated.UpdatedAt.After(updated.CreatedAt) {
		t.Fatalf("update: %+v, %v", updated, err)
	}
	other := mustCreate(t, e, Input{Name: "Other", Enabled: true})
	clash := other.Input()
	clash.Name = "Core switch"
	if _, err := e.records.Update(ctx, other.ID, other.Revision, clash); err == nil || !strings.Contains(err.Error(), "already exists") {
		t.Fatalf("rename onto an existing name: %v", err)
	}
	if _, err := e.records.Update(ctx, other.ID, other.Revision, Input{Name: "x", Status: "bogus"}); err == nil {
		t.Fatal("an invalid status was stored")
	}
	got, err := e.records.Get(ctx, created.ID)
	if err != nil || got != updated {
		t.Fatalf("get: %+v, %v", got, err)
	}
	if err := e.records.Delete(ctx, created.ID); err != nil {
		t.Fatal(err)
	}
	if err := e.records.Delete(ctx, created.ID); !errors.Is(err, ErrNotFound) {
		t.Fatalf("second delete: %v", err)
	}
	if _, err := e.records.Get(ctx, created.ID); !errors.Is(err, ErrNotFound) {
		t.Fatalf("get after delete: %v", err)
	}
	if count, _ := e.records.Count(ctx); count != 1 {
		t.Fatalf("count = %d", count)
	}
}

func TestConflictingEditsAreDetected(t *testing.T) {
	ctx := context.Background()
	e := newEnv(t)
	created := mustCreate(t, e, Input{Name: "Shared", Enabled: true})
	first := created.Input()
	first.Note = "from the dashboard"
	if _, err := e.records.Update(ctx, created.ID, created.Revision, first); err != nil {
		t.Fatal(err)
	}
	second := created.Input()
	second.Note = "from the terminal"
	_, err := e.records.Update(ctx, created.ID, created.Revision, second)
	var conflict *ConflictError
	if !errors.As(err, &conflict) || conflict.Deleted || conflict.Current.Note != "from the dashboard" || conflict.Current.Revision != 2 {
		t.Fatalf("got %v, want a conflict carrying the current record", err)
	}
	if !strings.Contains(err.Error(), "another window") {
		t.Fatalf("conflict text: %v", err)
	}
	stored, _ := e.records.Get(ctx, created.ID)
	if stored.Note != "from the dashboard" {
		t.Fatalf("the losing edit overwrote the record: %q", stored.Note)
	}
	if err := e.records.Delete(ctx, created.ID); err != nil {
		t.Fatal(err)
	}
	_, err = e.records.Update(ctx, created.ID, 2, second)
	if !errors.As(err, &conflict) || !conflict.Deleted || !strings.Contains(err.Error(), "removed") {
		t.Fatalf("got %v, want a removed-record conflict", err)
	}
}

func TestSetEnabled(t *testing.T) {
	ctx := context.Background()
	e := newEnv(t)
	created := mustCreate(t, e, Input{Name: "Toggle", Enabled: true})
	disabled, err := e.records.SetEnabled(ctx, created.ID, false)
	if err != nil || disabled.Enabled || disabled.Revision != 2 {
		t.Fatalf("disable: %+v, %v", disabled, err)
	}
	again, err := e.records.SetEnabled(ctx, created.ID, false)
	if err != nil || again.Revision != 2 {
		t.Fatalf("disabling twice must not create a new revision: %+v, %v", again, err)
	}
	if _, err := e.records.SetEnabled(ctx, 404, true); !errors.Is(err, ErrNotFound) {
		t.Fatalf("missing record: %v", err)
	}
	visible, _ := e.records.List(ctx, Query{})
	all, _ := e.records.List(ctx, Query{IncludeDisabled: true})
	if len(visible) != 0 || len(all) != 1 {
		t.Fatalf("disabled filtering: %d visible, %d total", len(visible), len(all))
	}
}

func TestListFilterSortSearchAndPaging(t *testing.T) {
	ctx := context.Background()
	e := newEnv(t)
	inputs := []Input{
		{Name: "alpha router", Status: StatusOpen, Severity: theme.SeverityCritical, Note: "edge firmware", Enabled: true},
		{Name: "Bravo switch", Status: StatusDone, Severity: theme.SeverityLow, Note: "core", Enabled: true},
		{Name: "charlie firewall", Status: StatusInProgress, Severity: theme.SeverityHigh, Note: "ruleset review", Enabled: true},
		{Name: "delta router", Status: StatusOpen, Severity: theme.SeverityInfo, Note: "lab", Enabled: false},
	}
	for _, in := range inputs {
		e.clock.Advance(time.Minute)
		mustCreate(t, e, in)
	}
	names := func(query Query) string {
		t.Helper()
		list, err := e.records.List(ctx, query)
		if err != nil {
			t.Fatalf("list %+v: %v", query, err)
		}
		var out []string
		for _, record := range list {
			out = append(out, strings.Fields(record.Name)[0])
		}
		return strings.Join(out, " ")
	}
	cases := []struct {
		name  string
		query Query
		want  string
	}{
		{"default hides disabled and sorts by name without case", Query{}, "alpha Bravo charlie"},
		{"include disabled", Query{IncludeDisabled: true}, "alpha Bravo charlie delta"},
		{"name descending", Query{Sort: "name", Descending: true}, "charlie Bravo alpha"},
		{"severity ascending", Query{Sort: "severity"}, "Bravo charlie alpha"},
		{"severity descending", Query{Sort: "severity", Descending: true}, "alpha charlie Bravo"},
		{"status ascending", Query{Sort: "status"}, "Bravo charlie alpha"},
		{"updated descending", Query{Sort: "updated", Descending: true, IncludeDisabled: true}, "delta charlie Bravo alpha"},
		{"status filter", Query{Status: StatusOpen, IncludeDisabled: true}, "alpha delta"},
		{"severity filter", Query{Severity: theme.SeverityHigh}, "charlie"},
		{"search in name", Query{Search: "router", IncludeDisabled: true}, "alpha delta"},
		{"search in note", Query{Search: "ruleset"}, "charlie"},
		{"search by prefix", Query{Search: "fire"}, "charlie"},
		{"search needs every term", Query{Search: "alpha firmware"}, "alpha"},
		{"search with no match", Query{Search: "nothing"}, ""},
		{"search made only of punctuation", Query{Search: "***"}, ""},
		{"search with operators is literal", Query{Search: `router" OR "switch`}, ""},
		{"search and filter", Query{Search: "router", Status: StatusOpen}, "alpha"},
		{"limit", Query{Limit: 2}, "alpha Bravo"},
		{"offset", Query{Limit: 2, Offset: 2}, "charlie"},
		{"negative offset", Query{Offset: -5, Limit: 1}, "alpha"},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			if got := names(tc.query); got != tc.want {
				t.Fatalf("got %q, want %q", got, tc.want)
			}
		})
	}
	for _, bad := range []Query{{Sort: "id; DROP TABLE records"}, {Status: "closed"}, {Severity: "urgent"}} {
		if _, err := e.records.List(ctx, bad); err == nil {
			t.Fatalf("query %+v was accepted", bad)
		}
	}
	if count, _ := e.records.Count(ctx); count != 4 {
		t.Fatalf("a hostile query changed the table: %d rows", count)
	}
}

func TestSearchFollowsEditsAndDeletes(t *testing.T) {
	ctx := context.Background()
	e := newEnv(t)
	created := mustCreate(t, e, Input{Name: "printer", Note: "toner", Enabled: true})
	change := created.Input()
	change.Name, change.Note = "scanner", "glass"
	if _, err := e.records.Update(ctx, created.ID, created.Revision, change); err != nil {
		t.Fatal(err)
	}
	for term, want := range map[string]int{"printer": 0, "toner": 0, "scanner": 1, "glass": 1} {
		if list, _ := e.records.List(ctx, Query{Search: term}); len(list) != want {
			t.Fatalf("search %q found %d, want %d", term, len(list), want)
		}
	}
	if err := e.records.Delete(ctx, created.ID); err != nil {
		t.Fatal(err)
	}
	if list, _ := e.records.List(ctx, Query{Search: "scanner"}); len(list) != 0 {
		t.Fatal("a deleted record is still searchable")
	}
}

func TestOverviewMatchesKnownFixture(t *testing.T) {
	ctx := context.Background()
	e := newEnv(t)
	empty, err := e.records.Overview(ctx)
	if err != nil || empty.Total() != 0 || empty.Open != 0 || len(empty.Breakdown) != 0 {
		t.Fatalf("empty overview: %+v, %v", empty, err)
	}
	added, err := e.records.LoadDemo(ctx)
	if err != nil || added != 6 {
		t.Fatalf("load demo: %d, %v", added, err)
	}
	mustCreate(t, e, Input{Name: "Real urgent", Severity: theme.SeverityCritical, Enabled: true})
	overview, err := e.records.Overview(ctx)
	if err != nil {
		t.Fatal(err)
	}
	if overview.Open != 5 || overview.Urgent != 3 || overview.Disabled != 1 || overview.Demo != 6 || overview.UnreadNotifications != 3 {
		t.Fatalf("attention numbers: %+v", overview)
	}
	if overview.Total() != 6 || overview.ByStatus(StatusOpen) != 4 || overview.ByStatus(StatusInProgress) != 1 || overview.ByStatus(StatusDone) != 1 {
		t.Fatalf("status totals: %+v", overview.Breakdown)
	}
	if overview.BySeverity(theme.SeverityCritical) != 2 || overview.BySeverity(theme.SeverityMedium) != 1 || overview.BySeverity(theme.SeverityInfo) != 0 {
		t.Fatalf("severity totals: %+v", overview.Breakdown)
	}
	demoTotal := 0
	for _, row := range overview.Breakdown {
		demoTotal += row.Demo
	}
	if demoTotal != 5 {
		t.Fatalf("demo rows among enabled records = %d, want 5", demoTotal)
	}
}

func TestDemoDataIsLabelledIdempotentAndRemovable(t *testing.T) {
	ctx := context.Background()
	e := newEnv(t)
	real := mustCreate(t, e, Input{Name: "Mine", Enabled: true})
	if added, err := e.records.LoadDemo(ctx); err != nil || added != 6 {
		t.Fatalf("first load: %d, %v", added, err)
	}
	if added, err := e.records.LoadDemo(ctx); err != nil || added != 0 {
		t.Fatalf("second load must add nothing: %d, %v", added, err)
	}
	list, _ := e.records.List(ctx, Query{IncludeDisabled: true})
	for _, record := range list {
		if record.Demo != strings.HasPrefix(record.Name, "Demo: ") {
			t.Fatalf("demo label and flag disagree for %q", record.Name)
		}
		if record.Demo && !strings.HasPrefix(record.Note, "Sample entry.") {
			t.Fatalf("demo note is not labelled: %q", record.Note)
		}
	}
	if count, err := e.records.RemoveDemo(ctx, true); err != nil || count != 6 {
		t.Fatalf("dry run: %d, %v", count, err)
	}
	if count, _ := e.records.DemoCount(ctx); count != 6 {
		t.Fatal("a dry run removed demo data")
	}
	if count, err := e.records.RemoveDemo(ctx, false); err != nil || count != 6 {
		t.Fatalf("remove: %d, %v", count, err)
	}
	if _, err := e.records.Get(ctx, real.ID); err != nil {
		t.Fatalf("removing demo data removed a real record: %v", err)
	}
	if count, _ := e.records.Count(ctx); count != 1 {
		t.Fatalf("count = %d", count)
	}
}

func TestNotificationsFollowRecordChanges(t *testing.T) {
	ctx := context.Background()
	e := newEnv(t)
	events := func() string {
		t.Helper()
		list, err := e.notices.List(ctx, notify.Filter{})
		if err != nil {
			t.Fatal(err)
		}
		var out []string
		for index := len(list) - 1; index >= 0; index-- {
			out = append(out, list[index].EventType)
		}
		return strings.Join(out, ",")
	}
	calm := mustCreate(t, e, Input{Name: "Calm", Severity: theme.SeverityLow, Enabled: true})
	if got := events(); got != "" {
		t.Fatalf("a low record raised %q", got)
	}
	e.clock.Advance(2 * time.Hour)
	change := calm.Input()
	change.Severity = theme.SeverityCritical
	raised, err := e.records.Update(ctx, calm.ID, calm.Revision, change)
	if err != nil {
		t.Fatal(err)
	}
	if got := events(); got != notify.EventRecordUrgent {
		t.Fatalf("after raising severity: %q", got)
	}
	e.clock.Advance(2 * time.Hour)
	change.Note = "still critical"
	edited, err := e.records.Update(ctx, raised.ID, raised.Revision, change)
	if err != nil {
		t.Fatal(err)
	}
	if got := events(); got != notify.EventRecordUrgent {
		t.Fatalf("an edit that keeps the record urgent raised another event: %q", got)
	}
	change.Status = StatusDone
	if _, err := e.records.Update(ctx, edited.ID, edited.Revision, change); err != nil {
		t.Fatal(err)
	}
	if got := events(); got != notify.EventRecordUrgent+","+notify.EventRecordCompleted {
		t.Fatalf("after completion: %q", got)
	}
	hidden := mustCreate(t, e, Input{Name: "Hidden", Severity: theme.SeverityCritical, Enabled: false})
	if got := events(); strings.Count(got, notify.EventRecordUrgent) != 1 {
		t.Fatalf("a disabled record raised an event: %q", got)
	}
	e.clock.Advance(2 * time.Hour)
	if _, err := e.records.SetEnabled(ctx, hidden.ID, true); err != nil {
		t.Fatal(err)
	}
	if got := events(); strings.Count(got, notify.EventRecordUrgent) != 2 {
		t.Fatalf("enabling an urgent record raised no event: %q", got)
	}
	list, _ := e.notices.List(ctx, notify.Filter{})
	for _, item := range list {
		if item.RecordKind != Kind || item.RecordID == 0 {
			t.Fatalf("notification does not link to a record: %+v", item)
		}
		if strings.Contains(item.Title, "Calm") || strings.Contains(item.Title, "Hidden") {
			t.Fatalf("notification title carries record content: %q", item.Title)
		}
	}
}

func TestNotificationRulesApplyToRecordEvents(t *testing.T) {
	ctx := context.Background()
	e := newEnv(t)
	if err := e.notices.SetRule(ctx, notify.Rule{EventType: notify.EventRecordUrgent, Enabled: true, MinSeverity: theme.SeverityCritical, DedupMinutes: 60}); err != nil {
		t.Fatal(err)
	}
	mustCreate(t, e, Input{Name: "High", Severity: theme.SeverityHigh, Enabled: true})
	mustCreate(t, e, Input{Name: "Critical", Severity: theme.SeverityCritical, Enabled: true})
	if count, _ := e.notices.UnreadCount(ctx); count != 1 {
		t.Fatalf("threshold rule not applied: %d notifications", count)
	}
}

func TestConcurrentWritersNeverCorruptData(t *testing.T) {
	ctx := context.Background()
	path := testkit.DatabasePath(t)
	clock := testkit.NewClock()
	first := New(testkit.OpenAt(t, path).DB(), clock.Now)
	second := New(testkit.OpenAt(t, path).DB(), clock.Now)
	shared, err := first.Create(ctx, Input{Name: "Shared", Enabled: true})
	if err != nil {
		t.Fatal(err)
	}
	var wait sync.WaitGroup
	failures := make(chan error, 200)
	wins := make(chan int, 2)
	for index, service := range []*Service{first, second} {
		wait.Add(1)
		go func(index int, service *Service) {
			defer wait.Done()
			won := 0
			for step := 0; step < 40; step++ {
				name := "writer " + string(rune('A'+index)) + " item " + string(rune('a'+step%26)) + string(rune('a'+step/26))
				if _, err := service.Create(ctx, Input{Name: name, Severity: theme.SeverityHigh, Enabled: true}); err != nil {
					failures <- err
				}
				current, err := service.Get(ctx, shared.ID)
				if err != nil {
					failures <- err
					continue
				}
				change := current.Input()
				change.Note = name
				_, err = service.Update(ctx, current.ID, current.Revision, change)
				var conflict *ConflictError
				if err == nil {
					won++
				} else if !errors.As(err, &conflict) {
					failures <- err
				}
			}
			wins <- won
		}(index, service)
	}
	wait.Wait()
	close(failures)
	for err := range failures {
		t.Errorf("writer failed: %v", err)
	}
	total := <-wins + <-wins
	final, err := first.Get(ctx, shared.ID)
	if err != nil || final.Revision != int64(1+total) {
		t.Fatalf("revision %d does not match %d successful updates: %v", final.Revision, total, err)
	}
	if count, _ := second.Count(ctx); count != 81 {
		t.Fatalf("count = %d, want 81", count)
	}
	opened := testkit.OpenAt(t, path)
	if err := opened.IntegrityCheck(ctx); err != nil {
		t.Fatalf("integrity check: %v", err)
	}
	found, _ := first.List(ctx, Query{Search: "writer", Limit: 1000})
	if total == 0 || len(found) != 81 {
		t.Fatalf("%d updates succeeded and the search index has %d of 81 rows", total, len(found))
	}
}

func TestExportImportRoundTrip(t *testing.T) {
	ctx := context.Background()
	source := newEnv(t)
	inputs := []Input{
		{Name: "plain", Status: StatusOpen, Severity: theme.SeverityLow, Note: "note", Enabled: true},
		{Name: "=HYPERLINK(\"x\")", Status: StatusDone, Severity: theme.SeverityHigh, Note: "+SUM(A1)", Enabled: false},
		{Name: "comma, \"quote\"", Status: StatusInProgress, Severity: theme.SeverityCritical, Note: "two\nlines", Enabled: true},
		{Name: "'already quoted", Status: StatusOpen, Severity: theme.SeverityInfo, Note: "-dash and @at", Enabled: true},
		{Name: "ünïcödé 名前", Status: StatusOpen, Severity: theme.SeverityMedium, Note: "", Enabled: true},
	}
	for _, in := range inputs {
		mustCreate(t, source, in)
	}
	list, _ := source.records.List(ctx, Query{IncludeDisabled: true})
	for _, format := range []string{FormatCSV, FormatJSON} {
		t.Run(format, func(t *testing.T) {
			var buffer bytes.Buffer
			if err := Export(&buffer, format, list); err != nil {
				t.Fatal(err)
			}
			if format == FormatCSV {
				for _, line := range strings.Split(buffer.String(), "\n")[1:] {
					for _, lead := range []string{"=", "+", "-", "@"} {
						if strings.HasPrefix(line, lead) || strings.HasPrefix(line, `"`+lead) {
							t.Fatalf("a CSV line starts a formula: %q", line)
						}
					}
				}
			}
			target := newEnv(t)
			result, err := target.records.Import(ctx, bytes.NewReader(buffer.Bytes()), format, false)
			if err != nil || result.Created != len(inputs) || len(result.Problems) != 0 {
				t.Fatalf("import: %+v, %v", result, err)
			}
			copied, _ := target.records.List(ctx, Query{IncludeDisabled: true})
			if len(copied) != len(list) {
				t.Fatalf("copied %d of %d", len(copied), len(list))
			}
			for index := range list {
				if copied[index].Input() != list[index].Input() {
					t.Fatalf("row %d differs:\n got %+v\nwant %+v", index, copied[index].Input(), list[index].Input())
				}
			}
			again, err := target.records.Import(ctx, bytes.NewReader(buffer.Bytes()), format, false)
			if err != nil || again.Created != 0 || again.Updated != 0 || again.Skipped != len(inputs) {
				t.Fatalf("second import must change nothing: %+v, %v", again, err)
			}
		})
	}
	if err := Export(&bytes.Buffer{}, "xml", list); err == nil {
		t.Fatal("an unknown export format was accepted")
	}
}

func TestImportUpdatesRejectsAndSupportsDryRun(t *testing.T) {
	ctx := context.Background()
	e := newEnv(t)
	existing := mustCreate(t, e, Input{Name: "existing", Severity: theme.SeverityLow, Enabled: true})
	content := "Name,Severity,Status,Enabled\nexisting,critical,open,yes\nfresh,low,,\n,low,open,true\nbad status,low,closed,true\nfresh,high,open,true\nbad flag,low,open,maybe\nshort row\n"
	dry, err := e.records.Import(ctx, strings.NewReader(content), FormatCSV, true)
	if err != nil {
		t.Fatal(err)
	}
	if !dry.DryRun || dry.Created != 2 || dry.Updated != 1 || len(dry.Problems) != 4 || !strings.HasPrefix(dry.Summary(), "Dry run: would have created 2, updated 1") {
		t.Fatalf("dry run: %+v %q", dry, dry.Summary())
	}
	if count, _ := e.records.Count(ctx); count != 1 {
		t.Fatalf("a dry run stored %d records", count)
	}
	if count, _ := e.notices.UnreadCount(ctx); count != 0 {
		t.Fatal("a dry run raised notifications")
	}
	unchanged, _ := e.records.Get(ctx, existing.ID)
	if unchanged != existing {
		t.Fatal("a dry run changed a record")
	}
	real, err := e.records.Import(ctx, strings.NewReader(content), FormatCSV, false)
	if err != nil || real.Created != 2 || real.Updated != 1 || real.Summary() != "created 2, updated 1, unchanged 0, rejected 4" {
		t.Fatalf("import: %+v, %v", real, err)
	}
	rows := map[int]string{}
	for _, problem := range real.Problems {
		rows[problem.Row] = problem.Error()
	}
	for row, text := range map[int]string{4: "name is empty", 5: "unknown status", 6: "repeats row 3", 7: "not a yes or no value"} {
		if !strings.Contains(rows[row], text) {
			t.Fatalf("row %d: got %q, want %q", row, rows[row], text)
		}
	}
	updated, _ := e.records.Get(ctx, existing.ID)
	if updated.Severity != theme.SeverityCritical || updated.Revision != 2 {
		t.Fatalf("existing record was not updated: %+v", updated)
	}
	if count, _ := e.notices.UnreadCount(ctx); count != 1 {
		t.Fatalf("import raised %d notifications, want 1", count)
	}
}

func TestParseRejectsBadInput(t *testing.T) {
	huge := strings.Repeat("x", MaxImportBytes+1)
	var manyRows strings.Builder
	manyRows.WriteString("name\n")
	for index := 0; index <= MaxImportRows; index++ {
		manyRows.WriteString("row ")
		manyRows.WriteString(strings.Repeat("i", index%7+1))
		manyRows.WriteString("\n")
	}
	cases := []struct {
		name, format, content, text string
	}{
		{"empty", FormatCSV, "", "empty"},
		{"only whitespace", FormatJSON, " \n\t", "empty"},
		{"too large", FormatCSV, huge, "larger than"},
		{"too many rows", FormatCSV, manyRows.String(), "more than"},
		{"no name column", FormatCSV, "title,status\na,open\n", `no "name" column`},
		{"broken quoting", FormatCSV, "name\n\"unterminated\n", "cannot be read"},
		{"json object instead of list", FormatJSON, `{"name":"a"}`, "cannot be read"},
		{"json unknown field", FormatJSON, `[{"name":"a","admin":true}]`, "cannot be read"},
		{"json trailing content", FormatJSON, `[{"name":"a"}] [{"name":"b"}]`, "extra content"},
		{"json truncated", FormatJSON, `[{"name":"a"`, "cannot be read"},
		{"json wrong type", FormatJSON, `[{"name":5}]`, "cannot be read"},
		{"unknown format", "xml", "<a/>", "unknown format"},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			_, _, err := Parse(strings.NewReader(tc.content), tc.format)
			if err == nil || !strings.Contains(err.Error(), tc.text) {
				t.Fatalf("got %v, want an error with %q", err, tc.text)
			}
		})
	}
	inputs, problems, err := Parse(strings.NewReader("\xef\xbb\xbfname\r\nwith bom\r\n"), FormatCSV)
	if err != nil || len(inputs) != 1 || inputs[0].Name != "with bom" || len(problems) != 0 {
		t.Fatalf("byte-order mark and CRLF: %+v, %v, %v", inputs, problems, err)
	}
	inputs, _, err = Parse(strings.NewReader(`[{"name":"defaults"}]`), FormatJSON)
	if err != nil || len(inputs) != 1 || !inputs[0].Enabled || inputs[0].Status != StatusOpen {
		t.Fatalf("JSON defaults: %+v, %v", inputs, err)
	}
}

type failingReader struct{}

func (failingReader) Read([]byte) (int, error) { return 0, errors.New("disk unplugged") }

func TestImportReportsReadFailure(t *testing.T) {
	e := newEnv(t)
	if _, err := e.records.Import(context.Background(), failingReader{}, FormatCSV, false); err == nil || !strings.Contains(err.Error(), "cannot read the import") {
		t.Fatalf("got %v", err)
	}
}

func FuzzParse(f *testing.F) {
	f.Add("name,status\na,open\n", true)
	f.Add(`[{"name":"a","enabled":false}]`, false)
	f.Add("name\n\"a\"\"b\"\n", true)
	f.Add("\xef\xbb\xbfname\n=1+1\n", true)
	f.Add(`[{"name":"\u0000"}]`, false)
	f.Fuzz(func(t *testing.T, content string, asCSV bool) {
		format := FormatJSON
		if asCSV {
			format = FormatCSV
		}
		inputs, problems, err := Parse(strings.NewReader(content), format)
		if err != nil {
			if len(inputs) != 0 || len(problems) != 0 {
				t.Fatal("a failed parse returned rows")
			}
			return
		}
		seen := map[string]bool{}
		for _, in := range inputs {
			again, err := Normalize(in)
			if err != nil || again != in {
				t.Fatalf("parsed row is not normalized: %+v, %v", in, err)
			}
			key := strings.ToLower(in.Name)
			if seen[key] {
				t.Fatalf("duplicate name %q survived", in.Name)
			}
			seen[key] = true
		}
	})
}

func TestStatusPresentation(t *testing.T) {
	seenLabel, seenGlyph := map[string]bool{}, map[string]bool{}
	for _, status := range Statuses {
		seenLabel[StatusLabel(status)], seenGlyph[StatusGlyph(status)] = true, true
	}
	if len(seenLabel) != 3 || len(seenGlyph) != 3 || StatusLabel("x") != "x" || StatusGlyph("x") != "?" {
		t.Fatal("each status needs its own label and glyph")
	}
}
