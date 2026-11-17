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

	"github.com/unalivin/templateapp/internal/app"
	"github.com/unalivin/templateapp/internal/launcher"
	"github.com/unalivin/templateapp/internal/meta"
	"github.com/unalivin/templateapp/internal/notify"
	"github.com/unalivin/templateapp/internal/settings"
	"github.com/unalivin/templateapp/internal/testkit"
	"github.com/unalivin/templateapp/internal/userconfig"
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
		Autostart: func() notify.Autostart {
			return notify.Autostart{GOOS: "linux", HomeDir: h.home, Executable: filepath.Join(h.home, "bin", meta.Slug)}
		},
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
	expect(t, h.ok("--help"), meta.AppName, "Commands:", "records", "notifier", "Global flags:", "--ui <value>", "--no-prompt", meta.EnvName("ui"), "NO_COLOR")
	expect(t, h.ok("help"), "Commands:")
	expect(t, h.ok("help", "records"), "Manage records", "import", "export")
	expect(t, h.ok("records", "remove", "--help"), "Remove a record", "--dry-run", "--yes", "<id>")
	expect(t, h.ok("help", "config", "theme"), "list", "set", "show")
	if out := h.ok("--version"); out != meta.Slug+" "+meta.Version+"\n" {
		t.Fatalf("version: %q", out)
	}
	if out := h.ok("version"); out != meta.Slug+" "+meta.Version+"\n" {
		t.Fatalf("version: %q", out)
	}
	h.fails(ExitUsage, `unknown command "recrods"`, "recrods")
	h.fails(ExitUsage, `Did you mean "records list"?`, "records", "lis")
	h.fails(ExitUsage, "flag provided but not defined", "--bogus")
	h.fails(ExitUsage, "flag provided but not defined", "records", "list", "--bogus")
	h.fails(ExitUsage, "unknown setting", "--set", "nope=1", "overview")
	h.fails(ExitUsage, "key=value", "--set", "novalue", "overview")
	_, stderr, code := h.run("records")
	if code != ExitUsage || !strings.Contains(stderr, "Commands:") {
		t.Fatalf("a command group without a command must print its help: %d %s", code, stderr)
	}
	if _, err := os.Stat(filepath.Join(h.home, ".local")); !errors.Is(err, os.ErrNotExist) {
		t.Fatal("help and usage errors must not create a database")
	}
}

func TestRecordLifecycle(t *testing.T) {
	h := newHarness(t)
	stdout, stderr, code := h.run("records", "list")
	if code != ExitOK || !strings.Contains(stdout, "No records yet") || !strings.Contains(stderr, "Created a new database at") {
		t.Fatalf("first run: %d\n%s\n%s", code, stdout, stderr)
	}
	expect(t, h.ok("records", "add", "--name", "Core switch", "--severity", "high", "--note", "rack 4"), "Added record 1: Core switch")
	expect(t, h.ok("records", "add", "--name=Edge router", "--disabled"), "Added record 2")
	h.fails(ExitUsage, "a name is required", "records", "add")
	h.fails(ExitUsage, "already exists", "records", "add", "--name", "core switch ", "--name", "Core switch")
	h.fails(ExitUsage, "unknown severity", "records", "add", "--name", "x", "--severity", "urgent")
	list := h.ok("records", "list")
	expect(t, list, "ID", "NAME", "Core switch", "High", "Open")
	if strings.Contains(list, "Edge router") {
		t.Fatal("a disabled record is listed without --all")
	}
	expect(t, h.ok("records", "list", "--all", "--sort", "name", "--desc"), "Edge router")
	expect(t, h.ok("records", "list", "--search", "rack"), "Core switch")
	expect(t, h.ok("records", "list", "--search", "nothing"), "No records match")
	h.fails(ExitUsage, "unknown sort column", "records", "list", "--sort", "id;drop")
	h.fails(ExitUsage, "unknown format", "records", "list", "--format", "xml")
	var parsed []map[string]any
	if err := json.Unmarshal([]byte(h.ok("records", "list", "--all", "--format", "json")), &parsed); err != nil || len(parsed) != 2 || parsed[0]["name"] != "Core switch" {
		t.Fatalf("json list: %v, %v", parsed, err)
	}
	expect(t, h.ok("records", "list", "--format", "csv"), "name,status,severity,note,enabled", "Core switch,open,high,rack 4,true")
	expect(t, h.ok("records", "show", "1"), "Core switch", "rack 4", "Revision")
	expect(t, h.ok("records", "edit", "1", "--status", "in_progress", "--note", "rack 5"), "Updated record 1 (revision 2)")
	expect(t, h.ok("records", "edit", "--severity", "critical", "1"), "revision 3")
	h.fails(ExitUsage, "nothing to change", "records", "edit", "1")
	h.fails(ExitUsage, "unknown status", "records", "edit", "1", "--status", "closed")
	h.fails(ExitUsage, "not a record identifier", "records", "show", "abc")
	h.fails(ExitUsage, "exactly one record identifier", "records", "show")
	h.fails(ExitFailure, "does not exist", "records", "show", "99")
	expect(t, h.ok("records", "enable", "2"), "Record 2 is enabled")
	expect(t, h.ok("records", "disable", "2"), "Record 2 is disabled")
	h.fails(ExitFailure, "does not exist", "records", "enable", "99")
	expect(t, h.ok("records", "remove", "2", "--dry-run"), "Dry run: This removes record 2 (Edge router) permanently. Nothing was changed.")
	h.fails(ExitUsage, "repeat the command with --yes", "records", "remove", "2")
	expect(t, h.ok("records", "show", "2"), "Edge router")
	expect(t, h.ok("records", "remove", "2", "--yes"), "Removed record 2")
	h.fails(ExitFailure, "does not exist", "records", "remove", "2", "--yes")
}

func TestRemoveAsksOnAnInteractiveTerminal(t *testing.T) {
	h := newHarness(t)
	h.ok("records", "add", "--name", "keep me")
	h.session.StdinTerminal = true
	h.stdin = "no\n"
	h.fails(ExitFailure, "cancelled, nothing was changed", "records", "remove", "1")
	h.stdin = "YES\n"
	_, stderr, code := h.run("records", "remove", "1")
	if code != ExitOK || !strings.Contains(stderr, "This removes record 1 (keep me) permanently.") {
		t.Fatalf("confirmed removal: %d %s", code, stderr)
	}
	h.ok("records", "add", "--name", "no questions")
	h.ok("config", "set", settings.KeyConfirm, "false")
	h.session.StdinTerminal = false
	expect(t, h.ok("records", "remove", "2"), "Removed record 2")
}

func TestImportAndExport(t *testing.T) {
	h := newHarness(t)
	source := filepath.Join(h.home, "in.csv")
	if err := os.WriteFile(source, []byte("name,severity\nalpha,high\nbeta,nope\n"), 0o600); err != nil {
		t.Fatal(err)
	}
	stdout, stderr, code := h.run("records", "import", source, "--dry-run")
	if code != ExitOK || !strings.Contains(stdout, "Dry run: would have created 1, updated 0, unchanged 0, rejected 1") || !strings.Contains(stderr, "rejected row 3") {
		t.Fatalf("dry run: %d\n%s\n%s", code, stdout, stderr)
	}
	expect(t, h.ok("records", "list"), "No records yet")
	expect(t, h.ok("records", "import", source), "Created 1, updated 0, unchanged 0, rejected 1")
	h.stdin = `[{"name":"gamma","severity":"low"}]`
	expect(t, h.ok("records", "import", "-", "--format", "json"), "Created 1")
	h.fails(ExitUsage, "add --format csv or --format json", "records", "import", "-")
	h.fails(ExitUsage, "add --format", "records", "import", filepath.Join(h.home, "data.txt"))
	h.fails(ExitFailure, "cannot open the import file", "records", "import", filepath.Join(h.home, "missing.csv"))
	h.fails(ExitUsage, "give one file", "records", "import")
	bad := filepath.Join(h.home, "bad.csv")
	if err := os.WriteFile(bad, []byte("name,status\nx,closed\n"), 0o600); err != nil {
		t.Fatal(err)
	}
	h.fails(ExitFailure, "no row could be imported", "records", "import", bad)
	expect(t, h.ok("records", "export"), "name,status,severity,note,enabled", "alpha,open,high,,true", "gamma")
	expect(t, h.ok("records", "export", "--format", "json"), `"name": "alpha"`)
	target := filepath.Join(h.home, "out.json")
	expect(t, h.ok("records", "export", "--format", "json", "--output", target), "Exported 2 records to "+target)
	h.fails(ExitUsage, "already exists", "records", "export", "--output", target)
	h.ok("records", "export", "--output", target, "--force")
	content, err := os.ReadFile(target)
	if err != nil || !strings.HasPrefix(string(content), "name,status") {
		t.Fatalf("forced export: %q, %v", content, err)
	}
	if info, _ := os.Stat(target); os.PathSeparator == '/' && info.Mode().Perm() != 0o600 {
		t.Fatalf("export mode %v", info.Mode().Perm())
	}
	h.fails(ExitFailure, "cannot write the export", "records", "export", "--output", filepath.Join(h.home, "no such dir", "x.csv"))
	h.fails(ExitUsage, "unknown format", "records", "export", "--format", "xml")
}

func TestDemoOverviewAndNotifications(t *testing.T) {
	h := newHarness(t)
	expect(t, h.ok("overview"), "Open records", "No records yet", "demo load")
	expect(t, h.ok("demo", "load"), "Added 6 demo records")
	expect(t, h.ok("demo", "load"), "already loaded")
	overview := h.ok("overview")
	expect(t, overview, "Open records                                 4", "Unread notifications                         2", "Critical", "Demo data is loaded: 6 sample records")
	var parsed map[string]any
	if err := json.Unmarshal([]byte(h.ok("overview", "--format", "json")), &parsed); err != nil || parsed["urgent_records"].(float64) != 2 {
		t.Fatalf("json overview: %v, %v", parsed, err)
	}
	list := h.ok("notifications", "list")
	expect(t, list, "ID", "SEVERITY", "unread", "A record needs attention", "record ")
	if strings.Contains(list, "Demo:") {
		t.Fatal("notification titles must not carry record names")
	}
	expect(t, h.ok("notifications", "list", "--severity", "critical"), "Critical")
	h.fails(ExitUsage, "unknown severity", "notifications", "list", "--severity", "urgent")
	expect(t, h.ok("notifications", "read", "1"), "Marked notification 1 as read")
	expect(t, h.ok("notifications", "list", "--unread", "--format", "json"), `"read": false`)
	h.fails(ExitFailure, "does not exist", "notifications", "read", "999")
	h.fails(ExitUsage, "not a notification identifier", "notifications", "read", "x")
	expect(t, h.ok("notifications", "read", "all"), "Marked 1 notifications as read")
	expect(t, h.ok("notifications", "list", "--unread"), "No notifications")
	expect(t, h.ok("demo", "remove", "--dry-run"), "Dry run: This removes 6 demo records.")
	h.fails(ExitUsage, "--yes", "demo", "remove")
	expect(t, h.ok("demo", "remove", "--yes"), "Removed 6 demo records")
	expect(t, h.ok("demo", "remove", "--yes"), "no demo data")
}

func TestNotificationRules(t *testing.T) {
	h := newHarness(t)
	expect(t, h.ok("notifications", "rules", "list"), notify.EventRecordUrgent, notify.EventRecordCompleted, "default", "none")
	expect(t, h.ok("notifications", "rules", "set", notify.EventRecordUrgent, "--min-severity", "CRITICAL", "--quiet-start", "22:00", "--quiet-end", "07:00", "--dedup-minutes", "15", "--enabled", "false"),
		"enabled no", "minimum severity Critical", "quiet hours 22:00 to 07:00", "de-duplication 15 minutes")
	expect(t, h.ok("notifications", "rules", "list"), "changed", "22:00 to 07:00")
	h.fails(ExitUsage, "both a start and an end", "notifications", "rules", "set", notify.EventRecordCompleted, "--quiet-start", "22:00")
	h.fails(ExitUsage, "not a time of day", "notifications", "rules", "set", notify.EventRecordCompleted, "--quiet-start", "25:00", "--quiet-end", "07:00")
	h.fails(ExitUsage, "whole number", "notifications", "rules", "set", notify.EventRecordCompleted, "--dedup-minutes", "soon")
	h.fails(ExitUsage, "true or false", "notifications", "rules", "set", notify.EventRecordCompleted, "--enabled", "maybe")
	h.fails(ExitUsage, "nothing to change", "notifications", "rules", "set", notify.EventRecordCompleted)
	h.fails(ExitUsage, "unknown event type", "notifications", "rules", "set", "nope", "--enabled", "true")
	h.fails(ExitUsage, "give one event type: "+notify.EventRecordUrgent, "notifications", "rules", "set")
	expect(t, h.ok("notifications", "rules", "reset", notify.EventRecordUrgent), "back to its defaults")
	h.fails(ExitUsage, "unknown event type", "notifications", "rules", "reset", "nope")
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
	expect(t, h.ok("notifier", "run", "--once"), "Checked once: 0 new notifications shown")
	h.ok("records", "add", "--name", "secret project name", "--severity", "critical")
	expect(t, h.ok("notifier", "run", "--once"), "Checked once: 1 new notifications shown")
	if len(h.delivered) != 1 || h.delivered[0] != meta.AppName+" | 1 new notification" {
		t.Fatalf("delivered: %v", h.delivered)
	}
	expect(t, h.ok("notifier", "run", "--once"), "0 new notifications shown")
	h.ok("config", "set", settings.KeyNotifierTitles, "true")
	h.ok("records", "add", "--name", "another", "--severity", "high")
	h.ok("notifier", "run", "--once")
	if len(h.delivered) != 2 || h.delivered[1] != meta.AppName+" | A record needs attention" || strings.Contains(h.delivered[1], "another") {
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
	expect(t, h.ok("config", "set", settings.KeyRecordColumns, "severity,id"), settings.KeyRecordColumns+" = id,name,severity")
	h.fails(ExitUsage, "not a column of this table", "config", "set", settings.KeyRecordColumns, "owner")
	h.fails(ExitUsage, "not a sortable column", "config", "set", settings.KeyRecordSort, "owner:desc")
	h.ok("config", "reset", settings.KeyRecordColumns)
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
	h.ok("records", "add", "--name", "travels")
	target := filepath.Join(h.home, "moved", "data.db")
	expect(t, h.ok("db", "move", target, "--dry-run"), "Dry run: Copy the current database to "+target)
	h.fails(ExitUsage, "--yes", "db", "move", target)
	expect(t, h.ok("db", "move", target, "--yes"), "Now using "+target)
	_, stderr, _ = h.run("db", "path")
	expect(t, stderr, "saved location, exists")
	expect(t, h.ok("records", "list"), "travels")
	h.fails(ExitUsage, "would be overwritten", "db", "move", defaultPath, "--yes")
	expect(t, h.ok("db", "move", defaultPath, "--switch", "--yes"), "Now using "+defaultPath)
	h.fails(ExitUsage, "already at this location", "db", "move", defaultPath)
	other := filepath.Join(h.home, "other.db")
	expect(t, h.ok("--db", other, "records", "list"), "No records yet")
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
	h.fails(ExitFailure, "left untouched", "--db", foreign, "records", "list")
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
	expect(t, about, meta.AppName, meta.Version, "Publisher", meta.Publisher, meta.RepositoryURL, "/security/advisories/new", "Nothing is opened or sent")
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
	set := root().find("records").find("list").flagSet()
	flags, positional := permute(set, []string{"first", "--search", "two words", "--all", "second", "--limit=5", "--", "--not-a-flag"})
	if strings.Join(flags, "|") != "--search|two words|--all|--limit=5" || strings.Join(positional, "|") != "first|second|--not-a-flag" {
		t.Fatalf("flags %q, positional %q", flags, positional)
	}
	flags, positional = permute(set, []string{"-", "--search"})
	if strings.Join(flags, "|") != "--search" || strings.Join(positional, "|") != "-" {
		t.Fatalf("flags %q, positional %q", flags, positional)
	}
}
