package inventory

import (
	"bytes"
	"context"
	"crypto/x509"
	"encoding/json"
	"errors"
	"fmt"
	"strings"
	"sync"
	"testing"
	"time"

	"github.com/NexzaDev/certwatch/internal/certgen"
	"github.com/NexzaDev/certwatch/internal/notify"
	"github.com/NexzaDev/certwatch/internal/testkit"
	"github.com/NexzaDev/certwatch/internal/theme"
	"github.com/NexzaDev/certwatch/internal/tlscheck"
)

type fixture struct {
	t       *testing.T
	ctx     context.Context
	clock   *testkit.Clock
	service *Service
	alerts  *notify.Service
	root    *certgen.Authority
	issuing *certgen.Authority
}

func newFixture(t *testing.T) *fixture {
	t.Helper()
	db := testkit.Open(t)
	clock := testkit.NewClock()
	now := clock.Now()
	root, err := certgen.NewAuthority("Fixture Root", now.AddDate(-1, 0, 0), now.AddDate(9, 0, 0))
	if err != nil {
		t.Fatal(err)
	}
	issuing, err := root.Intermediate("Fixture Issuing", now.AddDate(-1, 0, 0), now.AddDate(5, 0, 0))
	if err != nil {
		t.Fatal(err)
	}
	return &fixture{t: t, ctx: context.Background(), clock: clock, service: New(db.DB(), clock.Now), alerts: notify.New(db.DB(), clock.Now), root: root, issuing: issuing}
}

func (f *fixture) endpoint(host string) Endpoint {
	f.t.Helper()
	endpoint, err := f.service.CreateEndpoint(f.ctx, EndpointInput{Host: host, Enabled: true}, true)
	if err != nil {
		f.t.Fatal(err)
	}
	return endpoint
}

func (f *fixture) leaf(name string, days int) *x509.Certificate {
	f.t.Helper()
	now := f.clock.Now()
	issued, err := f.issuing.Issue(certgen.Request{CommonName: name, Names: []string{name}, NotBefore: now.AddDate(0, 0, -30), NotAfter: now.AddDate(0, 0, days).Add(time.Hour)})
	if err != nil {
		f.t.Fatal(err)
	}
	return issued.Certificate
}

func (f *fixture) ok(endpoint Endpoint, leaf *x509.Certificate) tlscheck.Result {
	return tlscheck.Result{Target: tlscheck.Target{Host: endpoint.Host, Port: endpoint.Port}, ObservedAt: f.clock.Now(), Outcome: tlscheck.OutcomeOK,
		Presented: []*x509.Certificate{leaf, f.issuing.Certificate}, Verified: []*x509.Certificate{leaf, f.issuing.Certificate, f.root.Certificate},
		ChainStatus: tlscheck.StatusValid, HostnameStatus: tlscheck.HostnameMatch, Version: "TLS 1.3", CipherSuite: "TLS_AES_128_GCM_SHA256"}
}

func (f *fixture) record(endpoint Endpoint, result tlscheck.Result) Observation {
	f.t.Helper()
	observation, err := f.service.Record(f.ctx, endpoint.ID, result, Policy{})
	if err != nil {
		f.t.Fatal(err)
	}
	return observation
}

func (f *fixture) titles() []string {
	f.t.Helper()
	list, err := f.alerts.List(f.ctx, notify.Filter{Limit: 100})
	if err != nil {
		f.t.Fatal(err)
	}
	var titles []string
	for i := len(list) - 1; i >= 0; i-- {
		titles = append(titles, list[i].Title)
	}
	return titles
}

func TestParseTarget(t *testing.T) {
	good := map[string]string{
		"www.example.org":                  "www.example.org 0",
		"  WWW.Example.ORG.  ":             "www.example.org 0",
		"www.example.org:8443":             "www.example.org 8443",
		"https://www.example.org/path?x=1": "www.example.org 0",
		"https://www.example.org:444/":     "www.example.org 444",
		"192.0.2.10:993":                   "192.0.2.10 993",
		"[2001:db8::1]:443":                "2001:db8::1 443",
		"2001:db8::1":                      "2001:db8::1 0",
		"xn--bcher-kva.example":            "xn--bcher-kva.example 0",
		"localhost":                        "localhost 0",
	}
	for input, want := range good {
		host, port, err := ParseTarget(input)
		if err != nil {
			t.Errorf("ParseTarget(%q) failed: %v", input, err)
			continue
		}
		if got := fmt.Sprintf("%s %d", host, port); got != want {
			t.Errorf("ParseTarget(%q) = %q %d, want %s", input, host, port, want)
		}
	}
	bad := map[string]string{
		"":                                "empty",
		"   ":                             "empty",
		"*.example.org":                   "not a host name",
		"exa mple.org":                    "not a host name",
		"-bad.example.org":                "not a host name",
		"bücher.example":                  "punycode",
		"www.example.org:0":               "not a port",
		"www.example.org:abc":             "not a port",
		"www.example.org:":                "not a port",
		"www..example.org":                "not a host name",
		strings.Repeat("a.", 140) + "org": "longer than",
	}
	for input, want := range bad {
		_, _, err := ParseTarget(input)
		var validation *ValidationError
		if !errors.As(err, &validation) || !strings.Contains(err.Error(), want) {
			t.Errorf("ParseTarget(%q) error = %v, want %q", input, err, want)
		}
	}
}

func FuzzParseTarget(f *testing.F) {
	for _, seed := range []string{"www.example.org", "https://www.example.org:8443/x", "[::1]:443", "a:b:c", "\x00", "xn--a.example:1"} {
		f.Add(seed)
	}
	f.Fuzz(func(t *testing.T, input string) {
		host, port, err := ParseTarget(input)
		if err != nil {
			return
		}
		if host == "" || len(host) > MaxHostLength || port < 0 || port > 65535 || strings.ContainsAny(host, " /\t\n*") {
			t.Fatalf("ParseTarget(%q) accepted host %q port %d", input, host, port)
		}
		again, _, err := ParseTarget(host)
		if err != nil || again != host {
			t.Fatalf("the accepted host %q is not stable: %q %v", host, again, err)
		}
	})
}

func TestEndpointLifecycleNeedsAuthorization(t *testing.T) {
	f := newFixture(t)
	if _, err := f.service.CreateEndpoint(f.ctx, EndpointInput{Host: "www.example.org", Enabled: true}, false); !errors.Is(err, ErrNotAuthorized) {
		t.Fatalf("an endpoint was created without authorization: %v", err)
	}
	created, err := f.service.CreateEndpoint(f.ctx, EndpointInput{Host: "https://WWW.example.org:8443/", ServerName: "www.example.org", Note: " edge ", Enabled: true}, true)
	if err != nil {
		t.Fatal(err)
	}
	if created.Address() != "www.example.org:8443" || created.ServerName != "" || created.Note != "edge" || created.Outcome != "never" || created.AuthorizedAt.IsZero() {
		t.Fatalf("unexpected endpoint: %+v", created)
	}
	if _, err := f.service.CreateEndpoint(f.ctx, EndpointInput{Host: "www.example.org", Port: 8443, Enabled: true}, true); err == nil || !strings.Contains(err.Error(), "already monitored") {
		t.Fatalf("a duplicate was accepted: %v", err)
	}
	noted, err := f.service.CreateEndpoint(f.ctx, EndpointInput{Host: "notes.example.org", Note: "first\nsecond\x1b[2J\u202e  end", Enabled: true}, true)
	if err != nil {
		t.Fatal(err)
	}
	if noted.Note != "first second\ufffd[2J\ufffd end" {
		t.Fatalf("control characters reached the stored note: %q", noted.Note)
	}
	if err := f.service.DeleteEndpoint(f.ctx, noted.ID); err != nil {
		t.Fatal(err)
	}
	in := created.Input()
	in.Note = "changed"
	updated, err := f.service.UpdateEndpoint(f.ctx, created.ID, created.Revision, in, false)
	if err != nil || updated.Note != "changed" || updated.Revision != created.Revision+1 {
		t.Fatalf("update failed: %+v %v", updated, err)
	}
	var conflict *ConflictError
	if _, err := f.service.UpdateEndpoint(f.ctx, created.ID, created.Revision, in, false); !errors.As(err, &conflict) {
		t.Fatalf("a stale revision was accepted: %v", err)
	}
	in.Host = "other.example.org"
	if _, err := f.service.UpdateEndpoint(f.ctx, created.ID, updated.Revision, in, false); !errors.Is(err, ErrNotAuthorized) {
		t.Fatalf("the host was changed without authorization: %v", err)
	}
	moved, err := f.service.UpdateEndpoint(f.ctx, created.ID, updated.Revision, in, true)
	if err != nil || moved.Host != "other.example.org" {
		t.Fatalf("authorized host change failed: %+v %v", moved, err)
	}
	disabled, err := f.service.SetEndpointEnabled(f.ctx, created.ID, false)
	if err != nil || disabled.Enabled {
		t.Fatalf("disable failed: %+v %v", disabled, err)
	}
	if list, err := f.service.Endpoints(f.ctx, Query{Filter: "enabled"}); err != nil || len(list) != 0 {
		t.Fatalf("the disabled endpoint is listed as enabled: %v %v", list, err)
	}
	if list, err := f.service.Endpoints(f.ctx, Query{Search: "OTHER"}); err != nil || len(list) != 1 {
		t.Fatalf("search failed: %v %v", list, err)
	}
	if _, err := f.service.Endpoints(f.ctx, Query{Filter: "nonsense"}); err == nil || !strings.Contains(err.Error(), "unknown filter") {
		t.Fatalf("an unknown filter was accepted: %v", err)
	}
	if err := f.service.DeleteEndpoint(f.ctx, created.ID); err != nil {
		t.Fatal(err)
	}
	if err := f.service.DeleteEndpoint(f.ctx, created.ID); !errors.Is(err, ErrNotFound) {
		t.Fatalf("deleting twice: %v", err)
	}
}

func TestRecordStoresTheChainAndAlertsOnceForEachThreshold(t *testing.T) {
	f := newFixture(t)
	endpoint := f.endpoint("app.example.org")
	leaf := f.leaf("app.example.org", 20)
	first := f.record(endpoint, f.ok(endpoint, leaf))
	if first.Changed || first.CertificateID == 0 || first.Alerts != 1 || len(first.Findings) != 0 {
		t.Fatalf("unexpected observation: %+v", first)
	}
	chain, err := f.service.Chain(f.ctx, first.ID)
	if err != nil || len(chain) != 3 {
		t.Fatalf("chain: %v %v", chain, err)
	}
	if chain[0].Role() != "leaf" || chain[1].Role() != "intermediate" || chain[2].Role() != "root" || chain[2].Origin != OriginTrustStore || chain[1].Origin != OriginServer {
		t.Fatalf("unexpected chain roles: %+v", chain)
	}
	f.clock.Advance(2 * time.Hour)
	second := f.record(endpoint, f.ok(endpoint, leaf))
	if second.Alerts != 0 || second.Changed {
		t.Fatalf("the same threshold alerted twice: %+v", second)
	}
	f.clock.Advance(7 * 24 * time.Hour)
	if count, err := f.service.EvaluateExpiry(f.ctx, Policy{}); err != nil || count != 1 {
		t.Fatalf("crossing the 14 day threshold: %d %v", count, err)
	}
	f.clock.Advance(14 * 24 * time.Hour)
	if count, err := f.service.EvaluateExpiry(f.ctx, Policy{}); err != nil || count != 1 {
		t.Fatalf("expiry: %d %v", count, err)
	}
	if count, err := f.service.EvaluateExpiry(f.ctx, Policy{}); err != nil || count != 0 {
		t.Fatalf("repeated evaluation: %d %v", count, err)
	}
	want := []string{"Certificate for app.example.org expires within 30 days", "Certificate for app.example.org expires within 14 days", "Certificate for app.example.org has expired"}
	if got := f.titles(); strings.Join(got, "|") != strings.Join(want, "|") {
		t.Fatalf("alerts:\n%s", strings.Join(got, "\n"))
	}
	reloaded, err := f.service.Endpoint(f.ctx, endpoint.ID)
	if err != nil || reloaded.ObservationID != second.ID || reloaded.CertificateName != "app.example.org" || reloaded.Version != "TLS 1.3" {
		t.Fatalf("endpoint status: %+v %v", reloaded, err)
	}
}

func TestRecordDetectsChangesAndValidationTransitions(t *testing.T) {
	f := newFixture(t)
	endpoint := f.endpoint("app.example.org")
	old := f.leaf("app.example.org", 200)
	f.record(endpoint, f.ok(endpoint, old))
	f.clock.Advance(time.Hour)
	failed := tlscheck.Result{Target: tlscheck.Target{Host: endpoint.Host, Port: 443}, ObservedAt: f.clock.Now(), Outcome: tlscheck.OutcomeUnreachable, Error: "no answer", ChainStatus: tlscheck.StatusUnknown, HostnameStatus: tlscheck.StatusUnknown}
	if observation := f.record(endpoint, failed); observation.Changed || observation.CertificateID != 0 {
		t.Fatalf("a failed check counted as a change: %+v", observation)
	}
	f.clock.Advance(time.Hour)
	fresh := f.leaf("other.example.org", 300)
	result := f.ok(endpoint, fresh)
	result.Verified = nil
	result.ChainStatus, result.ChainError = tlscheck.StatusInvalid, "the chain does not lead to a trusted root certificate"
	result.HostnameStatus, result.HostnameError = tlscheck.HostnameMismatch, "not valid for app.example.org"
	changed := f.record(endpoint, result)
	if !changed.Changed || changed.Alerts != 3 {
		t.Fatalf("unexpected observation: %+v", changed)
	}
	f.clock.Advance(2 * time.Hour)
	if again := f.record(endpoint, result); again.Changed || again.Alerts != 0 {
		t.Fatalf("a repeated failure alerted again: %+v", again)
	}
	want := "Chain validation failed for app.example.org:443|Host name validation failed for app.example.org:443|Certificate changed on app.example.org:443"
	if got := strings.Join(f.titles(), "|"); got != want {
		t.Fatalf("alerts: %s", got)
	}
	changes, err := f.service.EndpointChanges(f.ctx, endpoint.ID, 10)
	if err != nil || len(changes) != 1 || changes[0].FromName != "app.example.org" || changes[0].ToName != "other.example.org" {
		t.Fatalf("changes: %+v %v", changes, err)
	}
	for _, id := range []int64{changes[0].FromID, changes[0].ToID} {
		list, err := f.service.CertificateChanges(f.ctx, id, 10)
		if err != nil || len(list) != 1 {
			t.Fatalf("certificate %d changes: %+v %v", id, list, err)
		}
	}
	history, total, err := f.service.History(f.ctx, endpoint.ID, 2)
	if err != nil || total != 4 || len(history) != 2 || history[0].ChainStatus != tlscheck.StatusInvalid {
		t.Fatalf("history: %+v %d %v", history, total, err)
	}
	usage, err := f.service.Usage(f.ctx, changes[0].ToID)
	if err != nil || len(usage) != 1 || usage[0].Count != 2 || !usage[0].Current {
		t.Fatalf("usage: %+v %v", usage, err)
	}
	old1, err := f.service.Certificate(f.ctx, changes[0].FromID)
	if err != nil || old1.Tracked {
		t.Fatalf("the replaced certificate is still tracked: %+v %v", old1, err)
	}
	overview, err := f.service.Overview(f.ctx)
	if err != nil || overview.ChainFailures != 1 || overview.HostnameMismatches != 1 || overview.Certificates != 1 || overview.Findings != 2 {
		t.Fatalf("overview: %+v %v", overview, err)
	}
	findings, err := f.service.Findings(f.ctx, endpoint.ID)
	if err != nil || len(findings) != 2 || findings[0].Severity != theme.SeverityHigh {
		t.Fatalf("findings: %+v %v", findings, err)
	}
	stats, err := f.service.FindingStats(f.ctx)
	if err != nil || stats[endpoint.ID].Count != 2 || stats[endpoint.ID].Worst != theme.SeverityHigh {
		t.Fatalf("finding stats: %+v %v", stats, err)
	}
}

func TestRecordPrunesHistoryAndRemovesUnreferencedCertificates(t *testing.T) {
	f := newFixture(t)
	endpoint := f.endpoint("app.example.org")
	for i := 0; i < 4; i++ {
		f.clock.Advance(time.Hour)
		if _, err := f.service.Record(f.ctx, endpoint.ID, f.ok(endpoint, f.leaf("app.example.org", 200+i)), Policy{Keep: 2}); err != nil {
			t.Fatal(err)
		}
	}
	_, total, err := f.service.History(f.ctx, endpoint.ID, 10)
	if err != nil || total != 2 {
		t.Fatalf("history holds %d observations, %v", total, err)
	}
	all, err := f.service.Certificates(f.ctx, Query{Filter: "all"})
	if err != nil || len(all) != 5 {
		t.Fatalf("expected three leaves, the issuing and the root certificate, got %d %v", len(all), err)
	}
	if err := f.service.DeleteEndpoint(f.ctx, endpoint.ID); err != nil {
		t.Fatal(err)
	}
	if all, err = f.service.Certificates(f.ctx, Query{Filter: "all"}); err != nil || len(all) != 0 {
		t.Fatalf("certificates left after the endpoint was removed: %d %v", len(all), err)
	}
}

func TestDisabledEndpointsAreLeftOutOfCountsAndAlerts(t *testing.T) {
	f := newFixture(t)
	endpoint := f.endpoint("app.example.org")
	if _, err := f.service.SetEndpointEnabled(f.ctx, endpoint.ID, false); err != nil {
		t.Fatal(err)
	}
	observation := f.record(endpoint, f.ok(endpoint, f.leaf("app.example.org", 3)))
	if observation.Alerts != 0 {
		t.Fatalf("a disabled endpoint raised alerts: %+v", observation)
	}
	overview, err := f.service.Overview(f.ctx)
	if err != nil || overview.Within7 != 0 || overview.Certificates != 0 || overview.DisabledEndpoints != 1 {
		t.Fatalf("overview: %+v %v", overview, err)
	}
	due, err := f.service.DueEndpoints(f.ctx, time.Hour)
	if err != nil || len(due) != 0 {
		t.Fatalf("a disabled endpoint is due: %v %v", due, err)
	}
}

func TestOverviewBucketsMatchTheCertificateFilters(t *testing.T) {
	f := newFixture(t)
	for name, days := range map[string]int{"a.example.org": -2, "b.example.org": 3, "c.example.org": 20, "d.example.org": 60, "e.example.org": 300} {
		endpoint := f.endpoint(name)
		f.record(endpoint, f.ok(endpoint, f.leaf(name, days)))
	}
	overview, err := f.service.Overview(f.ctx)
	if err != nil {
		t.Fatal(err)
	}
	if overview.Expired != 1 || overview.Within7 != 1 || overview.Within30 != 2 || overview.Within90 != 3 || overview.Certificates != 5 || overview.Endpoints != 5 {
		t.Fatalf("overview: %+v", overview)
	}
	counts := map[string]int{"expired": overview.Expired, "expiring-7": overview.Within7, "expiring-30": overview.Within30, "expiring-90": overview.Within90, "tracked": overview.Certificates, "authorities": 2, "all": 7}
	for filter, want := range counts {
		list, err := f.service.Certificates(f.ctx, Query{Filter: filter})
		if err != nil || len(list) != want {
			t.Errorf("filter %s lists %d certificates, the overview says %d (%v)", filter, len(list), want, err)
		}
	}
	list, err := f.service.Certificates(f.ctx, Query{Search: "C.EXAMPLE"})
	if err != nil || len(list) != 1 || list[0].UsedBy != "c.example.org:443" || list[0].EndpointCount != 1 || list[0].DaysLeft(f.clock.Now()) != 20 {
		t.Fatalf("search: %+v %v", list, err)
	}
	due, err := f.service.DueEndpoints(f.ctx, time.Hour)
	if err != nil || len(due) != 0 {
		t.Fatalf("freshly checked endpoints are due: %d %v", len(due), err)
	}
	f.clock.Advance(2 * time.Hour)
	if due, err = f.service.DueEndpoints(f.ctx, time.Hour); err != nil || len(due) != 5 {
		t.Fatalf("due endpoints: %d %v", len(due), err)
	}
}

func TestExpiryState(t *testing.T) {
	now := time.Date(2026, 3, 1, 12, 0, 0, 0, time.UTC)
	cases := []struct {
		days     float64
		state    string
		severity theme.Severity
	}{{-1, "expired", theme.SeverityCritical}, {0, "expired", theme.SeverityCritical}, {0.5, "expiring", theme.SeverityHigh}, {6.9, "expiring", theme.SeverityHigh}, {7.5, "expiring", theme.SeverityMedium}, {45, "expiring", theme.SeverityLow}, {91, "valid", theme.SeverityInfo}}
	for _, test := range cases {
		state, severity := ExpiryState(now.Add(time.Duration(test.days*24)*time.Hour), now)
		if state != test.state || severity != test.severity {
			t.Errorf("%v days: %s %s, want %s %s", test.days, state, severity, test.state, test.severity)
		}
	}
}

func TestImportCertificatesTracksFilesAndStopTrackingReleasesThem(t *testing.T) {
	f := newFixture(t)
	bundle := certgen.PEM(f.leaf("file.example.org", 10), f.issuing.Certificate)
	preview, err := f.service.ImportCertificates(f.ctx, bundle, true, Policy{})
	if err != nil || preview.Added != 2 || preview.Summary() != "would import 2 certificates, 0 already tracked" {
		t.Fatalf("preview: %+v %v", preview, err)
	}
	if list, _ := f.service.Certificates(f.ctx, Query{Filter: "all"}); len(list) != 0 {
		t.Fatalf("the preview stored %d certificates", len(list))
	}
	imported, err := f.service.ImportCertificates(f.ctx, bundle, false, Policy{})
	if err != nil || imported.Added != 2 || imported.Alerts != 1 {
		t.Fatalf("import: %+v %v", imported, err)
	}
	again, err := f.service.ImportCertificates(f.ctx, bundle, false, Policy{})
	if err != nil || again.Added != 0 || again.Known != 2 {
		t.Fatalf("second import: %+v %v", again, err)
	}
	if _, err := f.service.ImportCertificates(f.ctx, []byte("not pem"), false, Policy{}); err == nil {
		t.Fatalf("text without certificates was imported")
	}
	tracked, err := f.service.Certificates(f.ctx, Query{})
	if err != nil || len(tracked) != 2 || tracked[0].Source != SourceImported || tracked[0].Name != "file.example.org" {
		t.Fatalf("tracked: %+v %v", tracked, err)
	}
	detail, err := f.service.Certificate(f.ctx, tracked[0].ID)
	if err != nil || !strings.HasPrefix(detail.PEM, "-----BEGIN CERTIFICATE-----") || len(detail.Names) != 1 {
		t.Fatalf("detail: %+v %v", detail, err)
	}
	if err := f.service.StopTracking(f.ctx, tracked[0].ID); err != nil {
		t.Fatal(err)
	}
	if _, err := f.service.Certificate(f.ctx, tracked[0].ID); !errors.Is(err, ErrNotFound) {
		t.Fatalf("the released certificate is still stored: %v", err)
	}
	endpoint := f.endpoint("app.example.org")
	observation := f.record(endpoint, f.ok(endpoint, f.leaf("app.example.org", 200)))
	if err := f.service.StopTracking(f.ctx, observation.CertificateID); err == nil || !strings.Contains(err.Error(), "not imported") {
		t.Fatalf("an observed certificate was released: %v", err)
	}
	if err := f.service.StopTracking(f.ctx, 9999); !errors.Is(err, ErrNotFound) {
		t.Fatalf("unknown certificate: %v", err)
	}
}

func TestReportsRenderTheSameRowsInEveryFormat(t *testing.T) {
	f := newFixture(t)
	endpoint := f.endpoint("app.example.org")
	result := f.ok(endpoint, f.leaf("app.example.org", 12))
	result.HostnameStatus, result.HostnameError = tlscheck.HostnameMismatch, "not valid"
	f.record(endpoint, result)
	for _, kind := range ReportKinds {
		for _, format := range ReportFormats {
			report, err := f.service.GenerateReport(f.ctx, kind, format)
			if err != nil || report.ID == 0 || report.Rows != 1 || len(report.SHA256) != 64 || report.IncludesDemo {
				t.Fatalf("%s %s: %+v %v", kind, format, report, err)
			}
			stored, err := f.service.Report(f.ctx, report.ID)
			if err != nil || stored.Content != report.Content || stored.Size != len(report.Content) {
				t.Fatalf("%s %s was stored differently: %v", kind, format, err)
			}
			if !strings.Contains(report.Content, "app.example.org") {
				t.Fatalf("%s %s lacks the row:\n%s", kind, format, report.Content)
			}
			if format == FormatJSON {
				var document struct {
					Rows []map[string]string `json:"rows"`
				}
				if err := json.Unmarshal([]byte(report.Content), &document); err != nil || len(document.Rows) != 1 {
					t.Fatalf("%s json: %v", kind, err)
				}
			}
		}
	}
	list, err := f.service.Reports(f.ctx, Query{Filter: ReportExpiry})
	if err != nil || len(list) != 3 || list[0].Content != "" || list[0].Size == 0 {
		t.Fatalf("list: %+v %v", list, err)
	}
	if !strings.HasPrefix(list[0].FileName(), "certwatch-expiry-") || !strings.HasSuffix(list[0].FileName(), ".json") {
		t.Fatalf("file name %q", list[0].FileName())
	}
	if _, err := f.service.GenerateReport(f.ctx, "nonsense", FormatCSV); err == nil {
		t.Fatalf("an unknown kind was accepted")
	}
	if _, err := f.service.GenerateReport(f.ctx, ReportExpiry, "pdf"); err == nil {
		t.Fatalf("an unknown format was accepted")
	}
	if err := f.service.DeleteReport(f.ctx, list[0].ID); err != nil {
		t.Fatal(err)
	}
	if err := f.service.DeleteReport(f.ctx, list[0].ID); !errors.Is(err, ErrNotFound) {
		t.Fatalf("deleting twice: %v", err)
	}
}

func TestRenderGuardsSpreadsheetFormulasAndTableSyntax(t *testing.T) {
	now := time.Date(2026, 1, 2, 3, 4, 5, 0, time.UTC)
	csvText, err := Render(FormatCSV, "T", now, false, []string{"A", "B"}, [][]string{{"=1+1", "plain"}})
	if err != nil || !strings.Contains(csvText, "'=1+1,plain") {
		t.Fatalf("csv: %q %v", csvText, err)
	}
	markdown, err := Render(FormatMarkdown, "T", now, true, []string{"A", "B"}, [][]string{{"a|b", ""}})
	if err != nil || !strings.Contains(markdown, `| a\|b | - |`) || !strings.Contains(markdown, "includes demo data") || !strings.Contains(markdown, "2026-01-02 03:04 UTC") {
		t.Fatalf("markdown: %q %v", markdown, err)
	}
	empty, err := Render(FormatMarkdown, "T", now, false, []string{"A"}, nil)
	if err != nil || !strings.Contains(empty, "Nothing to report.") {
		t.Fatalf("empty: %q %v", empty, err)
	}
}

func TestEndpointTransferRoundTripAndImportRules(t *testing.T) {
	f := newFixture(t)
	f.endpoint("a.example.org")
	if _, err := f.service.CreateEndpoint(f.ctx, EndpointInput{Host: "b.example.org", Port: 8443, ServerName: "www.example.org", Note: "=cmd", Enabled: false}, true); err != nil {
		t.Fatal(err)
	}
	list, err := f.service.Endpoints(f.ctx, Query{})
	if err != nil {
		t.Fatal(err)
	}
	for _, format := range []string{FormatCSV, FormatJSON} {
		var out bytes.Buffer
		if err := ExportEndpoints(&out, format, list); err != nil {
			t.Fatal(err)
		}
		if format == FormatCSV && !strings.Contains(out.String(), "'=cmd") {
			t.Fatalf("the note was not guarded: %s", out.String())
		}
		inputs, problems, err := ParseEndpoints(bytes.NewReader(out.Bytes()), format)
		if err != nil || len(problems) != 0 || len(inputs) != 2 {
			t.Fatalf("%s: %v %v %v", format, inputs, problems, err)
		}
		if inputs[1] != (EndpointInput{Host: "b.example.org", Port: 8443, ServerName: "www.example.org", Note: "=cmd", Enabled: false}) {
			t.Fatalf("%s round trip: %+v", format, inputs[1])
		}
	}
	file := "host,port,note\nc.example.org,,first\nA.example.org,443,known\nbad host,,x\nd.example.org,99999,\nc.example.org,443,twice\n"
	if _, err := f.service.ImportEndpoints(f.ctx, strings.NewReader(file), FormatCSV, false, false); !errors.Is(err, ErrNotAuthorized) {
		t.Fatalf("import without authorization: %v", err)
	}
	preview, err := f.service.ImportEndpoints(f.ctx, strings.NewReader(file), FormatCSV, true, false)
	if err != nil || preview.Summary() != "would import 1 endpoint, 2 already monitored, 2 rejected" {
		t.Fatalf("preview: %+v %v", preview, err)
	}
	if count, _ := f.service.EndpointCount(f.ctx); count != 2 {
		t.Fatalf("the preview created endpoints: %d", count)
	}
	result, err := f.service.ImportEndpoints(f.ctx, strings.NewReader(file), FormatCSV, false, true)
	if err != nil || result.Created != 1 || result.Known != 2 || len(result.Errors) != 2 || result.Errors[0].Row != 4 {
		t.Fatalf("import: %+v %v", result, err)
	}
	for input, want := range map[string]string{"": "empty", "name\nx\n": "no host column", "\"": "not valid CSV"} {
		if _, _, err := ParseEndpoints(strings.NewReader(input), FormatCSV); err == nil || !strings.Contains(err.Error(), want) {
			t.Errorf("ParseEndpoints(%q) error = %v, want %q", input, err, want)
		}
	}
	if _, _, err := ParseEndpoints(strings.NewReader(`[{"host":"x.example.org","extra":1}]`), FormatJSON); err == nil {
		t.Errorf("unknown JSON fields were accepted")
	}
	if _, _, err := ParseEndpoints(strings.NewReader("x"), "xml"); err == nil {
		t.Errorf("an unknown format was accepted")
	}
}

func FuzzParseEndpoints(f *testing.F) {
	f.Add("host,port,server_name,note,enabled\nwww.example.org,443,,note,true\n", true)
	f.Add(`[{"host":"www.example.org","port":443}]`, false)
	f.Add("host\n\"unterminated", true)
	f.Fuzz(func(t *testing.T, data string, asCSV bool) {
		format := FormatJSON
		if asCSV {
			format = FormatCSV
		}
		inputs, _, err := ParseEndpoints(strings.NewReader(data), format)
		if err != nil {
			return
		}
		for _, in := range inputs {
			if in.Host == "" || in.Port < 1 || in.Port > 65535 || len(in.Note) > MaxNoteLength {
				t.Fatalf("an invalid endpoint was accepted: %+v", in)
			}
		}
	})
}

func TestDemoDataIsLabeledAndRemovable(t *testing.T) {
	f := newFixture(t)
	real := f.endpoint("real.example.org")
	added, err := f.service.LoadDemo(f.ctx, Policy{})
	if err != nil || added != len(demoCases) {
		t.Fatalf("demo: %d %v", added, err)
	}
	if again, err := f.service.LoadDemo(f.ctx, Policy{}); err != nil || again != 0 {
		t.Fatalf("second load: %d %v", again, err)
	}
	overview, err := f.service.Overview(f.ctx)
	if err != nil {
		t.Fatal(err)
	}
	if overview.Expired != 1 || overview.Within7 != 1 || overview.Within30 != 2 || overview.Within90 != 5 || overview.ChainFailures != 3 || overview.HostnameMismatches != 1 || overview.FailedChecks != 1 || overview.DisabledEndpoints != 1 || overview.NeverChecked != 1 || overview.DemoRows == 0 {
		t.Fatalf("overview: %+v", overview)
	}
	endpoints, err := f.service.Endpoints(f.ctx, Query{})
	if err != nil {
		t.Fatal(err)
	}
	for _, endpoint := range endpoints {
		if endpoint.ID == real.ID {
			continue
		}
		if !endpoint.Demo || !strings.HasSuffix(endpoint.Host, ".demo.invalid") || !strings.HasPrefix(endpoint.Note, "Demo endpoint.") {
			t.Errorf("unlabeled demo endpoint: %+v", endpoint)
		}
	}
	for _, title := range f.titles() {
		if !strings.HasSuffix(title, "(demo)") {
			t.Errorf("unlabeled demo alert: %s", title)
		}
	}
	if len(f.titles()) < 6 {
		t.Fatalf("the demo raised %d alerts", len(f.titles()))
	}
	due, err := f.service.DueEndpoints(f.ctx, 0)
	if err != nil || len(due) != 1 || due[0].ID != real.ID {
		t.Fatalf("demo endpoints must never be due for a network check: %+v %v", due, err)
	}
	report, err := f.service.GenerateReport(f.ctx, ReportInventory, FormatMarkdown)
	if err != nil || !report.IncludesDemo || !strings.Contains(report.Content, "includes demo data") {
		t.Fatalf("report: %+v %v", report, err)
	}
	if count, err := f.service.RemoveDemo(f.ctx, true); err != nil || count == 0 {
		t.Fatalf("dry run: %d %v", count, err)
	}
	if count, _ := f.service.DemoCount(f.ctx); count == 0 {
		t.Fatalf("the dry run removed the demo data")
	}
	if _, err := f.service.RemoveDemo(f.ctx, false); err != nil {
		t.Fatal(err)
	}
	if count, _ := f.service.DemoCount(f.ctx); count != 0 {
		t.Fatalf("%d demo rows left", count)
	}
	if len(f.titles()) != 0 {
		t.Fatalf("demo alerts left: %v", f.titles())
	}
	if count, _ := f.service.EndpointCount(f.ctx); count != 1 {
		t.Fatalf("the real endpoint was removed with the demo data")
	}
}

func TestConcurrentWritersNeverCorruptData(t *testing.T) {
	f := newFixture(t)
	path := testkit.DatabasePath(t)
	first := New(testkit.OpenAt(t, path).DB(), f.clock.Now)
	second := New(testkit.OpenAt(t, path).DB(), f.clock.Now)
	shared, err := first.CreateEndpoint(f.ctx, EndpointInput{Host: "shared.example.org", Enabled: true}, true)
	if err != nil {
		t.Fatal(err)
	}
	leaves := []*x509.Certificate{f.leaf("shared.example.org", 40), f.leaf("shared.example.org", 80)}
	const steps = 30
	var wait sync.WaitGroup
	failures := make(chan error, 8*steps)
	wins := make(chan int, 2)
	for index, service := range []*Service{first, second} {
		wait.Add(1)
		go func(index int, service *Service) {
			defer wait.Done()
			won := 0
			for step := 0; step < steps; step++ {
				host := fmt.Sprintf("writer-%d-%d.example.org", index, step)
				if _, err := service.CreateEndpoint(f.ctx, EndpointInput{Host: host, Enabled: true}, true); err != nil {
					failures <- err
				}
				if _, err := service.Record(f.ctx, shared.ID, f.ok(shared, leaves[(index+step)%2]), Policy{Keep: 1000}); err != nil {
					failures <- err
				}
				current, err := service.Endpoint(f.ctx, shared.ID)
				if err != nil {
					failures <- err
					continue
				}
				change := current.Input()
				change.Note = host
				_, err = service.UpdateEndpoint(f.ctx, current.ID, current.Revision, change, false)
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
	final, err := first.Endpoint(f.ctx, shared.ID)
	if err != nil || total == 0 || final.Revision != int64(1+total) {
		t.Fatalf("revision %d does not match %d successful updates: %v", final.Revision, total, err)
	}
	if count, err := second.EndpointCount(f.ctx); err != nil || count != 1+2*steps {
		t.Fatalf("%d endpoints, want %d: %v", count, 1+2*steps, err)
	}
	if _, observed, err := first.History(f.ctx, shared.ID, 1000); err != nil || observed != 2*steps {
		t.Fatalf("%d observations, want %d: %v", observed, 2*steps, err)
	}
	found, err := second.Endpoints(f.ctx, Query{Search: "writer", Limit: 1000})
	if err != nil || len(found) != 1+2*steps {
		t.Fatalf("the search over hosts and notes finds %d of %d endpoints: %v", len(found), 1+2*steps, err)
	}
	if err := testkit.OpenAt(t, path).IntegrityCheck(f.ctx); err != nil {
		t.Fatalf("integrity check: %v", err)
	}
}
