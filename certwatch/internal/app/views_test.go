package app

import (
	"context"
	"crypto/tls"
	"errors"
	"net"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"sync"
	"testing"
	"time"

	"github.com/NexzaDev/certwatch/internal/certgen"
	"github.com/NexzaDev/certwatch/internal/inventory"
	"github.com/NexzaDev/certwatch/internal/settings"
	"github.com/NexzaDev/certwatch/internal/theme"
)

type liveServer struct {
	port  int
	lock  sync.Mutex
	chain tls.Certificate
	dials []string
}

func startServer(t *testing.T, root *certgen.Authority, now time.Time, name string, days int) *liveServer {
	t.Helper()
	server := &liveServer{}
	server.present(t, root, now, name, days)
	listener, err := tls.Listen("tcp", "127.0.0.1:0", &tls.Config{GetCertificate: func(*tls.ClientHelloInfo) (*tls.Certificate, error) {
		server.lock.Lock()
		defer server.lock.Unlock()
		chain := server.chain
		return &chain, nil
	}})
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = listener.Close() })
	server.port = listener.Addr().(*net.TCPAddr).Port
	go func() {
		for {
			connection, err := listener.Accept()
			if err != nil {
				return
			}
			go func() {
				_ = connection.SetDeadline(time.Now().Add(5 * time.Second))
				_ = connection.(*tls.Conn).Handshake()
				_ = connection.Close()
			}()
		}
	}()
	return server
}

func (s *liveServer) present(t *testing.T, root *certgen.Authority, now time.Time, name string, days int) {
	t.Helper()
	issued, err := root.Issue(certgen.Request{CommonName: name, Names: []string{name}, NotBefore: now.AddDate(0, 0, -5), NotAfter: now.AddDate(0, 0, days)})
	if err != nil {
		t.Fatal(err)
	}
	s.lock.Lock()
	s.chain = tls.Certificate{Certificate: [][]byte{issued.Certificate.Raw}, PrivateKey: issued.Key}
	s.lock.Unlock()
}

func (s *liveServer) dial(ctx context.Context, network, address string) (net.Conn, error) {
	s.lock.Lock()
	s.dials = append(s.dials, address)
	s.lock.Unlock()
	return (&net.Dialer{}).DialContext(ctx, network, "127.0.0.1:"+strconv.Itoa(s.port))
}

func (s *liveServer) dialed() []string {
	s.lock.Lock()
	defer s.lock.Unlock()
	return append([]string(nil), s.dials...)
}

type viewFixture struct {
	t      *testing.T
	ctx    context.Context
	s      *sandbox
	app    *App
	root   *certgen.Authority
	server *liveServer
}

func newViewFixture(t *testing.T) *viewFixture {
	t.Helper()
	s := newSandbox(t)
	s.clock.Set(time.Now().UTC().Truncate(time.Second))
	now := s.clock.Now()
	root, err := certgen.NewAuthority("Fixture Root", now.AddDate(-1, 0, 0), now.AddDate(5, 0, 0))
	if err != nil {
		t.Fatal(err)
	}
	server := startServer(t, root, now, "app.fixture.test", 20)
	opened := s.open(t, func(options *Options) {
		options.Roots = root.Pool()
		options.Dial = server.dial
	})
	return &viewFixture{t: t, ctx: context.Background(), s: s, app: opened, root: root, server: server}
}

func (f *viewFixture) run(view, action string, id int64, values map[string]string) Outcome {
	f.t.Helper()
	request := Request{View: view, Action: action, ID: id, Values: values}
	if _, err := f.app.Plan(f.ctx, request); err != nil {
		f.t.Fatalf("plan %s/%s: %v", view, action, err)
	}
	outcome, err := f.app.Run(f.ctx, request)
	if err != nil {
		f.t.Fatalf("run %s/%s: %v", view, action, err)
	}
	return outcome
}

func (f *viewFixture) table(view string, query ViewQuery) Table {
	f.t.Helper()
	table, err := f.app.Table(f.ctx, view, query)
	if err != nil {
		f.t.Fatalf("table %s: %v", view, err)
	}
	for _, row := range table.Rows {
		if len(row.Cells) != len(table.Columns) {
			f.t.Fatalf("%s: a row has %d cells for %d columns", view, len(row.Cells), len(table.Columns))
		}
	}
	return table
}

func cell(table Table, row int, column string) string {
	for index, candidate := range table.Columns {
		if candidate.ID == column {
			return table.Rows[row].Cells[index]
		}
	}
	return "<no column " + column + ">"
}

func TestCheckRunsOnlyAfterAnAuthorizedEndpointAndDisclosesTheConnection(t *testing.T) {
	f := newViewFixture(t)
	request := Request{View: ViewEndpoints, Action: "add", Values: map[string]string{"host": "app.fixture.test", "port": "8443"}}
	if _, err := f.app.Run(f.ctx, request); !errors.Is(err, inventory.ErrNotAuthorized) {
		t.Fatalf("an endpoint was added without authorization: %v", err)
	}
	request.Values["authorized"] = "true"
	added := f.run(ViewEndpoints, "add", 0, request.Values)
	if added.Select == 0 || !strings.Contains(added.Message, "app.fixture.test:8443") {
		t.Fatalf("outcome: %+v", added)
	}
	if len(f.server.dialed()) != 0 {
		t.Fatalf("adding an endpoint opened a connection: %v", f.server.dialed())
	}
	plan, err := f.app.Plan(f.ctx, Request{View: ViewEndpoints, Action: "check", ID: added.Select})
	if err != nil || !plan.Confirm || len(plan.Network) != 1 || !strings.Contains(plan.Network[0], "app.fixture.test:8443") || !strings.Contains(plan.Network[0], "one TCP connection") {
		t.Fatalf("plan: %+v %v", plan, err)
	}
	if len(f.server.dialed()) != 0 {
		t.Fatalf("planning a check opened a connection")
	}
	if _, err := f.app.Settings.Set(f.ctx, settings.KeyConfirm, "false"); err != nil {
		t.Fatal(err)
	}
	if plan, _ := f.app.Plan(f.ctx, Request{View: ViewEndpoints, Action: "check", ID: added.Select}); !plan.Confirm {
		t.Fatalf("network checks must always ask for confirmation")
	}
	checked := f.run(ViewEndpoints, "check", added.Select, nil)
	if !strings.Contains(checked.Message, "Checked 1 endpoint: 0 failed, 0 changed certificates, 1 new alerts") || len(checked.Lines) != 1 || !strings.Contains(checked.Lines[0], "app.fixture.test:8443: ok") {
		t.Fatalf("outcome: %+v", checked)
	}
	if dialed := f.server.dialed(); len(dialed) != 1 || dialed[0] != "app.fixture.test:8443" {
		t.Fatalf("connections: %v", dialed)
	}
	endpoints := f.table(ViewEndpoints, ViewQuery{})
	if len(endpoints.Rows) != 1 || cell(endpoints, 0, "certificate") != "app.fixture.test" || cell(endpoints, 0, "days") != "20 days" || cell(endpoints, 0, "tls") != "TLS 1.3" || endpoints.Rows[0].Severity != theme.SeverityMedium {
		t.Fatalf("endpoint row: %+v", endpoints.Rows)
	}
	f.server.present(t, f.root, f.s.clock.Now(), "other.fixture.test", 200)
	f.s.clock.Advance(2 * time.Hour)
	probe, err := f.app.Plan(f.ctx, Request{View: ViewConfiguration, Action: "probe", ID: added.Select})
	if err != nil || !strings.Contains(probe.Network[0], "up to four TCP connections") {
		t.Fatalf("probe plan: %+v %v", probe, err)
	}
	probed := f.run(ViewConfiguration, "probe", added.Select, nil)
	if !strings.Contains(probed.Message, "1 failed, 1 changed certificates, 2 new alerts") || !strings.Contains(probed.Lines[0], "name mismatch") {
		t.Fatalf("outcome: %+v", probed)
	}
	configuration := f.table(ViewConfiguration, ViewQuery{Filter: "findings"})
	if len(configuration.Rows) != 1 || cell(configuration, 0, "accepted") != "TLS 1.2, TLS 1.3" || cell(configuration, 0, "findings") != "1" {
		t.Fatalf("configuration row: %+v", configuration.Rows)
	}
	detail, err := f.app.Detail(f.ctx, ViewEndpoints, added.Select)
	if err != nil {
		t.Fatal(err)
	}
	text := detailText(detail)
	for _, want := range []string{"name mismatch", "Host name mismatch", "Observations (latest 2 of 2)", "certificate changed", "app.fixture.test", "other.fixture.test", "Fixture Root", "trust store"} {
		if !strings.Contains(text, want) {
			t.Errorf("the endpoint detail lacks %q:\n%s", want, text)
		}
	}
	overview, err := f.app.Overview(f.ctx)
	if err != nil || overview.Counts.HostnameMismatches != 1 || len(overview.Alerts.Rows) != 3 || len(overview.Soonest.Rows) != 1 {
		t.Fatalf("overview: %+v %v", overview, err)
	}
}

func detailText(detail Detail) string {
	var builder strings.Builder
	builder.WriteString(detail.Title + "\n" + detail.Subtitle + "\n")
	for _, section := range detail.Sections {
		builder.WriteString(section.Title + "\n" + section.Text + "\n")
		for _, fact := range section.Facts {
			builder.WriteString(fact[0] + ": " + fact[1] + "\n")
		}
		if section.Table != nil {
			for _, row := range section.Table.Rows {
				builder.WriteString(strings.Join(row.Cells, " | ") + "\n")
			}
			if len(section.Table.Rows) == 0 {
				builder.WriteString(section.Table.Empty + "\n")
			}
		}
	}
	return builder.String()
}

func TestDemoEndpointsAreNeverContacted(t *testing.T) {
	f := newViewFixture(t)
	if added, err := f.app.LoadDemo(f.ctx); err != nil || added == 0 {
		t.Fatalf("demo: %d %v", added, err)
	}
	plan, err := f.app.Plan(f.ctx, Request{View: ViewEndpoints, Action: "check-all"})
	if err != nil || plan.Confirm || len(plan.Network) != 0 || !strings.Contains(plan.Statement, "nothing to check") {
		t.Fatalf("plan: %+v %v", plan, err)
	}
	outcome := f.run(ViewEndpoints, "check-all", 0, nil)
	if !strings.Contains(outcome.Message, "Nothing was checked") || len(f.server.dialed()) != 0 {
		t.Fatalf("outcome %+v, connections %v", outcome, f.server.dialed())
	}
	if _, err := f.app.Settings.Set(f.ctx, settings.KeyCheckInterval, "5"); err != nil {
		t.Fatal(err)
	}
	if done, err := f.app.Housekeep(f.ctx); err != nil || done.Report.Checked != 0 || len(f.server.dialed()) != 0 {
		t.Fatalf("housekeeping contacted demo endpoints: %+v %v", done, err)
	}
}

func TestOverviewTilesAndEveryViewShowTheSameNumbers(t *testing.T) {
	f := newViewFixture(t)
	if _, err := f.app.LoadDemo(f.ctx); err != nil {
		t.Fatal(err)
	}
	overview, err := f.app.Overview(f.ctx)
	if err != nil || overview.Empty || !overview.Demo || len(overview.Tiles) != 8 {
		t.Fatalf("overview: %+v %v", overview, err)
	}
	for _, tile := range overview.Tiles {
		table := f.table(tile.View, ViewQuery{Filter: tile.Filter})
		if len(table.Rows) != tile.Value {
			t.Errorf("tile %q says %d, %s filtered by %s lists %d", tile.Label, tile.Value, tile.View, tile.Filter, len(table.Rows))
		}
		if (tile.Value == 0) != (tile.Severity == "") {
			t.Errorf("tile %q: value %d with severity %q", tile.Label, tile.Value, tile.Severity)
		}
	}
	if len(overview.Soonest.Rows) != 8 || !strings.Contains(overview.Soonest.Rows[0].Cells[2], "expired") || len(overview.Alerts.Rows) != 5 {
		t.Fatalf("soonest %+v alerts %d", overview.Soonest.Rows, len(overview.Alerts.Rows))
	}
	for _, view := range Views() {
		for _, filter := range view.Filters {
			if filter.Label == "" {
				t.Errorf("%s: filter %s has no label", view.ID, filter.ID)
			}
			table := f.table(view.ID, ViewQuery{Filter: filter.ID})
			if table.Empty == "" || table.Footer == "" {
				t.Errorf("%s filtered by %s has no empty text or footer", view.ID, filter.ID)
			}
			for _, row := range table.Rows {
				detail, err := f.app.Detail(f.ctx, view.ID, row.ID)
				if err != nil || detail.Title == "" || len(detail.Sections) == 0 {
					t.Fatalf("%s detail %d: %+v %v", view.ID, row.ID, detail, err)
				}
				if view.ID != ViewReports && !row.Demo {
					t.Errorf("%s: demo row %v is not labeled", view.ID, row.Cells)
				}
			}
		}
		if _, err := f.app.Table(f.ctx, view.ID, ViewQuery{Filter: "nonsense"}); err == nil {
			t.Errorf("%s accepted an unknown filter", view.ID)
		}
	}
	timeline := f.table(ViewTimeline, ViewQuery{Filter: "30"})
	if len(timeline.Rows) != 3 || timeline.Window != 30 || len(timeline.Marks) != 3 || !strings.Contains(timeline.Footer, "3 of 9 tracked certificates expire within 30 days") {
		t.Fatalf("timeline: %d rows, %q", len(timeline.Rows), timeline.Footer)
	}
	if cell(timeline, 0, "timeline") != Bar(-3, 30) || cell(timeline, 1, "timeline") != Bar(5, 30) {
		t.Fatalf("bars: %q %q", cell(timeline, 0, "timeline"), cell(timeline, 1, "timeline"))
	}
	everything := f.table(ViewTimeline, ViewQuery{Filter: "all"})
	if len(everything.Rows) != 9 || everything.Window < 330 {
		t.Fatalf("zoomed out: %d rows over %d days", len(everything.Rows), everything.Window)
	}
	chains := f.table(ViewChains, ViewQuery{Filter: "invalid"})
	if len(chains.Rows) != 3 || len(f.table(ViewChains, ViewQuery{}).Rows) != 9 {
		t.Fatalf("chains: %d invalid", len(chains.Rows))
	}
	searched := f.table(ViewCertificates, ViewQuery{Search: "www.demo"})
	if len(searched.Rows) != 1 || cell(searched, 0, "used") != "blog.demo.invalid:443" {
		t.Fatalf("search: %+v", searched.Rows)
	}
	detail, err := f.app.Detail(f.ctx, ViewCertificates, searched.Rows[0].ID)
	if err != nil {
		t.Fatal(err)
	}
	for _, want := range []string{"Subject alternative names (1)", "www.demo.invalid", "CertWatch Demo Issuing CA (demo)", "blog.demo.invalid:443", "intermediate", "observed, demo"} {
		if !strings.Contains(detailText(detail), want) {
			t.Errorf("the certificate detail lacks %q", want)
		}
	}
	portal := f.table(ViewEndpoints, ViewQuery{Filter: "changed"})
	if len(portal.Rows) != 1 || cell(portal, 0, "endpoint") != "portal.demo.invalid:443" {
		t.Fatalf("changed endpoints: %+v", portal.Rows)
	}
	history, err := f.app.Detail(f.ctx, ViewEndpoints, portal.Rows[0].ID)
	if err != nil || !strings.Contains(detailText(history), "Change history") || !strings.Contains(detailText(history), "never contacted") {
		t.Fatalf("history: %v\n%s", err, detailText(history))
	}
}

func TestBarAndDaysText(t *testing.T) {
	if got := Bar(15, 30); got != "▕"+strings.Repeat("█", 12)+strings.Repeat("░", 12)+"▏" {
		t.Fatalf("half: %q", got)
	}
	if got := Bar(0, 30); got != "▕"+strings.Repeat("░", 24)+"▏" {
		t.Fatalf("empty: %q", got)
	}
	if got := Bar(1, 365); !strings.HasPrefix(got, "▕█░") {
		t.Fatalf("one day must stay visible: %q", got)
	}
	if got := Bar(400, 365); got != "▕"+strings.Repeat("█", 24)+"›" {
		t.Fatalf("beyond: %q", got)
	}
	now := time.Date(2026, 5, 1, 12, 0, 0, 0, time.UTC)
	cases := map[time.Duration]string{-72 * time.Hour: "expired 3 days ago", -47 * time.Hour: "expired 1 day ago", -2 * time.Hour: "expired", 0: "expired", 3 * time.Hour: "today", 30 * time.Hour: "1 day", 49 * time.Hour: "2 days"}
	for offset, want := range cases {
		if got := DaysText(now.Add(offset), now); got != want {
			t.Errorf("DaysText(%s) = %q, want %q", offset, got, want)
		}
	}
	if DaysText(time.Time{}, now) != "" {
		t.Errorf("no expiry must give no text")
	}
}

func TestEveryActionIsPlannedExecutedAndListedAsACapability(t *testing.T) {
	f := newViewFixture(t)
	dir := t.TempDir()
	pemFile := filepath.Join(dir, "bundle.pem")
	if err := os.WriteFile(pemFile, expiringPEM(t, f.s.clock.Now(), 40), 0o600); err != nil {
		t.Fatal(err)
	}
	listFile := filepath.Join(dir, "list.csv")
	if err := os.WriteFile(listFile, []byte("host,port\nlisted.fixture.test,443\nbad host,1\n"), 0o600); err != nil {
		t.Fatal(err)
	}
	added := f.run(ViewEndpoints, "add", 0, map[string]string{"host": "app.fixture.test", "authorized": "true"})
	imported := f.run(ViewCertificates, "import", 0, map[string]string{"file": pemFile})
	if !strings.Contains(imported.Message, "Imported 1 certificates") {
		t.Fatalf("import: %+v", imported)
	}
	certificate := f.table(ViewCertificates, ViewQuery{}).Rows[0].ID
	report := f.run(ViewReports, "generate", 0, map[string]string{"kind": "expiry", "format": "csv"})
	values := map[string]map[string]string{
		"certificates/export": {"file": filepath.Join(dir, "one.pem")},
		"endpoints/edit":      {"note": "edited"},
		"endpoints/import":    {"file": listFile, "authorized": "true"},
		"endpoints/export":    {"file": filepath.Join(dir, "out.json")},
		"reports/export":      {"file": filepath.Join(dir, "report.csv")},
	}
	ids := map[string]int64{ViewCertificates: certificate, ViewEndpoints: added.Select, ViewConfiguration: added.Select, ViewReports: report.Select}
	capabilities := map[string]bool{}
	for _, capability := range Capabilities() {
		capabilities[capability.CLI] = true
	}
	order := map[string][]string{ViewEndpoints: {"edit", "disable", "enable", "check", "check-all", "import", "export", "remove"}, ViewCertificates: {"export", "stop-tracking"}, ViewConfiguration: {}, ViewReports: {"export", "remove"}}
	done := map[string]bool{"endpoints/add": true, "certificates/import": true, "reports/generate": true}
	for _, view := range Views() {
		keys := map[string]bool{}
		for _, action := range view.Actions {
			if action.Label == "" || action.Help == "" || action.Key == "" || keys[action.Key] {
				t.Errorf("%s/%s is incomplete or repeats a key: %+v", view.ID, action.ID, action)
			}
			keys[action.Key] = true
			if !capabilities[view.ID+" "+action.ID] {
				t.Errorf("%s/%s is missing from the capability matrix", view.ID, action.ID)
			}
			if action.Row {
				if _, err := f.app.Plan(f.ctx, Request{View: view.ID, Action: action.ID}); err == nil || !strings.Contains(err.Error(), "needs one entry") {
					t.Errorf("%s/%s without a row: %v", view.ID, action.ID, err)
				}
			}
		}
		for _, id := range order[view.ID] {
			action, found := view.Action(id)
			if !found {
				t.Fatalf("%s has no action %s", view.ID, id)
			}
			request := Request{View: view.ID, Action: id, Values: values[view.ID+"/"+id]}
			if action.Row {
				request.ID = ids[view.ID]
			}
			defaults, err := f.app.Defaults(f.ctx, request)
			if err != nil {
				t.Fatalf("%s/%s defaults: %v", view.ID, id, err)
			}
			for _, field := range action.Fields {
				if _, found := defaults[field.ID]; !found {
					t.Fatalf("%s/%s has no default for %s: %v", view.ID, id, field.ID, defaults)
				}
			}
			plan, err := f.app.Plan(f.ctx, request)
			if err != nil || plan.Statement == "" || plan.Confirm != (action.Network || action.Danger) || plan.Danger != action.Danger {
				t.Fatalf("%s/%s plan: %+v %v", view.ID, id, plan, err)
			}
			outcome, err := f.app.Run(f.ctx, request)
			if err != nil || outcome.Message == "" {
				t.Fatalf("%s/%s: %+v %v", view.ID, id, outcome, err)
			}
			done[view.ID+"/"+id] = true
		}
		for _, action := range view.Actions {
			if !done[view.ID+"/"+action.ID] && action.ID != "probe" {
				t.Errorf("%s/%s was not exercised", view.ID, action.ID)
			}
		}
	}
	for _, name := range []string{"one.pem", "out.json", "report.csv"} {
		info, err := os.Stat(filepath.Join(dir, name))
		if err != nil || info.Size() == 0 {
			t.Errorf("%s was not written: %v", name, err)
		}
	}
	var exists *ExistsError
	if _, err := f.app.Run(f.ctx, Request{View: ViewEndpoints, Action: "export", Values: map[string]string{"file": filepath.Join(dir, "out.json")}}); !errors.As(err, &exists) {
		t.Fatalf("an existing file was overwritten: %v", err)
	}
	if _, err := f.app.Run(f.ctx, Request{View: ViewEndpoints, Action: "export", Values: map[string]string{"file": filepath.Join(dir, "out.json"), "overwrite": "true"}}); err != nil {
		t.Fatalf("overwrite: %v", err)
	}
	failures := []struct {
		request Request
		want    string
	}{
		{Request{View: "nonsense", Action: "x"}, "unknown view"},
		{Request{View: ViewEndpoints, Action: "nonsense"}, "has no action"},
		{Request{View: ViewEndpoints, Action: "add", Values: map[string]string{"authorized": "true"}}, "Host is missing"},
		{Request{View: ViewEndpoints, Action: "add", Values: map[string]string{"host": "x.fixture.test", "port": "http", "authorized": "true"}}, "not a port"},
		{Request{View: ViewReports, Action: "generate", Values: map[string]string{"kind": "pdf"}}, "is not one of inventory, expiry, findings"},
		{Request{View: ViewEndpoints, Action: "export", Values: map[string]string{"file": filepath.Join(dir, "out.txt")}}, "choose csv or json"},
		{Request{View: ViewCertificates, Action: "import", Values: map[string]string{"file": filepath.Join(dir, "missing.pem")}}, "cannot open the file"},
		{Request{View: ViewCertificates, Action: "import", Values: map[string]string{"file": listFile}}, "no PEM certificate"},
		{Request{View: ViewReports, Action: "remove", ID: 999}, "no such entry"},
	}
	for _, failure := range failures {
		if _, err := f.app.Plan(f.ctx, failure.request); err == nil || !strings.Contains(err.Error(), failure.want) {
			t.Errorf("plan %s/%s: %v, want %q", failure.request.View, failure.request.Action, err, failure.want)
		}
		if _, err := f.app.Run(f.ctx, failure.request); err == nil {
			t.Errorf("run %s/%s succeeded", failure.request.View, failure.request.Action)
		}
	}
}

func TestEndpointTableFollowsTheColumnAndSortSettings(t *testing.T) {
	f := newViewFixture(t)
	for _, host := range []string{"b.fixture.test", "a.fixture.test", "c.fixture.test"} {
		f.run(ViewEndpoints, "add", 0, map[string]string{"host": host, "authorized": "true"})
	}
	table := f.table(ViewEndpoints, ViewQuery{})
	if len(table.Columns) != len(settings.EndpointColumns) || cell(table, 0, "endpoint") != "a.fixture.test:443" || cell(table, 0, "status") != "not checked yet" {
		t.Fatalf("default table: %+v", table)
	}
	for index, column := range table.Columns {
		if column.ID != settings.EndpointColumns[index] || column.Title == "" || column.Width == 0 {
			t.Fatalf("column %d: %+v", index, column)
		}
	}
	if _, err := f.app.Settings.Set(f.ctx, settings.KeyEndpointColumns, "status,state"); err != nil {
		t.Fatal(err)
	}
	if _, err := f.app.Settings.Set(f.ctx, settings.KeyEndpointSort, "endpoint:desc"); err != nil {
		t.Fatal(err)
	}
	table = f.table(ViewEndpoints, ViewQuery{})
	if len(table.Columns) != 3 || table.Columns[0].ID != "endpoint" || cell(table, 0, "endpoint") != "c.fixture.test:443" || cell(table, 2, "state") != "enabled" {
		t.Fatalf("customized table: %+v", table)
	}
	if empty := f.table(ViewEndpoints, ViewQuery{Search: "zzz"}); len(empty.Rows) != 0 || !strings.Contains(empty.Empty, "matches") {
		t.Fatalf("empty search: %+v", empty)
	}
}

func TestHousekeepingChecksDueEndpointsAndRaisesExpiryAlerts(t *testing.T) {
	f := newViewFixture(t)
	added := f.run(ViewEndpoints, "add", 0, map[string]string{"host": "app.fixture.test", "authorized": "true"})
	if done, err := f.app.Housekeep(f.ctx); err != nil || done.Report.Checked != 0 || len(f.server.dialed()) != 0 {
		t.Fatalf("automatic checks ran although the interval is off: %+v %v", done, err)
	}
	if _, err := f.app.Settings.Set(f.ctx, settings.KeyCheckInterval, "60"); err != nil {
		t.Fatal(err)
	}
	if err := f.app.Acknowledge(f.ctx); err != nil {
		t.Fatal(err)
	}
	done, err := f.app.Housekeep(f.ctx)
	if err != nil || done.Report.Checked != 1 || done.Report.Alerts != 1 {
		t.Fatalf("first run: %+v %v", done, err)
	}
	if change, _ := f.app.Poll(f.ctx); !change.Changed {
		t.Fatalf("the front ends are not told about the background check")
	}
	if done, err := f.app.Housekeep(f.ctx); err != nil || done.Report.Checked != 0 {
		t.Fatalf("the endpoint was checked again before the interval passed: %+v %v", done, err)
	}
	f.s.clock.Advance(61 * time.Minute)
	if done, err := f.app.Housekeep(f.ctx); err != nil || done.Report.Checked != 1 {
		t.Fatalf("after the interval: %+v %v", done, err)
	}
	f.run(ViewEndpoints, "disable", added.Select, nil)
	f.s.clock.Advance(2 * time.Hour)
	if done, err := f.app.Housekeep(f.ctx); err != nil || done.Report.Checked != 0 {
		t.Fatalf("a disabled endpoint was checked: %+v %v", done, err)
	}
	if len(f.server.dialed()) != 2 {
		t.Fatalf("connections: %v", f.server.dialed())
	}
	reports := make(chan Housekeeping, 4)
	f.run(ViewEndpoints, "enable", added.Select, nil)
	stop := f.app.StartScheduler(f.ctx, time.Hour, func(done Housekeeping, err error) {
		if err == nil {
			reports <- done
		}
	})
	select {
	case done := <-reports:
		if done.Report.Checked != 1 {
			t.Fatalf("scheduler: %+v", done)
		}
	case <-time.After(10 * time.Second):
		t.Fatal("the scheduler did not run")
	}
	stop()
}

func TestAdditionalTrustedRootsComeFromTheConfiguredFile(t *testing.T) {
	s := newSandbox(t)
	s.clock.Set(time.Now().UTC().Truncate(time.Second))
	root, err := certgen.NewAuthority("Private Root", s.clock.Now().AddDate(-1, 0, 0), s.clock.Now().AddDate(5, 0, 0))
	if err != nil {
		t.Fatal(err)
	}
	server := startServer(t, root, s.clock.Now(), "app.fixture.test", 200)
	opened := s.open(t, func(options *Options) { options.Dial = server.dial })
	ctx := context.Background()
	created, err := opened.Inventory.CreateEndpoint(ctx, inventory.EndpointInput{Host: "app.fixture.test", Enabled: true}, true)
	if err != nil {
		t.Fatal(err)
	}
	check := func() string {
		t.Helper()
		outcome, err := opened.Run(ctx, Request{View: ViewEndpoints, Action: "check", ID: created.ID})
		if err != nil {
			return "error: " + err.Error()
		}
		return outcome.Lines[0]
	}
	if line := check(); !strings.Contains(line, "chain not trusted") {
		t.Fatalf("without the root: %s", line)
	}
	path := filepath.Join(t.TempDir(), "roots.pem")
	if err := os.WriteFile(path, certgen.PEM(root.Certificate), 0o600); err != nil {
		t.Fatal(err)
	}
	if _, err := opened.Settings.Set(ctx, settings.KeyTrustFile, path); err != nil {
		t.Fatal(err)
	}
	if line := check(); !strings.Contains(line, ": ok") {
		t.Fatalf("with the root: %s", line)
	}
	if _, err := opened.Settings.Set(ctx, settings.KeyTrustFile, filepath.Join(t.TempDir(), "missing.pem")); err != nil {
		t.Fatal(err)
	}
	if line := check(); !strings.Contains(line, "additional trusted roots cannot be read") || !strings.Contains(line, settings.KeyTrustFile) {
		t.Fatalf("with a missing file: %s", line)
	}
}

func TestSplitMarkUndoesMarked(t *testing.T) {
	for _, severity := range theme.Severities {
		got, text := SplitMark(marked(severity, "5 days"))
		want := severity
		if severity == theme.SeverityInfo {
			want = ""
		}
		if got != want || text != "5 days" {
			t.Fatalf("%s: %q %q", severity, got, text)
		}
	}
	if severity, text := SplitMark("- "); severity != "" || text != "- " {
		t.Fatalf("a dash cell was read as a mark: %q %q", severity, text)
	}
	if severity, text := SplitMark("-"); severity != "" || text != "-" {
		t.Fatalf("a dash cell was read as a mark: %q %q", severity, text)
	}
}
