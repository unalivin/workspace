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

	"github.com/NexzaDev/templateapp/internal/app"
	"github.com/NexzaDev/templateapp/internal/launcher"
	"github.com/NexzaDev/templateapp/internal/meta"
	"github.com/NexzaDev/templateapp/internal/notify"
	"github.com/NexzaDev/templateapp/internal/records"
	"github.com/NexzaDev/templateapp/internal/settings"
	"github.com/NexzaDev/templateapp/internal/testkit"
	"github.com/NexzaDev/templateapp/internal/theme"
	"github.com/NexzaDev/templateapp/internal/userconfig"
)

type screen struct {
	t      *testing.T
	m      *Model
	home   string
	env    map[string]string
	opened []string
	clock  *testkit.Clock
}

func (s *screen) appOptions() app.Options {
	return app.Options{
		Environment: userconfig.Environment{Getenv: func(key string) string { return s.env[key] }, HomeDir: func() (string, error) { return s.home, nil }, GOOS: "linux"},
		Now:         s.clock.Now,
	}
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
	s.shows("Welcome to "+meta.AppName, "A new database was created at", "Press l to load labeled demo data")
	s.press("l")
	s.shows("Added 6 demo records", "What needs attention", "Open records            4", "Urgent records          2", "Unread notifications    2", "Demo data is loaded: 6 sample records", "3 Notifications (2)", "▌1 Overview")
	s.press("l")
	s.shows("already loaded")
	reopened := New(context.Background(), Options{App: s.second()})
	if reopened.welcome {
		t.Fatal("the first-start screen appears again for an existing database")
	}
	empty := newScreen(t, nil)
	empty.start()
	empty.shows("No records yet", "press l to load labeled demo data")
}

func TestNavigationAndFrame(t *testing.T) {
	s := newScreen(t, nil)
	s.start()
	for index, item := range app.Navigation() {
		s.press(string(rune('1' + index)))
		s.shows(meta.AppName+" · "+item.Label, "▌"+string(rune('1'+index))+" "+item.Label)
	}
	s.press("tab")
	s.shows("▌1 Overview")
	s.press("shift+tab")
	s.shows("▌5 About")
	s.press(",")
	s.shows("▌4 Settings")
	s.press("b")
	s.hides("4 Settings")
	if value, _ := s.m.app.Settings.Get(context.Background(), settings.KeySidebarCollapsed); value.Raw != "true" {
		t.Fatal("the collapsed sidebar was not saved")
	}
	s.press("b")
	s.shows("▌4 Settings")
	s.press("?")
	s.shows("▌Usage guide", "Getting started")
	s.press("f1", "r")
	s.shows("Refreshed")
	s.m.Update(tea.WindowSizeMsg{Width: 40, Height: 10})
	s.shows("needs a terminal of at least 60 columns and 16 rows", "This one is 40 by 10")
	s.m.Update(tea.WindowSizeMsg{Width: 60, Height: 16})
	for _, line := range strings.Split(s.m.View(), "\n") {
		if len([]rune(line)) > 60 {
			t.Fatalf("line wider than the terminal: %q", line)
		}
	}
	if len(strings.Split(s.m.View(), "\n")) > 16 {
		t.Fatalf("view taller than the terminal:\n%s", s.m.View())
	}
	if command := s.press("q"); command == nil || s.m.View() != "" {
		t.Fatal("q did not quit")
	}
}

func TestRecordsWorkflow(t *testing.T) {
	s := newScreen(t, nil)
	s.start()
	s.press("2")
	s.shows("No records yet. Press n to add the first record")
	s.press("n")
	s.shows("Add record", "Name", "Status", "Severity", "Note", "Enabled", "Required, unique")
	s.press("ctrl+s")
	s.shows("Not saved:", "the name is empty")
	s.write("Core switch")
	s.press("tab", "tab", "right", "right", "right", "tab")
	s.write("rack 4")
	s.press("ctrl+s")
	s.shows("Added record 1: Core switch", "▌ 1", "Core switch", theme.SeverityHigh.Glyph()+" High", "○ Open", "Enabled", "Row 1 of 1")
	s.press("n")
	s.write("Edge router")
	s.press("enter", "enter", "enter", "enter", "enter")
	s.shows("Added record 2: Edge router", "of 2")
	s.press("n")
	s.write("Core switch")
	s.press("ctrl+s")
	s.shows("already exists")
	s.press("esc")
	s.shows("Cancelled, nothing was changed")
	s.press("enter")
	s.shows("Record 2: Edge router", "Revision 1", "No note.")
	s.press("esc", "e")
	s.shows("Edit record 2")
	s.press("tab", "right", "ctrl+s")
	s.shows("Updated record 2", "◐ In progress")
	s.press("x")
	s.shows("Record 2 is disabled", "Disabled: shown", "Disabled")
	s.press("x")
	s.shows("Record 2 is enabled")
	s.press("/")
	s.write("rack")
	s.shows("Search: rack", "Core switch", "Type to search")
	s.hides("Edge router")
	s.press("enter")
	s.shows("Row 1 of 1")
	s.press("esc")
	s.shows("Search: none", "Edge router")
	s.press("/")
	s.write("zzz")
	s.press("enter")
	s.shows("No records match these filters. Press Esc to clear them.")
	s.press("esc", "f")
	s.shows("Status: Open")
	s.hides("Edge router")
	s.press("f")
	s.shows("Status: In progress", "Edge router")
	s.press("esc", "v", "v", "v", "v")
	s.shows("Severity: High", "Core switch")
	s.press("esc", "s")
	s.shows("Sort: name descending")
	s.press("S")
	s.shows("Sort: name ascending")
	s.press("down", "d")
	s.shows("Remove record", "This removes record 2 (Edge router) permanently.", "Press y to remove it")
	s.press("n")
	s.shows("Cancelled, nothing was changed", "Edge router")
	s.press("d", "y")
	s.shows("Removed record 2", "Row 1 of 1")
	s.hides("Edge router")
	if _, err := s.m.app.Records.Get(context.Background(), 2); !errors.Is(err, records.ErrNotFound) {
		t.Fatalf("the record still exists: %v", err)
	}
}

func TestRemoveWithoutConfirmationWhenTheSettingIsOff(t *testing.T) {
	s := newScreen(t, nil)
	s.start()
	ctx := context.Background()
	if _, err := s.m.app.Records.Create(ctx, records.Input{Name: "quick", Status: records.StatusOpen, Severity: theme.SeverityLow, Enabled: true}); err != nil {
		t.Fatal(err)
	}
	if _, err := s.m.app.Settings.Set(ctx, settings.KeyConfirm, "false"); err != nil {
		t.Fatal(err)
	}
	s.press("r", "2", "d")
	s.shows("Removed record 1")
}

func TestConflictingEditKeepsTheForm(t *testing.T) {
	s := newScreen(t, nil)
	s.start()
	ctx := context.Background()
	created, err := s.m.app.Records.Create(ctx, records.Input{Name: "shared", Status: records.StatusOpen, Severity: theme.SeverityLow, Enabled: true})
	if err != nil {
		t.Fatal(err)
	}
	s.press("r", "2", "e")
	s.write(" mine")
	other := s.second()
	changed := created.Input()
	changed.Note = "from the other window"
	if _, err := other.Records.Update(ctx, created.ID, created.Revision, changed); err != nil {
		t.Fatal(err)
	}
	s.m.Update(tickMsg{})
	s.shows("Edit record 1", "shared mine")
	s.press("ctrl+s")
	s.shows("Not saved:", "another window changed this record", "shared mine")
	stored, _ := s.m.app.Records.Get(ctx, created.ID)
	if stored.Name != "shared" || stored.Note != "from the other window" {
		t.Fatalf("the other window's change was overwritten: %+v", stored)
	}
	s.press("esc", "e")
	s.write(" mine")
	s.press("ctrl+s")
	s.shows("Updated record 1", "shared mine")
}

func TestExternalChangesAppearOnTheNextTick(t *testing.T) {
	s := newScreen(t, nil)
	s.start()
	s.press("2")
	other := s.second()
	ctx := context.Background()
	for _, name := range []string{"alpha", "beta", "gamma"} {
		if _, err := other.Records.Create(ctx, records.Input{Name: name, Status: records.StatusOpen, Severity: theme.SeverityCritical, Enabled: true}); err != nil {
			t.Fatal(err)
		}
	}
	s.hides("alpha")
	if _, command := s.m.Update(tickMsg{}); command == nil {
		t.Fatal("the refresh timer stopped")
	}
	s.shows("alpha", "beta", "gamma", "Updated by another window", "3 Notifications (3)")
	s.press("s", "S", "down")
	s.shows("Row 2 of 3")
	selected, _ := s.m.rec.selected()
	if _, err := other.Records.Create(ctx, records.Input{Name: "aaa first", Status: records.StatusOpen, Severity: theme.SeverityInfo, Enabled: true}); err != nil {
		t.Fatal(err)
	}
	s.m.Update(tickMsg{})
	if after, _ := s.m.rec.selected(); after.ID != selected.ID {
		t.Fatalf("the selection moved from %s to %s after an external change", selected.Name, after.Name)
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
}

func TestImportAndExport(t *testing.T) {
	s := newScreen(t, nil)
	s.start()
	source := filepath.Join(s.home, "in.csv")
	if err := os.WriteFile(source, []byte("name,severity,enabled\nalpha,high,true\nbeta,nope,true\ngamma,low,false\n"), 0o600); err != nil {
		t.Fatal(err)
	}
	s.press("2", "i")
	s.shows("Import records", "Nothing is written until you confirm the preview")
	s.press("ctrl+s")
	s.shows("give the path of the file to import")
	s.write(filepath.Join(s.home, "missing.csv"))
	s.press("ctrl+s")
	s.shows("cannot open the import file")
	s.clear()
	s.write(source)
	s.press("ctrl+s")
	s.shows("Import preview", "Dry run of", "would have created 2, updated 0, unchanged 0, rejected 1", "rejected row 3", "Press Enter to import these rows")
	if count, _ := s.m.app.Records.Count(context.Background()); count != 0 {
		t.Fatal("the preview wrote records")
	}
	s.press("esc")
	s.shows("Cancelled, nothing was changed")
	s.press("i")
	s.write(source)
	s.press("ctrl+s", "enter")
	s.shows("Import finished: created 2", "alpha", "gamma", "Disabled: shown")
	target := filepath.Join(s.home, "out.json")
	s.press("o")
	s.shows("Export records", "readable only by you")
	s.clear()
	s.write(target)
	s.press("ctrl+s")
	s.shows("Exported 2 records to " + target)
	content, err := os.ReadFile(target)
	if err != nil || !strings.Contains(string(content), `"name": "alpha"`) {
		t.Fatalf("export: %q, %v", content, err)
	}
	s.press("o")
	s.clear()
	s.write(target)
	s.press("ctrl+s")
	s.shows("already exists", "Overwrite it")
	s.press("tab", "tab", "right", "ctrl+s")
	s.shows("Exported 2 records")
	s.press("o")
	s.clear()
	s.write(filepath.Join(s.home, "out.txt"))
	s.press("ctrl+s")
	s.shows("choose csv or json")
}

func TestNotificationsAndRules(t *testing.T) {
	s := newScreen(t, nil)
	s.press("l", "3")
	s.shows("▌ Inbox", "Showing all notifications · 2 unread", "SEVERITY", "Unread", "A record needs attention", "record ")
	s.hides("Demo:")
	s.press("m")
	s.shows("Marked notification", "1 unread", "Read")
	s.press("u")
	s.shows("Showing unread only")
	s.press("M")
	s.shows("Marked 1 notifications as read", "No unread notifications. Press u to show all of them.")
	s.press("u", "enter")
	s.shows(meta.AppName+" · Records", "Record ", "Demo:")
	s.press("3", "right")
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
	s.press("r", "4")
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
	s.press("l", "4")
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
	s.shows("6 demo records")
	s.press("enter")
	s.shows("This removes 6 demo records. Your own records are not touched.")
	s.press("y")
	s.shows("Removed 6 demo records", "0 demo records")
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
	overridden := newScreen(t, func(options *Options) {})
	overridden.start()
	_ = overridden
}

func TestAboutGuideAndLinks(t *testing.T) {
	s := newScreen(t, nil)
	s.start()
	s.press("5")
	s.shows("▌Overview", meta.AppName+" "+meta.Version, "License: "+meta.License, "Source Code", meta.RepositoryURL, "Found a bug? Please report it on GitHub.", "Nothing is sent by the application.")
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
	s.shows("Search: quiet hours", "Notifications")
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
	failing.press("5", "enter")
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
	s.press("2")
	s.shows("Search: none", "No records yet")
}

func TestEveryTerminalCapabilityIsReachable(t *testing.T) {
	s := newScreen(t, nil)
	s.press("l")
	var help strings.Builder
	for index := range app.Navigation() {
		s.press(string(rune('1' + index)))
		help.WriteString(s.m.View())
		help.WriteString(s.m.hints())
		for range 3 {
			s.press("pgdown")
			help.WriteString(s.m.View())
		}
	}
	s.press("3", "right")
	help.WriteString(s.m.View() + s.m.hints())
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

func TestRecordTableColumnsAndSortAreSavedSettings(t *testing.T) {
	ctx := context.Background()
	s := newScreen(t, nil)
	s.start()
	other := s.second()
	for _, name := range []string{"Alpha", "Bravo"} {
		if _, err := other.Records.Create(ctx, records.Input{Name: name, Status: records.StatusOpen, Severity: theme.SeverityLow, Enabled: true}); err != nil {
			t.Fatal(err)
		}
		s.clock.Advance(time.Minute)
	}
	s.press("2", "r")
	s.shows("ID", "NAME", "STATUS", "SEVERITY", "UPDATED", "STATE", "Sort: updated descending")
	s.press("c")
	s.shows("Columns of the record table", "ID", "Status", "Severity", "Updated", "State", "Shown")
	s.hides("Name")
	s.press("right", "tab", "tab", "right", "ctrl+s")
	s.hides("SEVERITY", "Columns of the record table")
	s.shows("NAME", "STATUS", "UPDATED", "STATE", "Alpha")
	if value, err := other.Settings.Get(ctx, settings.KeyRecordColumns); err != nil || value.Raw != "name,status,updated,state" {
		t.Fatalf("columns were not saved: %q %v", value.Raw, err)
	}
	s.press("s", "S")
	s.shows("Sort: name ascending")
	if value, err := other.Settings.Get(ctx, settings.KeyRecordSort); err != nil || value.Raw != "name:asc" {
		t.Fatalf("sort was not saved: %q %v", value.Raw, err)
	}
	if _, err := other.Settings.Set(ctx, settings.KeyRecordColumns, "id,name"); err != nil {
		t.Fatal(err)
	}
	if _, err := other.Settings.Set(ctx, settings.KeyRecordSort, "name:desc"); err != nil {
		t.Fatal(err)
	}
	s.press("r")
	s.shows("ID", "NAME", "Sort: name descending")
	s.hides("STATUS", "UPDATED", "STATE")
	if first, found := s.m.rec.selected(); !found || s.m.rec.list[0].Name != "Bravo" {
		t.Fatalf("rows are not sorted by the saved order: %+v %v", first, found)
	}
	reopened := newScreen(t, func(options *Options) { options.App = other })
	reopened.press("2")
	reopened.shows("Sort: name descending")
	reopened.hides("STATUS")
	s.env[settings.EnvName(settings.KeyRecordSort)] = "severity:asc"
	s.press("r", "s")
	s.shows("Sort: severity ascending", "Sort order saved, but the environment variable "+settings.EnvName(settings.KeyRecordSort)+" still takes precedence")
}

func TestLongHintLinesKeepTheWayOut(t *testing.T) {
	s := newScreen(t, nil)
	s.start()
	s.press("2", "esc")
	s.m.notice = ""
	full := s.m.hints()
	if !strings.Contains(full, "c columns") || lipgloss.Width(full) <= 110 {
		t.Fatalf("the record hints are expected to be longer than the test terminal: %q", full)
	}
	s.shows("n add", "… · ? guide · q quit")
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
	s.press("5", "right", "right")
	s.shows("Records: choose the table columns", "Records: next sort column, reverse the order")
}

func TestLongNoticesWrapInsteadOfLosingTheirEnd(t *testing.T) {
	s := newScreen(t, nil)
	s.start()
	s.m.Update(tea.WindowSizeMsg{Width: 70, Height: 20})
	ending := "the-last-word-of-the-notice"
	s.m.say(noticeSuccess, "Exported 2 records to /a/rather/long/folder/name/that/does/not/fit/on/one/line/of/a/narrow/terminal/%s", ending)
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
