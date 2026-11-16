package tui

import (
	"context"
	"errors"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"

	"github.com/unalivin/certwatch/internal/app"
	"github.com/unalivin/certwatch/internal/inventory"
	"github.com/unalivin/certwatch/internal/launcher"
	"github.com/unalivin/certwatch/internal/meta"
	"github.com/unalivin/certwatch/internal/notify"
	"github.com/unalivin/certwatch/internal/settings"
	"github.com/unalivin/certwatch/internal/testkit"
	"github.com/unalivin/certwatch/internal/theme"
	"github.com/unalivin/certwatch/internal/userconfig"
)

type screen struct {
	t      *testing.T
	m      *Model
	home   string
	env    map[string]string
	opened []string
	clock  *testkit.Clock
	server *testkit.TLSFixture
}

func (s *screen) appOptions() app.Options {
	options := app.Options{
		Environment: userconfig.Environment{Getenv: func(key string) string { return s.env[key] }, HomeDir: func() (string, error) { return s.home, nil }, GOOS: "linux"},
		Now:         s.clock.Now,
	}
	if s.server != nil {
		options.Roots, options.Dial = s.server.Pool(), s.server.Dial
	}
	return options
}

func newLiveScreen(t *testing.T, name string, days int) *screen {
	t.Helper()
	s := &screen{t: t, home: t.TempDir(), env: map[string]string{}, clock: testkit.NewClock()}
	s.clock.Set(time.Now().UTC().Truncate(time.Second))
	s.server = testkit.StartTLS(t, s.clock.Now(), name, days)
	s.m = New(context.Background(), Options{App: s.second(), Getenv: func(key string) string { return s.env[key] }})
	s.m.Update(tea.WindowSizeMsg{Width: 130, Height: 40})
	s.start()
	return s
}

func (s *screen) finish(command tea.Cmd) {
	s.t.Helper()
	if command == nil {
		s.t.Fatalf("no background work was started:\n%s", s.m.View())
	}
	s.m.Update(command())
}

func (s *screen) addEndpoint(host string) inventory.Endpoint {
	s.t.Helper()
	created, err := s.m.app.Inventory.CreateEndpoint(context.Background(), inventory.EndpointInput{Host: host, Port: 443, Enabled: true}, true)
	if err != nil {
		s.t.Fatal(err)
	}
	return created
}

func (s *screen) second() *app.App {
	s.t.Helper()
	other, err := app.Open(context.Background(), s.appOptions())
	if err != nil {
		s.t.Fatal(err)
	}
	s.t.Cleanup(func() { _ = other.Close() })
	return other
}

func newScreen(t *testing.T, change func(*Options)) *screen {
	t.Helper()
	s := &screen{t: t, home: t.TempDir(), env: map[string]string{}, clock: testkit.NewClock()}
	application := s.second()
	options := Options{
		App:          application,
		Getenv:       func(key string) string { return s.env[key] },
		HasDashboard: true,
		OpenURL:      func(url string) error { s.opened = append(s.opened, url); return nil },
		Autostart: func() notify.Autostart {
			return notify.Autostart{GOOS: "linux", HomeDir: s.home, Executable: filepath.Join(s.home, "bin", meta.Slug)}
		},
	}
	if change != nil {
		change(&options)
	}
	s.m = New(context.Background(), options)
	s.m.Update(tea.WindowSizeMsg{Width: 110, Height: 34})
	return s
}

var named = map[string]tea.KeyType{
	"enter": tea.KeyEnter, "esc": tea.KeyEsc, "tab": tea.KeyTab, "shift+tab": tea.KeyShiftTab, "up": tea.KeyUp, "down": tea.KeyDown,
	"left": tea.KeyLeft, "right": tea.KeyRight, "backspace": tea.KeyBackspace, "ctrl+c": tea.KeyCtrlC, "ctrl+s": tea.KeyCtrlS, "ctrl+u": tea.KeyCtrlU,
	"f1": tea.KeyF1, "f10": tea.KeyF10, "pgdown": tea.KeyPgDown, "delete": tea.KeyDelete,
}

func (s *screen) press(keys ...string) tea.Cmd {
	s.t.Helper()
	var command tea.Cmd
	for _, key := range keys {
		message := tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune(key)}
		if kind, found := named[key]; found {
			message = tea.KeyMsg{Type: kind}
		} else if key == " " {
			message = tea.KeyMsg{Type: tea.KeySpace, Runes: []rune(" ")}
		}
		_, command = s.m.Update(message)
	}
	return command
}

func (s *screen) write(text string) {
	for _, r := range text {
		s.press(string(r))
	}
}

func (s *screen) clear() {
	s.press("ctrl+u")
	for range 40 {
		s.press("delete")
	}
}

func flatten(view string) string {
	var words []string
	for _, line := range strings.Split(view, "\n") {
		if _, content, found := strings.Cut(line, "│ "); found {
			line = content
		}
		words = append(words, strings.Fields(line)...)
	}
	return strings.Join(words, " ")
}

func dense(text string) string {
	return strings.Join(strings.Fields(text), "")
}

func (s *screen) shows(parts ...string) {
	s.t.Helper()
	view := s.m.View()
	flat := flatten(view)
	for _, part := range parts {
		if !strings.Contains(view, part) && !strings.Contains(flat, strings.Join(strings.Fields(part), " ")) && !strings.Contains(dense(flat), dense(part)) {
			s.t.Fatalf("screen lacks %q:\n%s", part, view)
		}
	}
}

func (s *screen) hides(parts ...string) {
	s.t.Helper()
	view := s.m.View()
	flat := flatten(view)
	for _, part := range parts {
		if strings.Contains(view, part) || strings.Contains(flat, strings.Join(strings.Fields(part), " ")) || strings.Contains(dense(flat), dense(part)) {
			s.t.Fatalf("screen contains %q:\n%s", part, view)
		}
	}
}

func (s *screen) start() {
	s.shows("Welcome to " + meta.AppName)
	s.press("enter")
}

func TestFirstStartAndOverview(t *testing.T) {
	s := newScreen(t, nil)
	s.shows("Welcome to "+meta.AppName, "A new database was created at", "connects only to hosts you add and confirm that you are authorized to test", "Press l to load labeled demo data")
	s.press("l")
	s.shows("Added 11 demo endpoints", "They are never contacted", "What needs attention", "Expired 1", "Expiring in 7 days 1", "Expiring in 30 days 2", "Expiring in 90 days 5", "Chain failures 3", "Name mismatches 1", "Failed checks 1",
		"Soonest to expire", "api.demo.invalid (demo)", "expired 3 days ago", "Recent alerts", "Demo data is loaded", "7 Alerts (9)", "▌1 Overview")
	s.press("l")
	s.shows("already loaded")
	s.press("down", "down", "enter")
	s.shows(meta.AppName+" · Certificates", "Show: Expiring in 30 days", "mail.demo.invalid (demo)", "Row 1 of 2")
	s.press("1", "down", "down", "enter")
	s.shows(meta.AppName+" · Endpoints", "Show: Chain failures", "Row 1 of 3")
	reopened := New(context.Background(), Options{App: s.second()})
	if reopened.welcome {
		t.Fatal("the first-start screen appears again for an existing database")
	}
	empty := newScreen(t, nil)
	empty.start()
	empty.shows("Nothing is monitored yet", "a host you are authorized to test", "press l to load labeled demo data")
}

func TestNavigationAndFrame(t *testing.T) {
	s := newScreen(t, nil)
	s.start()
	for index, item := range app.Navigation() {
		digit := string(rune('0' + (index+1)%10))
		s.press(digit)
		s.shows(meta.AppName+" · "+item.Label, "▌"+digit+" "+item.Label)
	}
	s.press("tab")
	s.shows("▌1 Overview")
	s.press("shift+tab")
	s.shows("▌0 Settings")
	s.press("1", ",")
	s.shows("▌0 Settings")
	s.press("b")
	s.hides("0 Settings")
	if value, _ := s.m.app.Settings.Get(context.Background(), settings.KeySidebarCollapsed); value.Raw != "true" {
		t.Fatal("the collapsed sidebar was not saved")
	}
	s.press("b")
	s.shows("▌0 Settings")
	s.press("?")
	s.shows("▌Usage guide", "Getting started")
	s.press("f1", "r")
	s.shows("Refreshed")
	s.m.Update(tea.WindowSizeMsg{Width: 40, Height: 10})
	s.shows("needs a terminal of at least 60 columns and 16 rows", "This one is 40 by 10")
	s.press("l")
	for _, size := range [][2]int{{60, 16}, {80, 24}, {200, 50}} {
		s.m.Update(tea.WindowSizeMsg{Width: size[0], Height: size[1]})
		for index := range app.Navigation() {
			s.press(string(rune('0' + (index+1)%10)))
			for _, key := range []string{"", "enter"} {
				if key != "" {
					s.press(key)
				}
				lines := strings.Split(s.m.View(), "\n")
				if len(lines) > size[1] {
					t.Fatalf("page %d is taller than a %dx%d terminal:\n%s", index+1, size[0], size[1], s.m.View())
				}
				for _, line := range lines {
					if lipgloss.Width(line) > size[0] {
						t.Fatalf("page %d has a line wider than a %dx%d terminal: %q", index+1, size[0], size[1], line)
					}
				}
			}
			s.press("esc", "esc")
		}
	}
	s.press("1")
	if command := s.press("q"); command == nil || s.m.View() != "" {
		t.Fatal("q did not quit")
	}
}

func TestEndpointWorkflowWithALiveCheck(t *testing.T) {
	s := newLiveScreen(t, "app.fixture.test", 20)
	s.press("3")
	s.shows("No endpoints yet")
	s.press("n")
	s.shows("Endpoints, Add endpoint", "Host", "Port", "Server name", "Note", "I am authorized to test this host", "Host name or IP address")
	s.press("ctrl+s")
	s.shows("Not saved:", "Host is missing")
	s.write("app.fixture.test")
	s.press("ctrl+s")
	s.shows("Not saved:", "confirm that you are authorized to test this host")
	if len(s.server.Dialed()) != 0 {
		t.Fatal("adding an endpoint opened a connection")
	}
	s.press("tab", "tab", "tab")
	s.write("edge")
	s.press("tab", "right", "ctrl+s")
	s.shows("Added endpoint 1: app.fixture.test:443", "▌ app.fixture.test:443", "not checked yet", "Row 1 of 1")
	s.press("n")
	s.write("app.fixture.test")
	s.press("tab", "tab", "tab", "tab", "right", "ctrl+s")
	s.shows("Not saved:", "already monitored")
	s.press("esc")
	s.shows("Cancelled, nothing was changed")
	command := s.press("c")
	s.shows("Endpoints, Check now", "Check 1 endpoint over the network", "Network connections this opens:", "app.fixture.test:443: name lookup for app.fixture.test, then one TCP connection with a TLS handshake", "Press Enter to check now, or Esc to cancel")
	if command != nil || len(s.server.Dialed()) != 0 {
		t.Fatal("the check started before it was confirmed")
	}
	s.press("esc")
	s.shows("Cancelled, nothing was changed")
	if len(s.server.Dialed()) != 0 {
		t.Fatal("a cancelled check opened a connection")
	}
	s.press("c")
	command = s.press("enter")
	s.shows("◌ Check now: waiting for the answers")
	s.press("d", "q")
	s.shows("◌ Check now: waiting for the answers", "app.fixture.test:443")
	s.finish(command)
	s.shows("Result", "Checked 1 endpoint: 0 failed, 0 changed certificates, 1 new alert.", "app.fixture.test:443: ok (expires ")
	s.press("esc")
	s.shows("ok", "app.fixture.test", "20 days", "TLS 1.3", "enabled", "7 Alerts (1)")
	if dialed := s.server.Dialed(); len(dialed) != 1 || dialed[0] != "app.fixture.test:443" {
		t.Fatalf("connections: %v", dialed)
	}
	s.press("enter")
	s.shows("app.fixture.test:443", "Authorization confirmed", "Findings (0)", "Chain", "Fixture Root", "trust store", "Esc back to the table", "Lines 1 to")
	s.press("down", "pgdown", "esc")
	s.shows("Row 1 of 1")
	s.press("e")
	s.shows("Endpoints, Edit", "app.fixture.test", "edge")
	s.press("tab", "tab", "tab")
	s.write(" two")
	s.press("ctrl+s")
	s.shows("Saved endpoint 1: app.fixture.test:443")
	s.press("x")
	s.shows("Endpoint app.fixture.test:443 is now enabled")
	s.press("X")
	s.shows("Endpoint app.fixture.test:443 is now disabled", "disabled")
	s.press("f")
	s.shows("Show: Enabled", "No endpoint matches", "Press Esc to clear the search and the filter.")
	s.press("F", "x", "/")
	s.write("zzz")
	s.shows("Search: zzz", "Type to search", "No endpoint matches")
	s.press("enter", "esc")
	s.shows("Search: none", "app.fixture.test:443")
	s.press("2")
	s.shows("app.fixture.test", "Fixture Root", "20 days", "Row 1 of 1")
	s.press("enter")
	s.shows("Subject alternative names (1)", "SHA-256", "PRESENTED NOW")
	s.press("4")
	s.shows("Zoom: Next 90 days", "█", "1 of 1 tracked certificate expires within 90 days")
	s.press("f")
	s.shows("Zoom: Next 30 days")
	s.press("5")
	s.shows("valid", "Row 1 of 1")
	s.press("enter")
	s.shows("leaf", "root", "trust store")
	s.press("6")
	s.shows("TLS 1.3", "not probed")
	s.press("c")
	s.shows("Probe TLS versions", "Network connections this opens:")
	s.finish(s.press("enter"))
	s.press("esc")
	s.shows("TLS 1.2, TLS 1.3")
	s.press("3", "d")
	s.shows("Endpoints, Remove", "Remove endpoint app.fixture.test:443 and its 2 observations.", "Press Enter to remove, or Esc to cancel")
	s.press("n")
	s.shows("Cancelled, nothing was changed", "app.fixture.test:443")
	s.press("delete", "y")
	s.shows("Removed endpoint app.fixture.test:443.", "No endpoints yet")
	if _, err := s.m.app.Inventory.Endpoint(context.Background(), 1); !errors.Is(err, inventory.ErrNotFound) {
		t.Fatalf("the endpoint still exists: %v", err)
	}
}

func TestRemoveWithoutConfirmationWhenTheSettingIsOffButChecksStillAsk(t *testing.T) {
	s := newLiveScreen(t, "quick.fixture.test", 200)
	ctx := context.Background()
	s.addEndpoint("quick.fixture.test")
	if _, err := s.m.app.Settings.Set(ctx, settings.KeyConfirm, "false"); err != nil {
		t.Fatal(err)
	}
	s.press("r", "3")
	if command := s.press("C"); command != nil || len(s.server.Dialed()) != 0 {
		t.Fatal("a network check started without confirmation")
	}
	s.shows("Check all", "Network connections this opens:", "quick.fixture.test:443", "Press Enter to check all")
	s.press("esc", "d")
	s.shows("Removed endpoint quick.fixture.test:443.")
}

func TestConflictingEditKeepsTheForm(t *testing.T) {
	s := newScreen(t, nil)
	s.start()
	ctx := context.Background()
	created := s.addEndpoint("shared.example.org")
	s.press("r", "3", "e", "tab", "tab", "tab")
	s.write("mine")
	other := s.second()
	changed := created.Input()
	changed.Note = "from the other window"
	if _, err := other.Inventory.UpdateEndpoint(ctx, created.ID, created.Revision, changed, false); err != nil {
		t.Fatal(err)
	}
	s.m.Update(tickMsg{})
	s.shows("Endpoints, Edit", "mine")
	s.press("ctrl+s")
	s.shows("Not saved:", "another window changed shared.example.org:443 while you were editing", "mine")
	stored, _ := s.m.app.Inventory.Endpoint(ctx, created.ID)
	if stored.Note != "from the other window" {
		t.Fatalf("the other window's change was overwritten: %+v", stored)
	}
	s.press("esc", "e", "tab", "tab", "tab")
	s.shows("from the other window")
	s.write(" and mine")
	s.press("ctrl+s")
	s.shows("Saved endpoint 1")
	if stored, _ := s.m.app.Inventory.Endpoint(ctx, created.ID); stored.Note != "from the other window and mine" {
		t.Fatalf("note: %q", stored.Note)
	}
}

func TestExternalChangesAppearOnTheNextTick(t *testing.T) {
	s := newScreen(t, nil)
	s.start()
	s.press("3")
	other := s.second()
	ctx := context.Background()
	for _, name := range []string{"alpha.example.org", "beta.example.org", "gamma.example.org"} {
		if _, err := other.Inventory.CreateEndpoint(ctx, inventory.EndpointInput{Host: name, Port: 443, Enabled: true}, true); err != nil {
			t.Fatal(err)
		}
	}
	s.hides("alpha.example.org")
	if _, command := s.m.Update(tickMsg{}); command == nil {
		t.Fatal("the refresh timer stopped")
	}
	s.shows("alpha.example.org", "beta.example.org", "gamma.example.org", "Updated by another window")
	s.press("down")
	s.shows("Row 2 of 3")
	selected, _ := s.m.pages[app.ViewEndpoints].selected()
	if _, err := other.Inventory.CreateEndpoint(ctx, inventory.EndpointInput{Host: "aaa.example.org", Port: 443, Enabled: true}, true); err != nil {
		t.Fatal(err)
	}
	s.m.Update(tickMsg{})
	if after, _ := s.m.pages[app.ViewEndpoints].selected(); after.ID != selected.ID {
		t.Fatalf("the selection moved from %v to %v after an external change", selected.Cells, after.Cells)
	}
	s.shows("Row 3 of 4")
	s.m.Update(tickMsg{})
	s.press("r")
	if _, err := other.Settings.Set(ctx, settings.KeyTheme, "green"); err != nil {
		t.Fatal(err)
	}
	s.m.Update(tickMsg{})
	if s.m.tokens.Name != "green" {
		t.Fatalf("a theme chosen in another window is not in use: %s", s.m.tokens.Name)
	}
	s.m.Update(housekeepMsg{done: app.Housekeeping{Report: app.CheckReport{Checked: 2, Failed: 1}}})
	s.shows("Automatic check: checked 2 endpoints: 1 failed")
	s.m.Update(housekeepMsg{done: app.Housekeeping{Alerts: 3}})
	s.shows("3 new expiry alerts")
	s.m.Update(housekeepMsg{err: errors.New("disk full")})
	s.shows("The automatic check stopped early: disk full")
}

func TestImportExportAndReports(t *testing.T) {
	s := newScreen(t, nil)
	s.start()
	s.clock.Set(time.Now().UTC().Truncate(time.Second))
	source := filepath.Join(s.home, "in.csv")
	if err := os.WriteFile(source, []byte("host,port,note\nalpha.example.org,,first\nbad host,,x\n"), 0o600); err != nil {
		t.Fatal(err)
	}
	s.press("3", "i")
	s.shows("Endpoints, Import list", "File", "I am authorized to test this host")
	s.press("ctrl+s")
	s.shows("Not saved:", "File is missing")
	s.write(source)
	s.press("tab", "right", "ctrl+s")
	s.shows("Import list", "would import 1 endpoint, 0 already monitored, 1 rejected", "alpha.example.org:443", "row 3", "Press Enter to import list")
	if count, _ := s.m.app.Inventory.EndpointCount(context.Background()); count != 0 {
		t.Fatal("the preview already imported rows")
	}
	s.press("enter")
	s.shows("Imported 1 endpoint, 0 already monitored, 1 rejected.", "row 3")
	s.press("esc")
	s.shows("alpha.example.org:443", "Row 1 of 1")
	target := filepath.Join(s.home, "out.json")
	s.press("o")
	s.shows("Endpoints, Export list", "certwatch-endpoints.csv")
	s.clear()
	s.write(target)
	s.press("ctrl+s")
	s.shows("Wrote 1 endpoint to " + target)
	s.press("o")
	s.clear()
	s.write(target)
	s.press("ctrl+s")
	s.shows("Not saved:", "already exists")
	s.press("tab", "right", "ctrl+s")
	s.shows("Wrote 1 endpoint to " + target)
	pem := testkit.PEMFile(t, s.clock.Now(), "file.fixture.test", 5)
	s.press("2", "i")
	s.write(pem)
	s.press("ctrl+s")
	s.shows("Import PEM file", "would import 1 certificate, 0 already tracked", "file.fixture.test")
	s.press("enter")
	s.shows("Result", "Imported 1 certificate, 0 already tracked.", "file.fixture.test", "7 Alerts (1)")
	s.press("esc")
	s.shows("file.fixture.test", "5 days", "Row 1 of 1")
	s.press("enter")
	s.shows("Source", "imported")
	s.press("esc")
	saved := filepath.Join(s.home, "one.pem")
	s.press("o")
	s.clear()
	s.write(saved)
	s.press("ctrl+s")
	s.shows("Wrote the certificate file.fixture.test")
	if written, err := os.ReadFile(saved); err != nil || !strings.HasPrefix(string(written), "-----BEGIN CERTIFICATE-----") {
		t.Fatalf("saved certificate: %v", err)
	}
	s.press("8")
	s.shows("No reports yet")
	s.press("n")
	s.shows("Reports, Generate report", "Content", "‹ Certificate inventory ›", "Format", "‹ Markdown ›")
	s.press("right", "tab", "right", "ctrl+s")
	s.shows("Generated report 1: Certificates expiring within 90 days, 1 row.", "csv", "Row 1 of 1")
	s.press("enter")
	s.shows("file.fixture.test", "Suggested file name")
	s.press("esc", "o")
	report := filepath.Join(s.home, "expiry.csv")
	s.clear()
	s.write(report)
	s.press("ctrl+s")
	s.shows("Wrote the report to " + report)
	if written, err := os.ReadFile(report); err != nil || !strings.Contains(string(written), "file.fixture.test") {
		t.Fatalf("saved report: %v", err)
	}
	s.press("d")
	s.shows("Reports, Remove", "Remove the report \"Certificates expiring within 90 days\"", "Saved files stay.")
	s.press("enter")
	s.shows("Removed report 1.", "No reports yet")
	s.press("2", "d")
	s.shows("Stop tracking the imported certificate file.fixture.test")
	s.press("y")
	s.shows("Stopped tracking file.fixture.test.", "No certificates yet")
}

func TestAlertsAndRules(t *testing.T) {
	s := newScreen(t, nil)
	s.press("l", "7")
	s.shows("▌ Inbox", "Showing all alerts · 9 unread", "SEVERITY", "Unread", "Certificate changed on", "endpoint 9")
	s.press("m")
	s.shows("Marked alert", "8 unread", "Read")
	s.press("enter")
	s.shows(meta.AppName+" · Endpoints", "portal.demo.invalid:443", "Esc back to the table", "This endpoint is demo data. It is never contacted.")
	s.press("7", "u")
	s.shows("Showing unread only")
	s.press("M")
	s.shows("Marked 8 alerts as read", "No unread alerts. Press u to show all of them.")
	s.press("u", "right")
	s.shows("▌ Rules", "EVENT", "MIN SEVERITY", "default", "none")
	s.press("e")
	s.shows("Rule for ", "Minimum severity", "Quiet hours start", "De-duplication")
	s.press("tab", "tab")
	s.write("22:00")
	s.press("ctrl+s")
	s.shows("Not saved:", "both a start and an end")
	s.press("tab")
	s.write("07:00")
	s.press("tab")
	s.clear()
	s.write("soon")
	s.press("ctrl+s")
	s.shows("whole number of minutes")
	s.clear()
	s.write("15")
	s.press("ctrl+s")
	s.shows("Saved the rule for", "22:00-07:00", "15 min", "changed")
	s.press("u")
	s.shows("back to its defaults")
	s.hides("22:00-07:00")
	s.press("left")
	s.shows("▌ Inbox")
}

func TestSettings(t *testing.T) {
	s := newScreen(t, nil)
	s.start()
	s.env[settings.EnvName(settings.KeyDensity)] = "compact"
	s.press("r", "0")
	s.shows("Appearance", "Mode", "Color theme", "Neutral slate", "default", "Light, dark or follow the operating system.", "overridden")
	s.press("down", "down", "down")
	s.shows("Overridden by the environment variable "+settings.EnvName(settings.KeyDensity), `The saved value is "comfortable"`)
	s.press("up", "up", "up")
	before := s.m.tokens.Palette.Accent
	s.press("down", "enter")
	s.shows("Appearance, Color theme", "‹ Neutral slate ›")
	s.press("right", "right", "enter")
	s.shows("Saved Color theme", "Green", "stored")
	if s.m.tokens.Name != "green" || s.m.tokens.Palette.Accent == before {
		t.Fatalf("the theme in use did not change: %+v", s.m.tokens.Name)
	}
	reopened := New(context.Background(), Options{App: s.second()})
	if reopened.tokens.Name != "green" {
		t.Fatal("the theme was not saved")
	}
	s.press("u")
	s.shows("Color theme is back to its default", "Neutral slate")
	s.press("down", "enter")
	s.shows("Custom accent")
	s.write("red")
	s.press("enter")
	s.shows("Not saved:", "#RRGGBB")
	s.clear()
	s.write("#FFFF00")
	s.press("enter")
	s.shows("was adjusted")
	s.press("down", "enter", "right", "enter")
	s.shows("Saved Density, but the environment variable " + settings.EnvName(settings.KeyDensity) + " still takes precedence")
	s.press("down", "enter")
	s.clear()
	s.write("9000")
	s.press("enter")
	s.shows("Not saved:", "80", "200")
	s.press("esc")
	for range 40 {
		s.press("down")
	}
	s.shows("Open with", "Ask every time", "Which front end opens")
	s.press("enter")
	s.shows("Open with: Dashboard window", "saved")
	if s.m.app.FrontEnd() != launcher.ModeGUI {
		t.Fatal("the front end was not remembered")
	}
	s.press("enter")
	s.shows("Open with: Terminal UI")
	s.press("u")
	s.shows("Open with: Ask every time")
}

func TestSettingsActions(t *testing.T) {
	s := newScreen(t, nil)
	s.press("l", "0")
	move := func(label string) {
		t.Helper()
		for index, row := range s.m.set.rows {
			if row.label == label {
				s.m.set.cursor = index
				return
			}
		}
		t.Fatalf("no settings row %q", label)
	}
	move("Remove demo data")
	s.shows("23 demo rows")
	s.press("enter")
	s.shows("This removes 23 demo endpoints and certificates with their alerts. Entries you added yourself stay.")
	s.press("y")
	s.shows("Removed 23 demo endpoints and certificates", "0 demo rows")
	s.press("enter")
	s.shows("There is no demo data to remove")
	move("Notifier at sign-in")
	s.shows("Off")
	entry := filepath.Join(s.home, ".config", "autostart", meta.Slug+"-notifier.desktop")
	s.press("enter")
	s.shows("This writes "+entry, "needs no administrator rights")
	s.press("n")
	if _, err := os.Stat(entry); !errors.Is(err, os.ErrNotExist) {
		t.Fatal("cancelling wrote the entry")
	}
	s.press("enter", "y")
	s.shows("The notifier starts the next time you sign in", "Entry: "+entry)
	s.press("enter")
	s.shows("This removes " + entry)
	s.press("y")
	s.shows("no longer starts at sign-in")
	move("Database location")
	s.shows(filepath.Join(".local", "share", meta.Slug, meta.Slug+".db"))
	s.press("enter")
	s.shows("Database location", "New location", "Copy my data there and use it")
	s.press("ctrl+s")
	s.shows("Not saved:")
	target := filepath.Join(s.home, "elsewhere", "data.db")
	s.write(target)
	s.press("ctrl+s")
	s.shows("Copy the current database to "+target, "Press y to continue")
	s.press("y")
	s.shows("Now using " + target)
	if s.m.app.Location().Path != target {
		t.Fatalf("location: %s", s.m.app.Location().Path)
	}
	for label, value := range map[string]string{"Automatic check interval": "0", "Expiry thresholds": "30,14,7,1", "Endpoint table columns": "expires,days,checked,tls,state", "Endpoint table sort": "endpoint:asc", "Additional trusted roots": "(empty)"} {
		move(label)
		s.shows(label, value)
	}
	move("Expiry thresholds")
	s.press("enter")
	s.clear()
	s.write("60, 10")
	s.press("enter")
	s.shows("Saved Expiry thresholds", "60,10")
	move("Endpoint table columns")
	s.press("enter")
	s.clear()
	s.write("status,owner")
	s.press("enter")
	s.shows("Not saved:", "not a column of this table")
	s.clear()
	s.write("status,tls")
	s.press("enter")
	s.shows("Saved Endpoint table columns", "endpoint,status,tls")
	move("Endpoint table sort")
	s.press("enter")
	s.clear()
	s.write("expires:desc")
	s.press("enter")
	s.shows("Saved Endpoint table sort")
	s.addEndpoint("columns.example.org")
	s.press("r", "3")
	s.shows("ENDPOINT", "STATUS", "TLS", "columns.example.org:443")
	s.hides("LAST CHECK", "CERTIFICATE")
}

func TestAboutGuideAndLinks(t *testing.T) {
	s := newScreen(t, nil)
	s.start()
	s.press("9")
	s.shows("▌Overview", meta.AppName+" "+meta.Version, "License: "+meta.License, "Source Code", meta.RepositoryURL, "Found a bug? Please report it on GitHub.", "connects only to the endpoints you added", "The application itself sends nothing", "to GitHub.")
	s.press("enter")
	if len(s.opened) != 1 || s.opened[0] != meta.RepositoryURL {
		t.Fatalf("opened: %v", s.opened)
	}
	s.press("down", "enter")
	s.shows("Report a bug", "prefilled with exactly this text and nothing else", "Operating system:", "Press Enter to open the report form in the browser")
	if strings.Contains(strings.Join(s.m.preview.lines, "\n"), s.home) {
		t.Fatal("the bug report preview contains a local path")
	}
	if len(s.opened) != 1 {
		t.Fatal("the browser opened before the preview was confirmed")
	}
	s.press("esc")
	s.shows("Cancelled, nothing was changed")
	s.press("enter", "enter")
	if len(s.opened) != 2 || !strings.HasPrefix(s.opened[1], meta.RepositoryURL+"/issues/new?") {
		t.Fatalf("opened: %v", s.opened)
	}
	s.press("right")
	s.shows("▌Usage guide", "Getting started", "Troubleshooting", "press / to search")
	s.press("/")
	s.write("quiet hours")
	s.shows("Search: quiet hours", "Alerts")
	s.hides("Getting started")
	s.press("enter", "enter")
	s.shows("minimum severity", "Esc back to the contents")
	s.press("esc", "/")
	s.write("zzzzqq")
	s.shows("No section mentions these words")
	s.press("esc", "right")
	s.shows("▌Shortcuts", "KEY", "ACTION")
	for _, shortcut := range app.Shortcuts() {
		s.shows(shortcut.Action)
	}
	s.press("right")
	s.shows("▌Diagnostics", "Schema version", "Integrity check", "ok", "Diagnostics stay on this computer")
	s.press("right")
	s.shows("▌Licenses", "MIT License")
	s.press("pgdown", "right")
	s.shows("▌Overview")
	failing := newScreen(t, func(options *Options) { options.OpenURL = nil })
	failing.start()
	failing.press("9", "enter")
	failing.shows("Copy this link: " + meta.RepositoryURL)
}

func TestMenuAndSwitchToDashboard(t *testing.T) {
	s := newScreen(t, nil)
	s.start()
	s.press(":")
	s.shows("Menu", "Open dashboard", "Usage guide", "Keyboard shortcuts", "Quit")
	s.press("down", "down", "enter")
	s.shows("▌Shortcuts")
	s.press("f10", "esc")
	s.hides("Open dashboard")
	s.press(":")
	if command := s.press("enter"); command == nil || s.m.SwitchTo() != launcher.ModeGUI {
		t.Fatalf("the menu did not hand over to the dashboard: %q", s.m.SwitchTo())
	}
	plain := newScreen(t, func(options *Options) { options.HasDashboard = false })
	plain.start()
	plain.press(":")
	plain.shows("Not available: this build has no dashboard")
	if command := plain.press("enter"); command != nil || plain.m.SwitchTo() != "" {
		t.Fatal("a build without dashboard tried to open it")
	}
	plain.shows("This build has no dashboard")
}

func TestStylesFollowTheTerminalProfile(t *testing.T) {
	s := newScreen(t, nil)
	s.start()
	if s.m.st.terminal.Profile != theme.DetectProfile(func(string) string { return "" }, false) {
		t.Fatal("unexpected default profile")
	}
	colored := s.m.st.terminal.Accent.Foreground
	if colored == "" {
		t.Fatal("the accent has no color on a color terminal")
	}
	s.env["NO_COLOR"] = "1"
	s.press("r")
	if s.m.st.terminal.Accent.Foreground != "" || s.m.st.terminal.Selected.Background != "" || !s.m.st.terminal.Selected.Reverse {
		t.Fatalf("NO_COLOR still uses colors: %+v", s.m.st.terminal)
	}
	delete(s.env, "NO_COLOR")
	if _, err := s.m.app.Settings.Set(context.Background(), settings.KeyMonochrome, "true"); err != nil {
		t.Fatal(err)
	}
	s.press("r")
	if s.m.st.terminal.Accent.Foreground != "" {
		t.Fatal("the monochrome setting still uses colors")
	}
	s.press("3")
	s.shows("Search: none", "No endpoints yet")
}

func TestEveryTerminalCapabilityIsReachable(t *testing.T) {
	s := newScreen(t, nil)
	s.press("l")
	var help strings.Builder
	for index, item := range app.Navigation() {
		s.press(string(rune('0' + (index+1)%10)))
		help.WriteString(item.Label + " ")
		help.WriteString(s.m.View())
		help.WriteString(s.m.hints() + " ")
		for range 4 {
			s.press("pgdown")
			help.WriteString(s.m.View())
		}
	}
	s.press("7", "right")
	help.WriteString(s.m.View() + s.m.hints() + " ")
	s.press(":")
	help.WriteString(s.m.View())
	text := help.String()
	for _, capability := range app.Capabilities() {
		if capability.Terminal == app.NotApplicable || capability.Terminal == "First-start screen" {
			continue
		}
		parts := strings.Split(capability.Terminal, ", ")
		last := parts[len(parts)-1]
		if len(last) <= 2 || strings.Contains(last, " and ") {
			for _, key := range strings.Split(last, " and ") {
				if !strings.Contains(text, key+" ") {
					t.Errorf("capability %s: key %q is not in any hint line", capability.ID, key)
				}
			}
			continue
		}
		if !strings.Contains(text, last) {
			t.Errorf("capability %s: %q is not on any screen", capability.ID, last)
		}
	}
}

func TestLongHintLinesKeepTheWayOut(t *testing.T) {
	s := newScreen(t, nil)
	s.start()
	s.press("3", "esc")
	s.m.notice = ""
	full := s.m.hints()
	if !strings.Contains(full, "o export list") || lipgloss.Width(full) <= 110 {
		t.Fatalf("the endpoint hints are expected to be longer than the test terminal: %q", full)
	}
	s.shows("Enter details", "… · ? guide · q quit")
	for _, line := range strings.Split(s.m.View(), "\n") {
		if lipgloss.Width(line) > 110 {
			t.Fatalf("line is wider than the terminal: %q", line)
		}
	}
	if short := shortHints(full, 400); short != full {
		t.Fatalf("a hint line that fits was changed: %q", short)
	}
	if tiny := shortHints(full, 10); tiny != "… · ? guide · q quit" {
		t.Fatalf("tiny width: %q", tiny)
	}
}

func TestLongNoticesWrapInsteadOfLosingTheirEnd(t *testing.T) {
	s := newScreen(t, nil)
	s.start()
	s.m.Update(tea.WindowSizeMsg{Width: 70, Height: 20})
	ending := "the-last-word-of-the-notice"
	s.m.say(noticeSuccess, "Wrote 2 endpoints to /a/rather/long/folder/name/that/does/not/fit/on/one/line/of/a/narrow/terminal/%s", ending)
	s.shows(ending)
	lines := strings.Split(s.m.View(), "\n")
	if len(lines) != 19 {
		t.Fatalf("the screen has %d lines with a wrapped notice", len(lines))
	}
	for _, line := range lines {
		if lipgloss.Width(line) > 70 {
			t.Fatalf("line wider than the terminal: %q", line)
		}
	}
	s.m.say(noticeInfo, "%s", strings.Repeat("word ", 200))
	if lines := strings.Split(s.m.View(), "\n"); len(lines) != 19 || !strings.Contains(lines[len(lines)-1], "…") {
		t.Fatalf("a very long notice is not limited to %d lines", maxNoticeLines)
	}
	s.press("r")
	if lines := strings.Split(s.m.View(), "\n"); len(lines) != 19 {
		t.Fatalf("the screen has %d lines after the notice went away", len(lines))
	}
}

func TestEveryViewActionKeyStartsItsAction(t *testing.T) {
	s := newScreen(t, nil)
	s.press("l")
	paths := map[string]string{}
	for _, capability := range app.Capabilities() {
		paths[capability.CLI] = capability.Terminal
	}
	for index, item := range app.Navigation() {
		view, found := app.LookupView(item.ID)
		if !found {
			continue
		}
		digit := string(rune('0' + (index+1)%10))
		for _, action := range view.Actions {
			if want := view.Title + ", " + action.Key; paths[view.ID+" "+action.ID] != want {
				t.Errorf("%s %s: the capability table says %q, the key is %q", view.ID, action.ID, paths[view.ID+" "+action.ID], want)
			}
			s.press("esc", "esc", digit)
			s.m.notice = ""
			if !strings.Contains(s.m.hints(), action.Key+" "+strings.ToLower(action.Label)) {
				t.Errorf("%s: the hint line does not offer %q for %s: %s", view.ID, action.Key, action.Label, s.m.hints())
			}
			command := s.press(action.Key)
			if s.m.busy != "" {
				s.finish(command)
			}
			title := view.Title + ", " + action.Label
			if !strings.Contains(s.m.View(), title) && s.m.notice == "" {
				t.Errorf("%s: pressing %q neither opened %q nor reported anything:\n%s", view.ID, action.Key, title, s.m.View())
			}
		}
	}
	if len(paths) == 0 {
		t.Fatal("no capabilities")
	}
}
