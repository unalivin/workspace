package cli

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/unalivin/certwatch/internal/app"
	"github.com/unalivin/certwatch/internal/launcher"
	"github.com/unalivin/certwatch/internal/meta"
	"github.com/unalivin/certwatch/internal/notify"
	"github.com/unalivin/certwatch/internal/settings"
	"github.com/unalivin/certwatch/internal/testkit"
	"github.com/unalivin/certwatch/internal/userconfig"
)

type harness struct {
	t         *testing.T
	home      string
	env       map[string]string
	clock     *testkit.Clock
	session   launcher.Session
	stdin     string
	gui       FrontEnd
	tui       FrontEnd
	chooser   func(string) (launcher.PromptResult, error)
	window    func(string, []string) error
	delivered []string
	server    *testkit.TLSFixture
	sleeps    int
}

func (h *harness) live(name string, days int) *testkit.TLSFixture {
	h.t.Helper()
	h.clock.Set(time.Now().UTC().Truncate(time.Second))
	h.server = testkit.StartTLS(h.t, h.clock.Now(), name, days)
	return h.server
}

func newHarness(t *testing.T) *harness {
	t.Helper()
	h := &harness{t: t, home: t.TempDir(), env: map[string]string{}, clock: testkit.NewClock()}
	h.session = launcher.Session{GOOS: "linux", Getenv: func(key string) string { return h.env[key] }}
	return h
}

func (h *harness) Deliver(_ context.Context, title, body string) error {
	h.delivered = append(h.delivered, title+" | "+body)
	return nil
}

func (h *harness) run(arguments ...string) (string, string, int) {
	h.t.Helper()
	var stdout, stderr bytes.Buffer
	env := &Env{
		Args:               arguments,
		Stdin:              strings.NewReader(h.stdin),
		Stdout:             &stdout,
		Stderr:             &stderr,
		Environment:        userconfig.Environment{Getenv: func(key string) string { return h.env[key] }, HomeDir: func() (string, error) { return h.home, nil }, GOOS: "linux"},
		Now:                h.clock.Now,
		Session:            h.session,
		Executable:         filepath.Join(h.home, "bin", meta.Slug),
		OpenGUI:            h.gui,
		OpenTUI:            h.tui,
		ShowChooser:        h.chooser,
		OpenTerminalWindow: h.window,
		Deliverer:          h,
		Sleep: func(context.Context, time.Duration) bool {
			h.sleeps++
			h.clock.Advance(time.Hour)
			return h.sleeps < 2
		},
		Autostart: func() notify.Autostart {
			return notify.Autostart{GOOS: "linux", HomeDir: h.home, Executable: filepath.Join(h.home, "bin", meta.Slug)}
		},
	}
	if h.server != nil {
		env.Roots, env.Dial = h.server.Pool(), h.server.Dial
	}
	code := Run(context.Background(), env)
	return stdout.String(), stderr.String(), code
}

func (h *harness) ok(arguments ...string) string {
	h.t.Helper()
	stdout, stderr, code := h.run(arguments...)
	if code != ExitOK {
		h.t.Fatalf("%v exited with %d\nstdout:\n%s\nstderr:\n%s", arguments, code, stdout, stderr)
	}
	return stdout
}

func (h *harness) fails(code int, text string, arguments ...string) {
	h.t.Helper()
	stdout, stderr, got := h.run(arguments...)
	if got != code || !strings.Contains(stderr, text) {
		h.t.Fatalf("%v: exit %d, want %d with %q\nstdout:\n%s\nstderr:\n%s", arguments, got, code, text, stdout, stderr)
	}
}

func expect(t *testing.T, text string, parts ...string) {
	t.Helper()
	for _, part := range parts {
		if !strings.Contains(text, part) {
			t.Fatalf("output lacks %q:\n%s", part, text)
		}
	}
}

func TestHelpVersionAndUnknownCommands(t *testing.T) {
	h := newHarness(t)
	expect(t, h.ok("--help"), meta.AppName, "Commands:", "endpoints", "certificates", "timeline", "chains", "configuration", "alerts", "reports", "watch", "notifier", "Global flags:", "--ui <value>", "--no-prompt", meta.EnvName("ui"), "NO_COLOR")
	expect(t, h.ok("help"), "Commands:")
	expect(t, h.ok("help", "endpoints"), "The hosts you monitor", "import", "export", "check-all")
	expect(t, h.ok("endpoints", "remove", "--help"), "Remove the selected endpoint", "--dry-run", "--yes", "<id>")
	expect(t, h.ok("help", "config", "theme"), "list", "set", "show")
	if out := h.ok("--version"); out != meta.Slug+" "+meta.Version+"\n" {
		t.Fatalf("version: %q", out)
	}
	if out := h.ok("version"); out != meta.Slug+" "+meta.Version+"\n" {
		t.Fatalf("version: %q", out)
	}
	h.fails(ExitUsage, `unknown command "endpiont"`, "endpiont")
	h.fails(ExitUsage, `Did you mean "endpoints list"?`, "endpoints", "lis")
	h.fails(ExitUsage, "flag provided but not defined", "--bogus")
	h.fails(ExitUsage, "flag provided but not defined", "endpoints", "list", "--bogus")
	h.fails(ExitUsage, "unknown setting", "--set", "nope=1", "overview")
	h.fails(ExitUsage, "key=value", "--set", "novalue", "overview")
	_, stderr, code := h.run("endpoints")
	if code != ExitUsage || !strings.Contains(stderr, "Commands:") {
		t.Fatalf("a command group without a command must print its help: %d %s", code, stderr)
	}
	if _, err := os.Stat(filepath.Join(h.home, ".local")); !errors.Is(err, os.ErrNotExist) {
		t.Fatal("help and usage errors must not create a database")
	}
}

func TestEndpointLifecycleWithALiveCheck(t *testing.T) {
	h := newHarness(t)
	server := h.live("app.fixture.test", 20)
	expect(t, h.ok("endpoints", "list"), "No endpoints yet")
	h.fails(ExitUsage, "repeat the command with --authorized", "endpoints", "add", "--host", "app.fixture.test")
	h.fails(ExitUsage, "Host is missing", "endpoints", "add", "--authorized")
	h.fails(ExitUsage, "not a host name", "endpoints", "add", "--host", "bad host", "--authorized")
	expect(t, h.ok("endpoints", "add", "--host", "app.fixture.test:8443", "--note", "edge", "--authorized"), "Added endpoint 1: app.fixture.test:8443")
	h.fails(ExitUsage, "already monitored", "endpoints", "add", "--host", "app.fixture.test", "--port", "8443", "--authorized")
	expect(t, strings.Join(strings.Fields(h.ok("endpoints", "list")), " "), "ID ENDPOINT STATUS", "1 app.fixture.test:8443 not checked yet", "1 endpoint")
	h.fails(ExitUsage, "This needs confirmation: repeat the command with --yes", "endpoints", "check", "1")
	preview := h.ok("endpoints", "check", "1", "--dry-run")
	expect(t, preview, "Dry run: Check 1 endpoint over the network", "network: app.fixture.test:8443: name lookup for app.fixture.test, then one TCP connection with a TLS handshake", "Nothing was changed.")
	if len(server.Dialed()) != 0 {
		t.Fatalf("a refused or previewed check opened a connection: %v", server.Dialed())
	}
	stdout, stderr, code := h.run("endpoints", "check", "1", "--yes")
	if code != ExitOK || !strings.Contains(stderr, "network: app.fixture.test:8443") {
		t.Fatalf("check: %d\n%s\n%s", code, stdout, stderr)
	}
	expect(t, stdout, "Checked 1 endpoint: 0 failed, 0 changed certificates, 1 new alert.", "app.fixture.test:8443: ok (expires ")
	expect(t, strings.Join(strings.Fields(h.ok("endpoints", "list", "--filter", "enabled")), " "), "app.fixture.test 20", "20 days", "TLS 1.3 enabled")
	shown := h.ok("endpoints", "show", "1")
	expect(t, shown, "app.fixture.test:8443", "Authorization confirmed", "Findings (0)", "Chain", "Fixture Root", "Observations (latest 1 of 1)", "Note", "edge")
	var parsed map[string]any
	if err := json.Unmarshal([]byte(h.ok("endpoints", "show", "1", "--format", "json")), &parsed); err != nil || parsed["title"] != "app.fixture.test:8443" {
		t.Fatalf("json: %v %v", parsed, err)
	}
	if err := json.Unmarshal([]byte(h.ok("endpoints", "list", "--format", "json")), &parsed); err != nil || len(parsed["rows"].([]any)) != 1 {
		t.Fatalf("json list: %v %v", parsed, err)
	}
	expect(t, h.ok("endpoints", "list", "--format", "csv"), "id,endpoint,status", "1,app.fixture.test:8443")
	h.fails(ExitUsage, "unknown format", "endpoints", "list", "--format", "xml")
	h.fails(ExitUsage, "unknown filter", "endpoints", "list", "--filter", "nonsense")
	expect(t, h.ok("certificates", "list"), "app.fixture.test", "Fixture Root", "app.fixture.test:8443", "1 certificate")
	expect(t, h.ok("certificates", "show", "1"), "Subject alternative names (1)", "SHA-256", "PRESENTED NOW", "yes")
	expect(t, h.ok("timeline", "list", "--zoom", "30"), "█", "1 of 1 tracked certificate expires within 30 days")
	h.fails(ExitUsage, "unknown zoom", "timeline", "list", "--zoom", "7")
	expect(t, h.ok("chains", "list"), "valid", "system or added root")
	expect(t, h.ok("chains", "show", "1"), "leaf", "root", "trust store")
	expect(t, h.ok("configuration", "list"), "TLS 1.3", "not probed")
	expect(t, h.ok("configuration", "probe", "1", "--yes"), "Checked 1 endpoint")
	expect(t, h.ok("configuration", "list"), "TLS 1.2, TLS 1.3")
	expect(t, h.ok("endpoints", "edit", "1", "--note", "changed", "--server-name", "app.fixture.test"), "Saved endpoint 1")
	h.fails(ExitUsage, "repeat the command with --authorized", "endpoints", "edit", "1", "--host", "other.fixture.test")
	expect(t, h.ok("endpoints", "disable", "1"), "is now disabled")
	expect(t, h.ok("endpoints", "list", "--filter", "enabled"), "No endpoint matches")
	expect(t, h.ok("endpoints", "enable", "1"), "is now enabled")
	h.fails(ExitUsage, "give exactly one entry identifier", "endpoints", "show")
	h.fails(ExitUsage, "is not a entry identifier", "endpoints", "show", "abc")
	h.fails(ExitFailure, "no such entry", "endpoints", "show", "99")
	expect(t, h.ok("endpoints", "remove", "1", "--dry-run"), "Dry run: Remove endpoint app.fixture.test:8443 and its 2 observations.")
	h.fails(ExitUsage, "--yes", "endpoints", "remove", "1")
	h.session.StdinTerminal = true
	h.stdin = "no\n"
	h.fails(ExitFailure, "cancelled, nothing was changed", "endpoints", "remove", "1")
	h.stdin = "yes\n"
	expect(t, h.ok("endpoints", "remove", "1"), "Removed endpoint app.fixture.test:8443.")
	expect(t, h.ok("certificates", "list", "--filter", "all"), "No certificate matches")
}

func TestWatchChecksDueEndpointsOnlyAfterConfirmation(t *testing.T) {
	h := newHarness(t)
	server := h.live("app.fixture.test", 200)
	h.ok("endpoints", "add", "--host", "app.fixture.test", "--authorized")
	h.fails(ExitUsage, "automatic checks are off", "watch")
	h.fails(ExitUsage, "--yes", "watch", "--once")
	expect(t, h.ok("watch", "--once", "--dry-run"), "Dry run: Check the enabled endpoints that are due over the network once (1 enabled).", "network: app.fixture.test:443")
	if len(server.Dialed()) != 0 {
		t.Fatalf("connections before confirmation: %v", server.Dialed())
	}
	expect(t, h.ok("watch", "--once", "--yes"), "checked 1 endpoint: 0 failed", "app.fixture.test:443: ok")
	h.fails(ExitUsage, "1 to 10080", "watch", "--interval", "0")
	out := h.ok("watch", "--interval", "30", "--yes")
	expect(t, out, "checked 0 endpoints", "checked 1 endpoint", "Stopped.")
	if len(server.Dialed()) != 2 {
		t.Fatalf("connections: %v", server.Dialed())
	}
}

func TestCertificateImportReportsAndFiles(t *testing.T) {
	h := newHarness(t)
	h.clock.Set(time.Now().UTC().Truncate(time.Second))
	pem := testkit.PEMFile(t, h.clock.Now(), "file.fixture.test", 5)
	dir := t.TempDir()
	expect(t, h.ok("certificates", "import", "--file", pem, "--dry-run"), "Dry run: Import certificates from", "would import 1 certificate, 0 already tracked", "file.fixture.test", "Nothing was changed.")
	expect(t, h.ok("certificates", "list"), "No certificates yet")
	expect(t, h.ok("certificates", "import", "--file", pem), "Imported 1 certificate, 0 already tracked.")
	h.fails(ExitUsage, "PEM file is missing", "certificates", "import")
	h.fails(ExitFailure, "cannot open the file", "certificates", "import", "--file", filepath.Join(dir, "missing.pem"))
	expect(t, h.ok("certificates", "list", "--filter", "expiring-7"), "file.fixture.test", "5 days", "imported")
	expect(t, h.ok("alerts", "list"), "Certificate for file.fixture.test expires within 7 days")
	one := filepath.Join(dir, "one.pem")
	expect(t, h.ok("certificates", "export", "1", "--file", one), "Wrote the certificate file.fixture.test")
	h.fails(ExitFailure, "already exists", "certificates", "export", "1", "--file", one)
	h.ok("certificates", "export", "1", "--file", one, "--overwrite")
	if written, err := os.ReadFile(one); err != nil || !strings.HasPrefix(string(written), "-----BEGIN CERTIFICATE-----") {
		t.Fatalf("exported file: %v", err)
	}
	h.fails(ExitUsage, "is not one of inventory, expiry, findings", "reports", "generate", "--kind", "pdf")
	expect(t, h.ok("reports", "generate", "--kind", "expiry", "--format", "csv"), "Generated report 1: Certificates expiring within 90 days, 1 row.")
	expect(t, h.ok("reports", "generate"), "Generated report 2: Certificate inventory, 1 row.")
	expect(t, h.ok("reports", "list"), "Certificate inventory", "markdown", "csv", "2 reports")
	expect(t, h.ok("reports", "show", "2"), "# Certificate inventory", "file.fixture.test", "Suggested file name")
	saved := filepath.Join(dir, "expiry.csv")
	expect(t, h.ok("reports", "export", "1", "--file", saved), "Wrote the report to "+saved)
	if written, err := os.ReadFile(saved); err != nil || !strings.Contains(string(written), "file.fixture.test") {
		t.Fatalf("saved report: %v", err)
	}
	expect(t, h.ok("reports", "remove", "1", "--yes"), "Removed report 1.")
	list := filepath.Join(dir, "endpoints.csv")
	if err := os.WriteFile(list, []byte("host,port,note\na.fixture.test,,first\nbad host,,x\n"), 0o600); err != nil {
		t.Fatal(err)
	}
	expect(t, h.ok("endpoints", "import", "--file", list, "--dry-run"), "would import 1 endpoint, 0 already monitored, 1 rejected", "a.fixture.test:443", "row 3")
	h.fails(ExitUsage, "--authorized", "endpoints", "import", "--file", list)
	expect(t, h.ok("endpoints", "import", "--file", list, "--authorized"), "Imported 1 endpoint, 0 already monitored, 1 rejected.", "row 3")
	out := filepath.Join(dir, "out.json")
	expect(t, h.ok("endpoints", "export", "--file", out), "Wrote 1 endpoint to "+out)
	h.fails(ExitUsage, "choose csv or json", "endpoints", "export", "--file", filepath.Join(dir, "out.txt"))
	expect(t, h.ok("certificates", "stop-tracking", "1", "--dry-run"), "Dry run: Stop tracking the imported certificate file.fixture.test")
	expect(t, h.ok("certificates", "stop-tracking", "1", "--yes"), "Stopped tracking file.fixture.test.")
}

func TestDemoOverviewAndAlerts(t *testing.T) {
	h := newHarness(t)
	expect(t, h.ok("overview"), "Expiring in 7 days", "Nothing is monitored yet", "demo load", "--authorized")
	expect(t, h.ok("demo", "load"), "Added 11 demo endpoints", ".demo.invalid", "never contacted")
	expect(t, h.ok("demo", "load"), "already loaded")
	overview := strings.Join(strings.Fields(h.ok("overview")), " ")
	expect(t, overview, "Expired 1 certwatch certificates list --filter expired", "Expiring in 7 days 1", "Expiring in 30 days 2", "Expiring in 90 days 5", "Chain failures 3", "Name mismatches 1", "Failed checks 1",
		"Soonest to expire", "legacy.demo.invalid (demo)", "Recent alerts", "Demo data is loaded.")
	var parsed map[string]any
	if err := json.Unmarshal([]byte(h.ok("overview", "--format", "json")), &parsed); err != nil || parsed["counts"].(map[string]any)["within90"].(float64) != 5 {
		t.Fatalf("json overview: %v, %v", parsed, err)
	}
	expect(t, h.ok("endpoints", "check-all", "--dry-run"), "There is nothing to check", "Demo endpoints are never contacted")
	expect(t, h.ok("endpoints", "check-all", "--yes"), "Nothing was checked. Demo endpoints are never contacted.")
	list := h.ok("alerts", "list")
	expect(t, list, "ID", "SEVERITY", "unread", "Certificate for legacy.demo.invalid has expired (demo)", "Chain validation failed for staging.demo.invalid:443 (demo)", "Certificate changed on portal.demo.invalid:443 (demo)")
	expect(t, h.ok("alerts", "list", "--severity", "critical"), "Critical")
	h.fails(ExitUsage, "unknown severity", "alerts", "list", "--severity", "urgent")
	expect(t, h.ok("alerts", "read", "1"), "Marked alert 1 as read")
	expect(t, h.ok("alerts", "list", "--unread", "--format", "json"), `"read": false`)
	h.fails(ExitFailure, "does not exist", "alerts", "read", "999")
	h.fails(ExitUsage, "not an alert identifier", "alerts", "read", "x")
	expect(t, h.ok("alerts", "read", "all"), "alerts as read")
	expect(t, h.ok("alerts", "list", "--unread"), "No alerts")
	expect(t, h.ok("demo", "remove", "--dry-run"), "Dry run: This removes ", "demo endpoints and certificates")
	h.fails(ExitUsage, "--yes", "demo", "remove")
	expect(t, h.ok("demo", "remove", "--yes"), "demo endpoints and certificates")
	expect(t, h.ok("demo", "remove", "--yes"), "no demo data")
	expect(t, h.ok("alerts", "list"), "No alerts")
}

func TestNotificationRules(t *testing.T) {
	h := newHarness(t)
	expect(t, h.ok("alerts", "rules", "list"), notify.EventExpiring, notify.EventValidation, notify.EventChanged, "default", "none")
	expect(t, h.ok("alerts", "rules", "set", notify.EventExpiring, "--min-severity", "CRITICAL", "--quiet-start", "22:00", "--quiet-end", "07:00", "--dedup-minutes", "15", "--enabled", "false"),
		"enabled no", "minimum severity Critical", "quiet hours 22:00 to 07:00", "de-duplication 15 minutes")
	expect(t, h.ok("alerts", "rules", "list"), "changed", "22:00 to 07:00")
	h.fails(ExitUsage, "both a start and an end", "alerts", "rules", "set", notify.EventChanged, "--quiet-start", "22:00")
	h.fails(ExitUsage, "not a time of day", "alerts", "rules", "set", notify.EventChanged, "--quiet-start", "25:00", "--quiet-end", "07:00")
	h.fails(ExitUsage, "whole number", "alerts", "rules", "set", notify.EventChanged, "--dedup-minutes", "soon")
	h.fails(ExitUsage, "true or false", "alerts", "rules", "set", notify.EventChanged, "--enabled", "maybe")
	h.fails(ExitUsage, "nothing to change", "alerts", "rules", "set", notify.EventChanged)
	h.fails(ExitUsage, "unknown event type", "alerts", "rules", "set", "nope", "--enabled", "true")
	h.fails(ExitUsage, "give one event type: "+notify.EventExpiring, "alerts", "rules", "set")
	expect(t, h.ok("alerts", "rules", "reset", notify.EventExpiring), "back to its defaults")
	h.fails(ExitUsage, "unknown event type", "alerts", "rules", "reset", "nope")
}

func TestNotifierCommands(t *testing.T) {
	h := newHarness(t)
	entry := filepath.Join(h.home, ".config", "autostart", meta.Slug+"-notifier.desktop")
	expect(t, h.ok("notifier", "status"), "Start at sign-in: off", "not verified")
	expect(t, h.ok("notifier", "install", "--dry-run"), "Dry run: would write "+entry)
	if _, err := os.Stat(entry); !errors.Is(err, os.ErrNotExist) {
		t.Fatal("a dry run installed the entry")
	}
	expect(t, h.ok("notifier", "install"), "Wrote "+entry)
	expect(t, h.ok("notifier", "status"), "Start at sign-in: on")
	if _, err := os.Stat(filepath.Join(h.home, ".local")); !errors.Is(err, os.ErrNotExist) {
		t.Fatal("managing the sign-in entry must not create a database")
	}
	expect(t, h.ok("notifier", "uninstall", "--dry-run"), "Dry run: would remove")
	expect(t, h.ok("notifier", "uninstall"), "Removed "+entry)
	expect(t, h.ok("notifier", "uninstall"), "not set to start at sign-in")
	expect(t, h.ok("notifier", "run", "--once"), "Checked once: 0 new alerts shown")
	h.clock.Set(time.Now().UTC().Truncate(time.Second))
	h.ok("certificates", "import", "--file", testkit.PEMFile(t, h.clock.Now(), "secret.fixture.test", 5))
	expect(t, h.ok("notifier", "run", "--once"), "Checked once: 1 new alert shown")
	if len(h.delivered) != 1 || h.delivered[0] != meta.AppName+" | 1 new alert" {
		t.Fatalf("delivered: %v", h.delivered)
	}
	expect(t, h.ok("notifier", "run", "--once"), "0 new alerts shown")
	h.ok("config", "set", settings.KeyNotifierTitles, "true")
	h.ok("certificates", "import", "--file", testkit.PEMFile(t, h.clock.Now(), "another.fixture.test", 5))
	h.ok("notifier", "run", "--once")
	if len(h.delivered) != 2 || h.delivered[1] != meta.AppName+" | Certificate for another.fixture.test expires within 7 days" {
		t.Fatalf("delivered: %v", h.delivered)
	}
}

func TestConfigCommands(t *testing.T) {
	h := newHarness(t)
	list := h.ok("config", "list")
	for _, definition := range settings.Definitions() {
		expect(t, list, definition.Key)
	}
	expect(t, list, "default", "(empty)")
	if out := h.ok("config", "get", settings.KeyDensity); out != "comfortable\n" {
		t.Fatalf("get: %q", out)
	}
	expect(t, h.ok("config", "set", settings.KeyDensity, "compact"), settings.KeyDensity+" = compact")
	expect(t, strings.Join(strings.Fields(h.ok("config", "list")), " "), settings.KeyDensity+" compact stored")
	expect(t, h.ok("config", "set", settings.KeyEndpointColumns, "tls,status"), settings.KeyEndpointColumns+" = endpoint,status,tls")
	h.fails(ExitUsage, "not a column of this table", "config", "set", settings.KeyEndpointColumns, "owner")
	h.fails(ExitUsage, "not a sortable column", "config", "set", settings.KeyEndpointSort, "owner:desc")
	h.ok("config", "reset", settings.KeyEndpointColumns)
	h.fails(ExitUsage, "not one of the allowed values", "config", "set", settings.KeyDensity, "tiny")
	h.fails(ExitUsage, "unknown setting", "config", "set", "nope", "1")
	h.fails(ExitUsage, "unknown setting", "config", "get", "nope")
	h.fails(ExitUsage, "give a key and a value", "config", "set", settings.KeyDensity)
	h.env[settings.EnvName(settings.KeyDensity)] = "comfortable"
	stdout, stderr, _ := h.run("config", "get", settings.KeyDensity)
	if stdout != "comfortable\n" || !strings.Contains(stderr, "overridden by environment variable "+settings.EnvName(settings.KeyDensity)) || !strings.Contains(stderr, `"compact"`) {
		t.Fatalf("override: %q %q", stdout, stderr)
	}
	expect(t, h.ok("config", "list"), "overridden by environment variable")
	stdout, stderr, _ = h.run("--set", settings.KeyDensity+"=compact", "config", "get", settings.KeyDensity)
	if stdout != "compact\n" || !strings.Contains(stderr, "command-line flag --set") {
		t.Fatalf("flag override: %q %q", stdout, stderr)
	}
	_, stderr, _ = h.run("config", "set", settings.KeyDensity, "compact")
	expect(t, stderr, "still takes precedence")
	delete(h.env, settings.EnvName(settings.KeyDensity))
	expect(t, h.ok("config", "reset", settings.KeyDensity), "back to its default: comfortable")
	var parsed []map[string]string
	if err := json.Unmarshal([]byte(h.ok("config", "list", "--format", "json")), &parsed); err != nil || len(parsed) < len(settings.Definitions()) {
		t.Fatalf("json: %v", err)
	}
	h.env[settings.EnvName(settings.KeyTextScale)] = "9000"
	_, stderr, _ = h.run("config", "list")
	expect(t, stderr, settings.KeyTextScale, "is ignored")
}

func TestThemeCommands(t *testing.T) {
	h := newHarness(t)
	list := h.ok("config", "theme", "list")
	expect(t, list, "slate", "Neutral slate", "blue", "green", "crimson", "amber", "purple", "contrast", "High contrast", "in use")
	expect(t, h.ok("config", "theme", "set", "purple"), "Color theme: purple (Purple)")
	h.fails(ExitUsage, "not one of the allowed values", "config", "theme", "set", "neon")
	h.fails(ExitUsage, "give one theme name", "config", "theme", "set")
	show := h.ok("config", "theme", "show")
	expect(t, show, "purple (Purple), stored", "Colors listed    light", "#6230C2", "contrast", "Terminal colors  16 colors")
	h.ok("config", "set", settings.KeyMode, "dark")
	h.env["NO_COLOR"] = "1"
	expect(t, h.ok("config", "theme", "show"), "Colors listed    dark", "#B79BFF", "monochrome")
	h.ok("config", "set", settings.KeyAccent, "#101010")
	_, stderr, _ := h.run("config", "theme", "show")
	expect(t, stderr, "was adjusted")
	reopened := h.ok("config", "get", settings.KeyTheme)
	if reopened != "purple\n" {
		t.Fatalf("the theme was not saved: %q", reopened)
	}
}

func TestLauncherPreferenceCommands(t *testing.T) {
	h := newHarness(t)
	expect(t, h.ok("config", "launcher", "show"), "not set")
	expect(t, h.ok("config", "launcher", "set", "tui"), "tui (terminal UI)")
	expect(t, h.ok("config", "launcher", "show"), "tui (terminal UI)")
	h.fails(ExitUsage, "give gui or tui", "config", "launcher", "set", "web")
	h.env[meta.EnvName("ui")] = "gui"
	_, stderr, _ := h.run("config", "launcher", "show")
	expect(t, stderr, "overridden by environment variable "+meta.EnvName("ui")+"=gui")
	expect(t, h.ok("config", "launcher", "reset"), "not set")
}

func TestDatabaseCommands(t *testing.T) {
	h := newHarness(t)
	defaultPath := filepath.Join(h.home, ".local", "share", meta.Slug, meta.Slug+".db")
	stdout, stderr, code := h.run("db", "path")
	if code != ExitOK || stdout != defaultPath+"\n" || !strings.Contains(stderr, "default location, not created yet") {
		t.Fatalf("path: %q %q", stdout, stderr)
	}
	if _, err := os.Stat(defaultPath); !errors.Is(err, os.ErrNotExist) {
		t.Fatal("db path created the database")
	}
	expect(t, h.ok("db", "init"), "Database ready at "+defaultPath)
	expect(t, h.ok("db", "init"), "already exists")
	expect(t, h.ok("db", "check"), "Database is healthy", "schema version 1")
	h.ok("endpoints", "add", "--host", "travels.example.org", "--authorized")
	target := filepath.Join(h.home, "moved", "data.db")
	expect(t, h.ok("db", "move", target, "--dry-run"), "Dry run: Copy the current database to "+target)
	h.fails(ExitUsage, "--yes", "db", "move", target)
	expect(t, h.ok("db", "move", target, "--yes"), "Now using "+target)
	_, stderr, _ = h.run("db", "path")
	expect(t, stderr, "saved location, exists")
	expect(t, h.ok("endpoints", "list"), "travels")
	h.fails(ExitUsage, "would be overwritten", "db", "move", defaultPath, "--yes")
	expect(t, h.ok("db", "move", defaultPath, "--switch", "--yes"), "Now using "+defaultPath)
	h.fails(ExitUsage, "already at this location", "db", "move", defaultPath)
	other := filepath.Join(h.home, "other.db")
	expect(t, h.ok("--db", other, "endpoints", "list"), "No endpoints yet")
	stdout, stderr, _ = h.run("--db", other, "db", "path")
	if stdout != other+"\n" || !strings.Contains(stderr, "command-line flag --db") {
		t.Fatalf("flag path: %q %q", stdout, stderr)
	}
	h.fails(ExitUsage, "remove the override first", "--db", other, "db", "move", filepath.Join(h.home, "x.db"), "--yes")
	h.env[meta.EnvName("db")] = other
	_, stderr, _ = h.run("db", "path")
	expect(t, stderr, "environment variable "+meta.EnvName("db"))
	foreign := filepath.Join(h.home, "notes.txt")
	if err := os.WriteFile(foreign, []byte("hello"), 0o600); err != nil {
		t.Fatal(err)
	}
	h.fails(ExitFailure, "left untouched", "--db", foreign, "endpoints", "list")
}

func TestDiagnosticsGuideAboutAndLicenses(t *testing.T) {
	h := newHarness(t)
	expect(t, h.ok("diagnostics"), "Version", meta.Version, "Schema version", "Integrity check", "ok", "Dashboard in this build        no")
	guide := h.ok("guide")
	expect(t, guide, "usage guide", "Contents:")
	sections := app.UsageGuide().Sections
	first := sections[len(sections)-1].Title
	expect(t, h.ok("guide", first), first)
	h.fails(ExitFailure, "no section of the usage guide mentions", "guide", "zzzzqqqq")
	about := h.ok("about")
	expect(t, about, meta.AppName, meta.Version, "Publisher", meta.Publisher, meta.RepositoryURL, "/security/advisories/new", app.PrivacyNote, "Nothing is opened or sent")
	issue := h.ok("about", "--issue")
	expect(t, issue, "Found a bug? Please report it on GitHub.", "exactly this text", "Operating system:", meta.RepositoryURL+"/issues/new?")
	if strings.Contains(issue, h.home) {
		t.Fatal("the bug report preview contains a local path")
	}
	expect(t, h.ok("licenses"), "MIT License")
	if _, err := os.Stat(filepath.Join(h.home, ".local")); !errors.Is(err, os.ErrNotExist) {
		h.t.Log("database exists because diagnostics and about need it")
	}
}

func TestLaunchDecisions(t *testing.T) {
	type opened struct{ gui, tui int }
	setup := func(t *testing.T) (*harness, *opened) {
		h := newHarness(t)
		count := &opened{}
		h.gui = func(context.Context, *app.App) (string, error) { count.gui++; return "", nil }
		h.tui = func(context.Context, *app.App) (string, error) { count.tui++; return "", nil }
		return h, count
	}
	terminal := func(h *harness) {
		h.session.StdinTerminal, h.session.StdoutTerminal, h.session.StderrTerminal = true, true, true
		h.env["DISPLAY"], h.env["TERM"] = ":0", "xterm"
	}
	t.Run("terminal prompt opens the chosen front end and remembers it", func(t *testing.T) {
		h, count := setup(t)
		terminal(h)
		h.stdin = "r\n2\n"
		expect(t, h.ok(), "How do you want to open "+meta.AppName+"?", "1  Dashboard window", "2  Terminal UI")
		if count.tui != 1 || count.gui != 0 {
			t.Fatalf("opened %+v", count)
		}
		h.stdin = ""
		h.ok()
		if count.tui != 2 {
			t.Fatalf("the remembered choice was not used: %+v", count)
		}
		h.stdin = "1\n"
		h.ok("--ask")
		if count.gui != 1 {
			t.Fatalf("--ask did not ask again: %+v", count)
		}
		h.ok("--ui", "gui")
		if count.gui != 2 {
			t.Fatalf("--ui gui: %+v", count)
		}
		h.env[meta.EnvName("ui")] = "tui"
		h.ok()
		if count.tui != 3 {
			t.Fatalf("environment choice: %+v", count)
		}
	})
	t.Run("enter uses the default and quit opens nothing", func(t *testing.T) {
		h, count := setup(t)
		terminal(h)
		h.stdin = "\n"
		h.ok()
		h.stdin = "q\n"
		h.ok()
		if count.gui != 1 || count.tui != 0 {
			t.Fatalf("opened %+v", count)
		}
	})
	t.Run("no prompt opens the default", func(t *testing.T) {
		h, count := setup(t)
		terminal(h)
		out := h.ok("--no-prompt")
		if count.gui != 1 || strings.Contains(out, "How do you want") {
			t.Fatalf("opened %+v, output %q", count, out)
		}
	})
	t.Run("headless opens the terminal UI with a reason", func(t *testing.T) {
		h, count := setup(t)
		terminal(h)
		delete(h.env, "DISPLAY")
		_, stderr, code := h.run()
		if code != ExitOK || count.tui != 1 || !strings.Contains(stderr, "no display is available, opening the terminal UI") {
			t.Fatalf("code %d, opened %+v, stderr %q", code, count, stderr)
		}
		h.fails(ExitUsage, "no display is available", "--ui", "gui")
	})
	t.Run("automation and pipes never prompt", func(t *testing.T) {
		h, count := setup(t)
		terminal(h)
		h.env["CI"] = "true"
		h.stdin = "1\n"
		stdout, stderr, code := h.run()
		if code != ExitOK || count.gui+count.tui != 0 || strings.Contains(stdout, "How do you want") || !strings.Contains(stderr, "automation environment detected") || !strings.Contains(stderr, "Commands:") {
			t.Fatalf("code %d, opened %+v\n%s\n%s", code, count, stdout, stderr)
		}
		delete(h.env, "CI")
		h.session.StdinTerminal = false
		_, stderr, code = h.run("--ask")
		if code != ExitOK || count.gui+count.tui != 0 || !strings.Contains(stderr, "not interactive") {
			t.Fatalf("piped stdin: %d %+v %q", code, count, stderr)
		}
		h.fails(ExitUsage, "not an interactive terminal", "--ui", "tui")
	})
	t.Run("invalid choices fail with the fix", func(t *testing.T) {
		h, _ := setup(t)
		terminal(h)
		h.fails(ExitUsage, "use --ui gui or --ui tui", "--ui", "web")
		h.env[meta.EnvName("ui")] = "web"
		h.fails(ExitUsage, meta.EnvName("ui"))
	})
	t.Run("build without dashboard", func(t *testing.T) {
		h, count := setup(t)
		terminal(h)
		h.gui = nil
		_, _, code := h.run()
		if code != ExitOK || count.tui != 1 {
			t.Fatalf("code %d, opened %+v", code, count)
		}
		h.fails(ExitUsage, "this build has no dashboard", "--ui", "gui")
		expect(t, h.ok("diagnostics"), "Dashboard in this build        no")
	})
	t.Run("desktop launch shows the chooser", func(t *testing.T) {
		h, count := setup(t)
		h.env["DISPLAY"] = ":0"
		var defaults []string
		h.chooser = func(defaultMode string) (launcher.PromptResult, error) {
			defaults = append(defaults, defaultMode)
			return launcher.PromptResult{Mode: launcher.ModeGUI, Remember: true}, nil
		}
		h.ok()
		h.ok()
		if count.gui != 2 || len(defaults) != 1 || defaults[0] != launcher.ModeGUI {
			t.Fatalf("opened %+v, chooser defaults %v", count, defaults)
		}
		h.chooser = func(string) (launcher.PromptResult, error) { return launcher.PromptResult{Quit: true}, nil }
		h.ok("--ask")
		if count.gui != 2 {
			t.Fatal("quitting the chooser opened a front end")
		}
		var windowArgs []string
		h.window = func(executable string, arguments []string) error {
			windowArgs = append([]string{executable}, arguments...)
			return nil
		}
		h.chooser = func(string) (launcher.PromptResult, error) {
			return launcher.PromptResult{Mode: launcher.ModeTUI}, nil
		}
		h.ok("--ask", "--set", settings.KeyTheme+"=green")
		want := filepath.Join(h.home, "bin", meta.Slug) + " --ui tui --set " + settings.KeyTheme + "=green"
		if strings.Join(windowArgs, " ") != want || count.tui != 0 {
			t.Fatalf("terminal window: %v, want %s", windowArgs, want)
		}
		h.window = nil
		h.fails(ExitFailure, "no terminal window can be opened", "--ui", "tui")
	})
	t.Run("front ends can hand over to each other", func(t *testing.T) {
		h, count := setup(t)
		terminal(h)
		h.gui = func(context.Context, *app.App) (string, error) {
			count.gui++
			if count.gui == 1 {
				return launcher.ModeTUI, nil
			}
			return "", nil
		}
		h.tui = func(context.Context, *app.App) (string, error) { count.tui++; return launcher.ModeGUI, nil }
		h.ok("--ui", "gui")
		if count.gui != 2 || count.tui != 1 {
			t.Fatalf("hand-over: %+v", count)
		}
		h.gui = func(context.Context, *app.App) (string, error) { return "", errors.New("no window system") }
		h.fails(ExitFailure, "no window system", "--ui", "gui")
	})
	t.Run("subcommands never open a front end", func(t *testing.T) {
		h, count := setup(t)
		terminal(h)
		h.ok("--ui", "gui", "--ask", "overview")
		if count.gui+count.tui != 0 {
			t.Fatalf("a subcommand opened a front end: %+v", count)
		}
	})
}

func TestEveryCapabilityHasACommand(t *testing.T) {
	paths := map[string]bool{}
	for _, path := range CommandPaths() {
		paths[path] = true
	}
	flags := map[string]bool{}
	for _, line := range describeFlags(globalFlagSet(&globals{overrides: map[string]string{}}), nil) {
		flags[strings.Fields(line)[0]] = true
	}
	for _, capability := range app.Capabilities() {
		if strings.HasPrefix(capability.CLI, "--") {
			if !flags[capability.CLI] {
				t.Errorf("capability %s names the global flag %s, which does not exist", capability.ID, capability.CLI)
			}
			continue
		}
		if !paths[capability.CLI] {
			t.Errorf("capability %s names the command %q, which does not exist", capability.ID, capability.CLI)
		}
	}
	used := map[string]bool{}
	for _, capability := range app.Capabilities() {
		used[capability.CLI] = true
	}
	for path := range paths {
		if !used[path] && path != "version" && path != "db path" && path != "config get" {
			t.Errorf("command %q is missing from the capability matrix", path)
		}
	}
}

func TestCommandReferenceIsDocumented(t *testing.T) {
	reference := Reference()
	for _, path := range CommandPaths() {
		if !strings.Contains(reference, "`"+meta.Slug+" "+path+"`") {
			t.Fatalf("reference lacks %s", path)
		}
	}
	documented, err := os.ReadFile(filepath.Join("..", "..", "docs", "COMMANDS.md"))
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(strings.ReplaceAll(string(documented), "\r\n", "\n"), reference) {
		t.Fatalf("docs/COMMANDS.md does not contain the current command reference; replace its table with:\n%s", reference)
	}
}

func TestEveryCommandHasHelp(t *testing.T) {
	h := newHarness(t)
	for _, path := range CommandPaths() {
		out := h.ok(append(strings.Fields(path), "--help")...)
		if !strings.Contains(out, "Usage:") || !strings.Contains(out, meta.Slug+" "+path) {
			t.Fatalf("help for %q:\n%s", path, out)
		}
	}
	for _, leaf := range leaves(root()) {
		if leaf.summary == "" || leaf.run == nil {
			t.Fatalf("command %q has no summary or no action", leaf.path())
		}
	}
}

func TestPermute(t *testing.T) {
	set := root().find("endpoints").find("list").flagSet()
	flags, positional := permute(set, []string{"first", "--search", "two words", "--all", "second", "--limit=5", "--", "--not-a-flag"})
	if strings.Join(flags, "|") != "--search|two words|--all|--limit=5" || strings.Join(positional, "|") != "first|second|--not-a-flag" {
		t.Fatalf("flags %q, positional %q", flags, positional)
	}
	flags, positional = permute(set, []string{"-", "--search"})
	if strings.Join(flags, "|") != "--search" || strings.Join(positional, "|") != "-" {
		t.Fatalf("flags %q, positional %q", flags, positional)
	}
}
