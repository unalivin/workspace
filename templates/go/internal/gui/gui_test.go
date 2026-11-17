package gui

import (
	"context"
	"errors"
	"flag"
	"image/png"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"fyne.io/fyne/v2"
	"fyne.io/fyne/v2/container"
	"fyne.io/fyne/v2/test"
	fynetheme "fyne.io/fyne/v2/theme"
	"fyne.io/fyne/v2/widget"

	"github.com/unalivin/templateapp/internal/app"
	"github.com/unalivin/templateapp/internal/launcher"
	"github.com/unalivin/templateapp/internal/meta"
	"github.com/unalivin/templateapp/internal/notify"
	"github.com/unalivin/templateapp/internal/records"
	"github.com/unalivin/templateapp/internal/settings"
	"github.com/unalivin/templateapp/internal/testkit"
	"github.com/unalivin/templateapp/internal/theme"
	"github.com/unalivin/templateapp/internal/userconfig"
)

var screenshots = flag.String("screenshots", "", "directory that receives dashboard screenshots")

type desk struct {
	t        *testing.T
	d        *dashboard
	window   fyne.Window
	home     string
	env      map[string]string
	clock    *testkit.Clock
	opened   []string
	terminal int
	closed   []string
}

func (s *desk) second() *app.App {
	s.t.Helper()
	other, err := app.Open(context.Background(), app.Options{
		Environment: userconfig.Environment{Getenv: func(key string) string { return s.env[key] }, HomeDir: func() (string, error) { return s.home, nil }, GOOS: "linux"},
		Now:         s.clock.Now,
	})
	if err != nil {
		s.t.Fatal(err)
	}
	s.t.Cleanup(func() { _ = other.Close() })
	return other
}

func newDesk(t *testing.T, change func(*Options)) *desk {
	t.Helper()
	s := &desk{t: t, home: t.TempDir(), env: map[string]string{}, clock: testkit.NewClock()}
	fyneApp := test.NewApp()
	t.Cleanup(fyneApp.Quit)
	s.window = fyneApp.NewWindow(meta.AppName)
	s.window.Resize(fyne.NewSize(1180, 760))
	options := Options{
		App:          s.second(),
		Getenv:       func(key string) string { return s.env[key] },
		OpenURL:      func(url string) error { s.opened = append(s.opened, url); return nil },
		OpenTerminal: func() error { s.terminal++; return nil },
		Autostart: func() notify.Autostart {
			return notify.Autostart{GOOS: "linux", HomeDir: s.home, Executable: filepath.Join(s.home, "bin", meta.Slug)}
		},
	}
	if change != nil {
		change(&options)
	}
	s.d = newDashboard(context.Background(), fyneApp, s.window, options, func(switchTo string) { s.closed = append(s.closed, switchTo) })
	s.window.Resize(fyne.NewSize(1180, 760))
	return s
}

func walk(object fyne.CanvasObject, visit func(fyne.CanvasObject)) {
	if object == nil {
		return
	}
	visit(object)
	switch typed := object.(type) {
	case *fyne.Container:
		for _, child := range typed.Objects {
			walk(child, visit)
		}
	case *container.Scroll:
		walk(typed.Content, visit)
	case *container.Split:
		walk(typed.Leading, visit)
		walk(typed.Trailing, visit)
	case *container.AppTabs:
		for _, item := range typed.Items {
			visit(widget.NewLabel(item.Text))
			walk(item.Content, visit)
		}
	case *surface:
		walk(typed.content, visit)
	case *widget.Form:
		for _, item := range typed.Items {
			visit(widget.NewLabel(item.Text))
			visit(widget.NewLabel(item.HintText))
			walk(item.Widget, visit)
		}
	case fyne.Widget:
		for _, child := range test.WidgetRenderer(typed).Objects() {
			if _, isWidget := child.(fyne.Widget); isWidget {
				walk(child, visit)
			} else if _, isContainer := child.(*fyne.Container); isContainer {
				walk(child, visit)
			}
		}
	}
}

func (s *desk) texts(visibleOnly bool) string {
	var builder strings.Builder
	collect := func(object fyne.CanvasObject) {
		if visibleOnly && !object.Visible() {
			return
		}
		switch typed := object.(type) {
		case *widget.Label:
			builder.WriteString(typed.Text + "\n")
		case *widget.Button:
			builder.WriteString(typed.Text + "\n")
		case *widget.Check:
			builder.WriteString(typed.Text + "\n")
		case *widget.Entry:
			builder.WriteString(typed.Text + "\n" + typed.PlaceHolder + "\n")
		case *widget.Select:
			builder.WriteString(typed.Selected + "\n")
		case *widget.RichText:
			builder.WriteString(typed.String() + "\n")
		}
	}
	walk(s.window.Content(), collect)
	for _, overlay := range s.window.Canvas().Overlays().List() {
		walk(overlay, collect)
	}
	for _, menu := range s.window.MainMenu().Items {
		builder.WriteString("Menu\n")
		for _, item := range menu.Items {
			builder.WriteString(item.Label + "\n")
		}
	}
	return builder.String()
}

func (s *desk) pageText() string {
	var builder strings.Builder
	walk(s.d.pages[s.d.current].object(), func(object fyne.CanvasObject) {
		if !object.Visible() {
			return
		}
		switch typed := object.(type) {
		case *widget.Label:
			builder.WriteString(typed.Text + "\n")
		case *widget.Button:
			builder.WriteString(typed.Text + "\n")
		case *widget.Check:
			builder.WriteString(typed.Text + "\n")
		}
	})
	return builder.String() + s.d.notice.Text
}

func (s *desk) shows(parts ...string) {
	s.t.Helper()
	text := s.pageText()
	if s.d.form != nil {
		text += s.d.form.title + "\n" + s.d.form.problem.Text
		for _, field := range s.d.form.fields {
			text += field.label + "\n" + field.hint + "\n"
		}
	}
	if s.d.confirm != nil {
		text += s.d.confirm.title + "\n" + s.d.confirm.statement
	}
	for _, part := range parts {
		if !strings.Contains(text, part) {
			s.t.Fatalf("the window lacks %q:\n%s", part, text)
		}
	}
}

func (s *desk) button(label string) *widget.Button {
	s.t.Helper()
	var found *widget.Button
	walk(s.d.pages[s.d.current].object(), func(object fyne.CanvasObject) {
		if button, ok := object.(*widget.Button); ok && button.Text == label && button.Visible() && found == nil {
			found = button
		}
	})
	if found == nil {
		s.t.Fatalf("no visible button %q on %s", label, s.d.current)
	}
	return found
}

func (s *desk) tap(label string) {
	s.t.Helper()
	test.Tap(s.button(label))
}

func (s *desk) menu(label string) {
	s.t.Helper()
	for _, menu := range s.window.MainMenu().Items {
		for _, item := range menu.Items {
			if item.Label == label {
				item.Action()
				return
			}
		}
	}
	s.t.Fatalf("no menu item %q", label)
}

func (s *desk) capture(name string) {
	s.t.Helper()
	if *screenshots == "" {
		return
	}
	if err := os.MkdirAll(*screenshots, 0o755); err != nil {
		s.t.Fatal(err)
	}
	file, err := os.Create(filepath.Join(*screenshots, name+".png"))
	if err != nil {
		s.t.Fatal(err)
	}
	defer file.Close()
	if err := png.Encode(file, s.window.Canvas().Capture()); err != nil {
		s.t.Fatal(err)
	}
}

func (s *desk) start() {
	s.t.Helper()
	if s.d.confirm == nil || s.d.confirm.title != "Welcome to "+meta.AppName {
		s.t.Fatal("the first start did not show the welcome dialog")
	}
	s.d.confirm.cancel()
}

func TestFirstStartAndOverview(t *testing.T) {
	s := newDesk(t, nil)
	s.shows("Welcome to "+meta.AppName, "A new database was created at", "Load demo data")
	s.capture("first-start")
	s.d.confirm.accept()
	s.shows("What needs attention", "Open records", "Urgent records", "Unread notifications", "Disabled records", "Added 6 demo records", "Demo data is loaded: 6 sample records", "Critical", "In progress")
	if s.d.overP.numbers["open"].Text != "4" || s.d.overP.numbers["urgent"].Text != "2" || s.d.overP.numbers["unread"].Text != "2" || s.d.overP.numbers["disabled"].Text != "1" {
		t.Fatalf("numbers: %s %s %s %s", s.d.overP.numbers["open"].Text, s.d.overP.numbers["urgent"].Text, s.d.overP.numbers["unread"].Text, s.d.overP.numbers["disabled"].Text)
	}
	if s.d.buttons[app.NavNotifications].Text != "Notifications (2)" {
		t.Fatalf("badge: %q", s.d.buttons[app.NavNotifications].Text)
	}
	s.capture("overview")
	empty := newDesk(t, nil)
	empty.start()
	empty.shows("No records yet", "Load demo data")
	empty.tap("Load demo data")
	empty.shows("Added 6 demo records")
	again := newDashboard(context.Background(), test.NewApp(), test.NewWindow(nil), Options{App: empty.second()}, nil)
	if again.confirm != nil {
		t.Fatal("the welcome dialog appears again for an existing database")
	}
}

func TestNavigationMenuAndShortcuts(t *testing.T) {
	s := newDesk(t, nil)
	s.start()
	for _, item := range app.Navigation() {
		test.Tap(s.d.buttons[item.ID])
		if s.d.current != item.ID || s.d.title.Text != item.Label || !s.d.pages[item.ID].object().Visible() || s.d.buttons[item.ID].Importance != widget.HighImportance {
			t.Fatalf("category %s did not open", item.ID)
		}
		for _, other := range app.Navigation() {
			if other.ID != item.ID && s.d.pages[other.ID].object().Visible() {
				t.Fatalf("%s is visible while %s is open", other.ID, item.ID)
			}
		}
	}
	test.Tap(s.d.buttons["sidebar"])
	if s.d.buttons[app.NavOverview].Text != "" {
		t.Fatal("the collapsed sidebar still shows labels")
	}
	if value, _ := s.d.app.Settings.Get(context.Background(), settings.KeySidebarCollapsed); value.Raw != "true" {
		t.Fatal("the collapsed sidebar was not saved")
	}
	s.menu("Collapse or expand the sidebar")
	if s.d.buttons[app.NavOverview].Text != "Overview" {
		t.Fatal("the sidebar did not expand")
	}
	s.menu("Usage guide")
	if s.d.current != app.NavAbout || s.d.aboutP.tabs.SelectedIndex() != aboutGuide {
		t.Fatal("the usage guide did not open")
	}
	s.menu("Keyboard shortcuts")
	if s.d.aboutP.tabs.SelectedIndex() != aboutShortcuts {
		t.Fatal("the shortcuts did not open")
	}
	s.menu("Records")
	if s.d.current != app.NavRecords {
		t.Fatal("the View menu did not open Records")
	}
	s.menu("Refresh now")
	s.shows("Refreshed")
	s.menu("Open terminal UI")
	if s.terminal != 1 || len(s.closed) != 1 || s.closed[0] != "" {
		t.Fatalf("terminal windows %d, closed %v", s.terminal, s.closed)
	}
	inTerminal := newDesk(t, func(options *Options) { options.InTerminal = true })
	inTerminal.start()
	inTerminal.menu("Open terminal UI")
	if inTerminal.terminal != 0 || len(inTerminal.closed) != 1 || inTerminal.closed[0] != launcher.ModeTUI {
		t.Fatalf("hand-over: %v", inTerminal.closed)
	}
	none := newDesk(t, func(options *Options) { options.OpenTerminal = nil })
	none.start()
	none.menu("Open terminal UI")
	none.shows("No terminal window can be opened from here")
	failing := newDesk(t, func(options *Options) {
		options.OpenTerminal = func() error { return errors.New("no terminal window could be opened") }
	})
	failing.start()
	failing.menu("Open terminal UI")
	failing.shows("no terminal window could be opened")
	if len(failing.closed) != 0 {
		t.Fatal("the dashboard closed although no terminal opened")
	}
	failing.menu("Quit")
	if len(failing.closed) != 1 {
		t.Fatal("Quit did not close the dashboard")
	}
}

func TestRecordsWorkflow(t *testing.T) {
	s := newDesk(t, nil)
	s.start()
	ctx := context.Background()
	test.Tap(s.d.buttons[app.NavRecords])
	s.shows("No records yet. Choose Add record")
	s.tap("Add record")
	s.shows("Add record", "Name", "Required, unique")
	s.d.form.save()
	s.shows("Not saved:", "the name is empty")
	s.d.form.set("name", "Core switch")
	s.d.form.set("severity", "high")
	s.d.form.set("note", "rack 4")
	s.d.form.save()
	if s.d.form != nil {
		t.Fatal("the form stayed open after a successful save")
	}
	s.shows("Added record 1: Core switch", "1 records shown", "Record 1", "Severity: High", "rack 4", "Disable")
	s.tap("Add record")
	s.d.form.set("name", "Core switch")
	s.d.form.save()
	s.shows("already exists")
	if s.d.form.entries["name"].Text != "Core switch" {
		t.Fatal("the form lost its input after an error")
	}
	s.d.form.set("name", "Edge router")
	s.d.form.save()
	s.shows("Added record 2: Edge router", "2 records shown")
	s.tap("Edit")
	s.shows("Edit record 2")
	s.d.form.set("status", records.StatusInProgress)
	s.d.form.save()
	s.shows("Updated record 2", "Status: In progress", "Revision: 2")
	s.tap("Disable")
	s.shows("Record 2 is disabled", "State: Disabled", "Enable")
	if !s.d.recordsP.disabled.Checked {
		t.Fatal("a disabled record vanished from the list")
	}
	s.tap("Enable")
	s.shows("Record 2 is enabled")
	s.d.recordsP.search.SetText("rack")
	s.shows("1 records shown")
	if s.d.recordsP.detail.Visible() {
		t.Fatal("the detail panel shows a record that is filtered out")
	}
	s.d.recordsP.search.SetText("zzz")
	s.shows("No records match these filters")
	s.d.recordsP.search.SetText("")
	s.d.recordsP.status.SetSelected("In progress")
	s.shows("1 records shown")
	s.d.recordsP.status.SetSelected("All statuses")
	s.d.recordsP.severity.SetSelected("High")
	if len(s.d.recordsP.rows) != 1 || s.d.recordsP.rows[0].Name != "Core switch" {
		t.Fatalf("severity filter: %+v", s.d.recordsP.rows)
	}
	s.d.recordsP.severity.SetSelected("All severities")
	s.d.recordsP.sort.SetSelected("Sort by name")
	s.d.recordsP.reverse.SetChecked(false)
	if s.d.recordsP.rows[0].Name != "Core switch" || s.d.recordsP.rows[1].Name != "Edge router" {
		t.Fatalf("sorting: %+v", s.d.recordsP.rows)
	}
	s.d.recordsP.list.Select(1)
	s.capture("records")
	s.tap("Remove")
	s.shows("Remove record", "This removes record 2 (Edge router) permanently.")
	s.d.confirm.cancel()
	if _, err := s.d.app.Records.Get(ctx, 2); err != nil {
		t.Fatal("cancelling removed the record")
	}
	s.tap("Remove")
	s.d.confirm.accept()
	s.shows("Removed record 2", "1 records shown")
	if _, err := s.d.app.Records.Get(ctx, 2); !errors.Is(err, records.ErrNotFound) {
		t.Fatalf("the record still exists: %v", err)
	}
	if s.d.recordsP.detail.Visible() {
		t.Fatal("the detail panel of a removed record is still open")
	}
}

func TestConflictingEditKeepsTheForm(t *testing.T) {
	s := newDesk(t, nil)
	s.start()
	ctx := context.Background()
	created, err := s.d.app.Records.Create(ctx, records.Input{Name: "shared", Status: records.StatusOpen, Severity: theme.SeverityLow, Enabled: true})
	if err != nil {
		t.Fatal(err)
	}
	s.d.refresh()
	test.Tap(s.d.buttons[app.NavRecords])
	s.d.recordsP.list.Select(0)
	s.tap("Edit")
	s.d.form.set("name", "shared mine")
	other := s.second()
	changed := created.Input()
	changed.Note = "from the other window"
	if _, err := other.Records.Update(ctx, created.ID, created.Revision, changed); err != nil {
		t.Fatal(err)
	}
	s.d.poll()
	if s.d.form == nil || s.d.form.entries["name"].Text != "shared mine" {
		t.Fatal("an external change discarded unsaved input")
	}
	s.d.form.save()
	s.shows("Not saved:", "another window changed this record")
	stored, _ := s.d.app.Records.Get(ctx, created.ID)
	if stored.Name != "shared" || stored.Note != "from the other window" {
		t.Fatalf("the other window's change was overwritten: %+v", stored)
	}
	s.d.form.close()
	s.tap("Edit")
	if s.d.form.entries["note"].Text != "from the other window" {
		t.Fatal("the form did not load the new version")
	}
	s.d.form.set("name", "shared mine")
	s.d.form.save()
	s.shows("Updated record 1")
}

func TestExternalChangesKeepSelectionAndInput(t *testing.T) {
	s := newDesk(t, nil)
	s.start()
	ctx := context.Background()
	other := s.second()
	for _, name := range []string{"alpha", "beta", "gamma"} {
		if _, err := other.Records.Create(ctx, records.Input{Name: name, Status: records.StatusOpen, Severity: theme.SeverityCritical, Enabled: true}); err != nil {
			t.Fatal(err)
		}
	}
	test.Tap(s.d.buttons[app.NavRecords])
	s.d.poll()
	s.shows("3 records shown", "Updated by another window")
	if s.d.buttons[app.NavNotifications].Text != "Notifications (3)" {
		t.Fatalf("badge: %q", s.d.buttons[app.NavNotifications].Text)
	}
	s.d.recordsP.sort.SetSelected("Sort by name")
	s.d.recordsP.reverse.SetChecked(false)
	s.d.recordsP.list.Select(1)
	s.d.poll()
	test.Tap(s.d.buttons[app.NavSettings])
	textScale := s.d.settingP.rows[settings.KeyTextScale]
	textScale.entry.SetText("12")
	if _, err := other.Records.Create(ctx, records.Input{Name: "aaa first", Status: records.StatusOpen, Severity: theme.SeverityInfo, Enabled: true}); err != nil {
		t.Fatal(err)
	}
	if _, err := other.Settings.Set(ctx, settings.KeyTheme, "green"); err != nil {
		t.Fatal(err)
	}
	s.d.poll()
	if chosen, found := s.d.recordsP.current(); !found || chosen.Name != "beta" {
		t.Fatalf("the selection moved to %+v", chosen)
	}
	if textScale.entry.Text != "12" {
		t.Fatal("an external change discarded unsaved input in Settings")
	}
	if s.d.tokens.Name != "green" || s.d.settingP.rows[settings.KeyTheme].choice.Selected != "Green" {
		t.Fatalf("the theme chosen in another window is not in use: %s", s.d.tokens.Name)
	}
	s.d.poll()
	if s.d.notice.Text != "Updated by another window" {
		t.Fatalf("notice: %q", s.d.notice.Text)
	}
}

func TestImportAndExport(t *testing.T) {
	s := newDesk(t, nil)
	s.start()
	test.Tap(s.d.buttons[app.NavRecords])
	source := filepath.Join(s.home, "in.csv")
	if err := os.WriteFile(source, []byte("name,severity,enabled\nalpha,high,true\nbeta,nope,true\ngamma,low,false\n"), 0o600); err != nil {
		t.Fatal(err)
	}
	s.tap("Import")
	s.shows("Import records", "Nothing is written until you confirm the preview")
	s.d.form.save()
	s.shows("choose the file to import")
	s.d.form.set("path", filepath.Join(s.home, "missing.csv"))
	s.d.form.save()
	s.shows("cannot open the import file")
	s.d.form.set("path", source)
	s.d.form.save()
	s.shows("Import preview", "would have created 2, updated 0, unchanged 0, rejected 1", "Rejected row 3")
	if count, _ := s.d.app.Records.Count(context.Background()); count != 0 {
		t.Fatal("the preview wrote records")
	}
	s.d.confirm.accept()
	s.shows("Import finished: created 2", "2 records shown")
	target := filepath.Join(s.home, "out.json")
	s.tap("Export")
	s.shows("Export records", "readable only by your user account")
	s.d.form.set("path", target)
	s.d.form.save()
	s.shows("Exported 2 records to " + target)
	s.tap("Export")
	s.d.form.set("path", target)
	s.d.form.save()
	s.shows("already exists", "Overwrite it")
	s.d.form.set("overwrite", "true")
	s.d.form.set("format", records.FormatCSV)
	s.d.form.save()
	content, err := os.ReadFile(target)
	if err != nil || !strings.HasPrefix(string(content), "name,status") {
		t.Fatalf("export: %q, %v", content, err)
	}
}

func TestNotificationsAndRules(t *testing.T) {
	s := newDesk(t, nil)
	s.d.confirm.accept()
	test.Tap(s.d.buttons[app.NavNotifications])
	s.shows("2 shown, 2 unread", "Mark as read", "Mark all as read", "Open record")
	s.tap("Mark as read")
	s.shows("Select a notification first")
	s.d.inboxP.list.Select(0)
	s.capture("notifications")
	s.tap("Mark as read")
	s.shows("Marked the notification as read", "2 shown, 1 unread")
	s.d.inboxP.unreadOnly.SetChecked(true)
	s.shows("1 shown, 1 unread")
	s.tap("Mark all as read")
	s.shows("Marked 1 notifications as read", "No unread notifications")
	s.d.inboxP.unreadOnly.SetChecked(false)
	s.d.inboxP.list.Select(1)
	target := s.d.inboxP.items[1].RecordID
	s.tap("Open record")
	if chosen, found := s.d.recordsP.current(); s.d.current != app.NavRecords || !found || chosen.ID != target {
		t.Fatalf("the link opened %s with %+v", s.d.current, chosen)
	}
	test.Tap(s.d.buttons[app.NavNotifications])
	s.d.inboxP.tabs.SelectIndex(1)
	s.tap("Edit rule")
	s.shows("Select an event type first")
	s.d.inboxP.ruleList.Select(0)
	s.tap("Edit rule")
	s.shows("Rule for ", "Minimum severity", "Quiet hours start", "De-duplication")
	s.d.form.set("start", "22:00")
	s.d.form.save()
	s.shows("Not saved:", "both a start and an end")
	s.d.form.set("end", "07:00")
	s.d.form.set("dedup", "soon")
	s.d.form.save()
	s.shows("whole number of minutes")
	s.d.form.set("dedup", "15")
	s.d.form.set("severity", "critical")
	s.d.form.save()
	s.shows("Saved the rule for")
	rule := s.d.inboxP.rules[0]
	if rule.QuietStart != "22:00" || rule.DedupMinutes != 15 || rule.MinSeverity != theme.SeverityCritical || !rule.Customized {
		t.Fatalf("rule: %+v", rule)
	}
	s.tap("Reset")
	if s.d.inboxP.rules[0].Customized {
		t.Fatal("the rule was not reset")
	}
}

func TestSettings(t *testing.T) {
	s := newDesk(t, nil)
	s.start()
	ctx := context.Background()
	s.env[settings.EnvName(settings.KeyDensity)] = "compact"
	s.d.refresh()
	test.Tap(s.d.buttons[app.NavSettings])
	rows := s.d.settingP.rows
	for _, definition := range settings.Definitions() {
		if rows[definition.Key] == nil {
			t.Fatalf("setting %s has no control", definition.Key)
		}
	}
	if !strings.Contains(rows[settings.KeyDensity].source.Text, "overridden by environment variable "+settings.EnvName(settings.KeyDensity)) {
		t.Fatalf("override not shown: %q", rows[settings.KeyDensity].source.Text)
	}
	before := fynetheme.Color(fynetheme.ColorNamePrimary)
	rows[settings.KeyTheme].choice.SetSelected("Green")
	s.shows("Saved Color theme")
	if s.d.tokens.Name != "green" || fynetheme.Color(fynetheme.ColorNamePrimary) == before {
		t.Fatal("the toolkit theme did not change")
	}
	if value, _ := s.second().Settings.Get(ctx, settings.KeyTheme); value.Raw != "green" {
		t.Fatal("the theme was not saved")
	}
	if rows[settings.KeyTheme].reset.Disabled() {
		t.Fatal("a stored setting cannot be reset")
	}
	test.Tap(rows[settings.KeyTheme].reset)
	if s.d.tokens.Name != "slate" || rows[settings.KeyTheme].choice.Selected != "Neutral slate" || !rows[settings.KeyTheme].reset.Disabled() {
		t.Fatal("reset did not return to the default theme")
	}
	rows[settings.KeyMode].choice.SetSelected("Dark")
	if !s.d.tokens.Dark {
		t.Fatal("dark mode is not in use")
	}
	s.capture("settings-dark")
	rows[settings.KeyMode].choice.SetSelected("Light")
	accent := rows[settings.KeyAccent]
	accent.entry.SetText("red")
	accent.entry.OnSubmitted("red")
	if !accent.problem.Visible() || !strings.Contains(accent.problem.Text, "#RRGGBB") || accent.entry.Text != "red" {
		t.Fatalf("invalid accent: %q", accent.problem.Text)
	}
	accent.entry.SetText("#FFFF00")
	accent.entry.OnSubmitted("#FFFF00")
	s.shows("was adjusted")
	if accent.problem.Visible() {
		t.Fatal("the error stayed after a valid value")
	}
	rows[settings.KeyDensity].choice.SetSelected("Compact")
	s.shows("Saved Density, but the environment variable " + settings.EnvName(settings.KeyDensity) + " still takes precedence")
	scale := rows[settings.KeyTextScale]
	scale.entry.SetText("9000")
	scale.entry.OnSubmitted("9000")
	if !strings.Contains(scale.problem.Text, "80") || !strings.Contains(scale.problem.Text, "200") {
		t.Fatalf("text size: %q", scale.problem.Text)
	}
	scale.entry.SetText("150")
	scale.entry.OnSubmitted("150")
	if fynetheme.TextSize() <= 14 {
		t.Fatalf("text size did not grow: %v", fynetheme.TextSize())
	}
	rows[settings.KeyConfirm].check.SetChecked(false)
	if value, _ := s.d.app.Settings.Get(ctx, settings.KeyConfirm); value.Raw != "false" {
		t.Fatal("the confirmation setting was not saved")
	}
	s.d.settingP.launcher.SetSelected("Terminal UI")
	if s.d.app.FrontEnd() != launcher.ModeTUI {
		t.Fatal("the front end was not remembered")
	}
	s.d.settingP.launcher.SetSelected("Ask every time")
	if s.d.app.FrontEnd() != "" {
		t.Fatal("the front end was not forgotten")
	}
}

func TestSettingsActions(t *testing.T) {
	s := newDesk(t, nil)
	s.d.confirm.accept()
	test.Tap(s.d.buttons[app.NavSettings])
	s.shows("6 demo records", "Database location", "Start at sign-in", "Open with")
	s.tap("Remove demo data")
	s.shows("This removes 6 demo records. Your own records are not touched.")
	s.d.confirm.accept()
	s.shows("Removed 6 demo records", "0 demo records")
	s.tap("Remove demo data")
	s.shows("There is no demo data to remove")
	entry := filepath.Join(s.home, ".config", "autostart", meta.Slug+"-notifier.desktop")
	s.d.settingP.notifier.SetChecked(true)
	s.shows("This writes "+entry, "needs no administrator rights")
	s.d.confirm.cancel()
	if _, err := os.Stat(entry); !errors.Is(err, os.ErrNotExist) || s.d.settingP.notifier.Checked {
		t.Fatal("cancelling turned the notifier on")
	}
	s.d.settingP.notifier.SetChecked(true)
	s.d.confirm.accept()
	if _, err := os.Stat(entry); err != nil || !s.d.settingP.notifier.Checked {
		t.Fatalf("the entry was not written: %v", err)
	}
	s.shows("The notifier starts the next time you sign in", "On: "+entry)
	s.d.settingP.notifier.SetChecked(false)
	if _, err := os.Stat(entry); !errors.Is(err, os.ErrNotExist) {
		t.Fatal("the entry was not removed")
	}
	s.tap("Change…")
	s.shows("Database location", "New location")
	s.d.form.save()
	s.shows("Not saved:")
	target := filepath.Join(s.home, "elsewhere", "data.db")
	s.d.form.set("path", target)
	s.d.form.save()
	s.shows("Copy the current database to " + target)
	s.d.confirm.accept()
	s.shows("Now using "+target, target)
	if s.d.app.Location().Path != target {
		t.Fatalf("location: %s", s.d.app.Location().Path)
	}
}

func TestAbout(t *testing.T) {
	s := newDesk(t, nil)
	if icon := s.d.aboutP.icon; icon == nil || icon.Resource == nil || len(icon.Resource.Content()) == 0 {
		t.Fatal("About does not show the application icon")
	}
	s.start()
	test.Tap(s.d.buttons[app.NavAbout])
	s.shows(meta.AppName+" "+meta.Version, "Publisher: "+meta.Publisher, "License: "+meta.License, "Source Code", meta.RepositoryURL, "Found a bug? Please report it on GitHub.", "makes no network requests")
	s.capture("about")
	s.tap("Source Code")
	if len(s.opened) != 1 || s.opened[0] != meta.RepositoryURL {
		t.Fatalf("opened: %v", s.opened)
	}
	s.tap("Found a bug? Please report it on GitHub.")
	s.shows("Report a bug", "prefilled with exactly this text and nothing else", "Operating system:")
	if strings.Contains(s.d.confirm.statement, s.home) {
		t.Fatal("the bug report preview contains a local path")
	}
	if len(s.opened) != 1 {
		t.Fatal("the browser opened before the preview was confirmed")
	}
	s.d.confirm.cancel()
	s.tap("Found a bug? Please report it on GitHub.")
	s.d.confirm.accept()
	if len(s.opened) != 2 || !strings.HasPrefix(s.opened[1], meta.RepositoryURL+"/issues/new?") {
		t.Fatalf("opened: %v", s.opened)
	}
	s.tap("Report a vulnerability privately")
	if len(s.opened) != 3 || !strings.HasSuffix(s.opened[2], "/security/advisories/new") {
		t.Fatalf("opened: %v", s.opened)
	}
	about := s.d.aboutP
	about.tabs.SelectIndex(aboutGuide)
	if len(about.hits) != len(about.usage.Sections) || !strings.Contains(about.body.String(), meta.AppName) {
		t.Fatalf("guide: %d sections, body %q", len(about.hits), about.body.String())
	}
	about.search.SetText("quiet hours")
	if len(about.hits) == 0 || len(about.hits) == len(about.usage.Sections) {
		t.Fatalf("search hits: %d", len(about.hits))
	}
	about.sections.Select(0)
	if !strings.Contains(about.body.String(), "quiet hours") {
		t.Fatalf("the selected section is not shown: %q", about.body.String())
	}
	s.capture("guide")
	about.search.SetText("zzzzqq")
	if !about.noHits.Visible() {
		t.Fatal("no message for a search without hits")
	}
	about.tabs.SelectIndex(aboutShortcuts)
	for _, shortcut := range app.Shortcuts() {
		s.shows(shortcut.Keys, shortcut.Action)
	}
	about.tabs.SelectIndex(aboutDiagnostics)
	s.shows("Schema version", "Integrity check", "Diagnostics stay on this computer")
	s.tap("Check the database")
	s.shows("The database is healthy")
	s.tap("Copy diagnostics")
	if !strings.Contains(s.d.fyneApp.Clipboard().Content(), "Schema version") {
		t.Fatal("diagnostics were not copied")
	}
	about.tabs.SelectIndex(aboutLicenses)
	s.shows("MIT License")
	blocked := newDesk(t, func(options *Options) { options.OpenURL = nil })
	blocked.start()
	test.Tap(blocked.d.buttons[app.NavAbout])
	blocked.tap("Source Code")
	blocked.shows("Copy this link: " + meta.RepositoryURL)
}

func TestWindowSizeIsSavedOnClose(t *testing.T) {
	s := newDesk(t, nil)
	s.start()
	s.window.Resize(fyne.NewSize(900, 700))
	s.d.close("")
	s.d.close("")
	if len(s.closed) != 1 {
		t.Fatalf("closed %d times", len(s.closed))
	}
	values, _ := s.second().Settings.Effective(context.Background())
	if values[settings.KeyWindowWidth].Int() != 900 || values[settings.KeyWindowHeight].Int() != 700 {
		t.Fatalf("saved size %s x %s", values[settings.KeyWindowWidth].Raw, values[settings.KeyWindowHeight].Raw)
	}
}

func TestChooser(t *testing.T) {
	var results []launcher.PromptResult
	c := newChooser(launcher.ModeGUI, func(result launcher.PromptResult) { results = append(results, result) })
	if c.dashboard.Text != "Dashboard (graphical window)" || c.terminal.Text != "Terminal (text interface)" || c.remember.Text != "Remember my choice" || c.quit.Text != "Quit" {
		t.Fatalf("labels: %q %q %q %q", c.dashboard.Text, c.terminal.Text, c.remember.Text, c.quit.Text)
	}
	if c.dashboard.Importance != widget.HighImportance || c.terminal.Importance == widget.HighImportance {
		t.Fatal("the default choice is not marked")
	}
	test.Tap(c.terminal)
	c.remember.SetChecked(true)
	test.Tap(c.dashboard)
	test.Tap(c.quit)
	want := []launcher.PromptResult{{Mode: launcher.ModeTUI}, {Mode: launcher.ModeGUI, Remember: true}, {Quit: true}}
	if len(results) != 3 || results[0] != want[0] || results[1] != want[1] || results[2] != want[2] {
		t.Fatalf("results: %+v", results)
	}
	if other := newChooser(launcher.ModeTUI, nil); other.terminal.Importance != widget.HighImportance {
		t.Fatal("the terminal default is not marked")
	}
	fyneApp := test.NewApp()
	defer fyneApp.Quit()
	applyDefaultSkin(fyneApp)
	if _, own := fyneApp.Settings().Theme().(skin); !own {
		t.Fatal("the chooser would be drawn with the toolkit's default look")
	}
	if c.icon == nil || c.icon.Resource == nil || len(c.icon.Resource.Content()) == 0 {
		t.Fatal("the chooser does not show the application icon")
	}
	window := fyneApp.NewWindow("")
	showChooser(window, launcher.ModeGUI, func(launcher.PromptResult) {})
	if window.Title() != "How do you want to open "+meta.AppName+"?" {
		t.Fatalf("title: %q", window.Title())
	}
	if *screenshots != "" {
		file, err := os.Create(filepath.Join(*screenshots, "chooser.png"))
		if err != nil {
			t.Fatal(err)
		}
		defer file.Close()
		if err := png.Encode(file, window.Canvas().Capture()); err != nil {
			t.Fatal(err)
		}
	}
}

func TestEveryDashboardCapabilityIsReachable(t *testing.T) {
	s := newDesk(t, nil)
	s.d.confirm.accept()
	s.d.recordsP.list.Select(0)
	text := s.texts(false) + "\nFirst-start dialog\nrow detail panel\n"
	for _, item := range app.Navigation() {
		text += item.Label + "\n"
	}
	for _, capability := range app.Capabilities() {
		if capability.Dashboard == app.NotApplicable {
			continue
		}
		for _, part := range strings.Split(capability.Dashboard, ", ") {
			if !strings.Contains(text, part) {
				t.Errorf("capability %s: %q is not in the dashboard", capability.ID, part)
			}
		}
	}
}

func TestSkinUsesTheTokens(t *testing.T) {
	for _, name := range theme.Names() {
		for _, mode := range []theme.Mode{theme.ModeLight, theme.ModeDark} {
			tokens := theme.Resolve(theme.Options{Name: name, Mode: mode, TextScale: 100})
			look := skin{tokens: tokens}
			if look.Color(fynetheme.ColorNamePrimary, fynetheme.VariantLight) != rgb(tokens.Palette.Accent) || look.Color(fynetheme.ColorNameBackground, fynetheme.VariantDark) != rgb(tokens.Palette.Background) || look.Color(fynetheme.ColorNameForeground, fynetheme.VariantLight) != rgb(tokens.Palette.Text) {
				t.Fatalf("%s %s: the toolkit colors do not come from the tokens", name, mode)
			}
			if look.Size(fynetheme.SizeNameText) != tokens.Type.Body || look.Size(fynetheme.SizeNameInputRadius) != tokens.Radius {
				t.Fatalf("%s %s: the toolkit sizes do not come from the tokens", name, mode)
			}
			if look.Font(fyne.TextStyle{}) == nil || look.Icon(fynetheme.IconNameCancel) == nil {
				t.Fatal("the skin has no font or icon")
			}
		}
	}
	compact := skin{tokens: theme.Resolve(theme.Options{Name: "slate", Mode: theme.ModeLight, Density: "compact", TextScale: 100})}
	regular := skin{tokens: theme.Resolve(theme.Options{Name: "slate", Mode: theme.ModeLight, Density: "comfortable", TextScale: 100})}
	if compact.Size(fynetheme.SizeNamePadding) >= regular.Size(fynetheme.SizeNamePadding) {
		t.Fatal("compact density does not reduce padding")
	}
	for name := range iconPaths {
		if resource := icon(name, theme.Hex("#102030")); !strings.Contains(string(resource.Content()), `fill="#102030"`) {
			t.Fatalf("icon %s is not tinted", name)
		}
	}
	for _, level := range theme.Severities {
		if _, found := iconPaths[level.Icon()]; !found {
			t.Fatalf("severity %s has no icon", level)
		}
	}
	for _, item := range app.Navigation() {
		if _, found := iconPaths[item.Icon]; !found {
			t.Fatalf("category %s has no icon", item.ID)
		}
	}
}

func TestTopBarSearchChipBellAndHelp(t *testing.T) {
	s := newDesk(t, nil)
	s.start()
	if s.d.chip.Visible() || s.d.bell.Text != "" {
		t.Fatal("an empty database shows a demo chip or an unread count")
	}
	test.Tap(&s.d.help.Button)
	if s.d.current != app.NavAbout || s.d.aboutP.tabs.SelectedIndex() != aboutGuide {
		t.Fatal("Help did not open the usage guide")
	}
	s.d.show(app.NavOverview)
	s.tap("Load demo data")
	if !s.d.chip.Visible() || s.d.chip.Text != "Demo data" {
		t.Fatal("the demo chip is missing while demo data is loaded")
	}
	if s.d.bell.Text != "2 unread" || s.d.bellHint() != "Notifications: 2 unread" {
		t.Fatalf("bell %q, hint %q", s.d.bell.Text, s.d.bellHint())
	}
	test.Tap(&s.d.bell.Button)
	if s.d.current != app.NavNotifications {
		t.Fatal("the bell did not open Notifications")
	}
	test.Tap(s.d.chip)
	if s.d.current != app.NavSettings {
		t.Fatal("the demo chip did not open Settings")
	}
	s.shows("Remove demo data")
	s.d.search.SetText("   ")
	s.d.search.OnSubmitted(s.d.search.Text)
	s.shows("Type one or more words")
	s.d.search.SetText("demo")
	s.d.search.OnSubmitted(s.d.search.Text)
	if s.d.current != app.NavRecords || s.d.recordsP.search.Text != "demo" || len(s.d.recordsP.rows) != 6 {
		t.Fatalf("search opened %s with %q and %d rows", s.d.current, s.d.recordsP.search.Text, len(s.d.recordsP.rows))
	}
	s.shows("6 records and")
	s.d.search.SetText("quiet hours")
	s.d.search.OnSubmitted(s.d.search.Text)
	if s.d.current != app.NavAbout || s.d.aboutP.tabs.SelectedIndex() != aboutGuide || s.d.aboutP.search.Text != "quiet hours" {
		t.Fatal("a guide-only search did not open the usage guide")
	}
	s.d.search.SetText("zzzqqq")
	s.d.search.OnSubmitted(s.d.search.Text)
	s.shows("Nothing matches \"zzzqqq\"")
	s.capture("top-bar")
}

func TestCollapsedSidebarShowsHints(t *testing.T) {
	s := newDesk(t, nil)
	s.start()
	overview := s.d.sidebarButton(app.NavOverview)
	overview.MouseIn(nil)
	if s.d.hintBox.Visible() {
		t.Fatal("an expanded sidebar repeats its labels as hints")
	}
	test.Tap(s.d.buttons["sidebar"])
	overview.MouseIn(nil)
	if !s.d.hintBox.Visible() || s.d.hintLabel != "Overview" {
		t.Fatalf("hint %q, visible %v", s.d.hintLabel, s.d.hintBox.Visible())
	}
	if s.d.hintBox.Position().X < overview.Size().Width {
		t.Fatal("the hint covers the button it describes")
	}
	s.capture("collapsed")
	overview.MouseOut()
	if s.d.hintBox.Visible() {
		t.Fatal("the hint stayed after the pointer left")
	}
	settingsButton := s.d.sidebarButton(app.NavSettings)
	settingsButton.FocusGained()
	if s.d.hintLabel != "Settings" {
		t.Fatal("keyboard focus shows no hint")
	}
	settingsButton.FocusLost()
	s.d.sidebarButton("sidebar").MouseIn(nil)
	if s.d.hintLabel != "Expand the sidebar" {
		t.Fatalf("toggle hint %q", s.d.hintLabel)
	}
}

func TestReducedMotionSkipsTheNoticeFade(t *testing.T) {
	s := newDesk(t, nil)
	s.start()
	before := s.d.fades
	s.menu("Refresh now")
	if s.d.fades != before+1 {
		t.Fatal("a notice did not fade in although motion is allowed")
	}
	if _, err := s.d.app.Settings.Set(context.Background(), settings.KeyReducedMotion, "true"); err != nil {
		t.Fatal(err)
	}
	s.d.refresh()
	if s.d.tokens.Motion {
		t.Fatal("the reduced motion setting did not reach the tokens")
	}
	before = s.d.fades
	s.menu("Refresh now")
	s.shows("Refreshed")
	if s.d.fades != before {
		t.Fatal("a notice animated although reduced motion is on")
	}
	if _, _, _, alphaValue := s.d.noticeFill.FillColor.RGBA(); alphaValue != 0 {
		t.Fatal("the notice kept a highlight with reduced motion")
	}
}

func TestApplicationIconsComeFromOneSet(t *testing.T) {
	sources, err := filepath.Glob("*.go")
	if err != nil {
		t.Fatal(err)
	}
	for _, source := range sources {
		if strings.HasSuffix(source, "_test.go") {
			continue
		}
		content, err := os.ReadFile(source)
		if err != nil {
			t.Fatal(err)
		}
		for _, line := range strings.Split(string(content), "\n") {
			if strings.Contains(line, "fynetheme.") && strings.Contains(line, "Icon()") {
				t.Errorf("%s uses a toolkit icon instead of the application set: %s", source, strings.TrimSpace(line))
			}
		}
	}
	for name := range iconPaths {
		resource := glyph(name)
		if resource == nil || len(resource.Content()) == 0 {
			t.Fatalf("glyph %s is empty", name)
		}
	}
}

func TestRecordTableColumnsSortAndCopyAreKept(t *testing.T) {
	ctx := context.Background()
	s := newDesk(t, nil)
	s.start()
	other := s.second()
	for _, name := range []string{"Alpha", "Bravo"} {
		if _, err := other.Records.Create(ctx, records.Input{Name: name, Status: records.StatusOpen, Severity: theme.SeverityLow, Note: "rack " + name, Enabled: true}); err != nil {
			t.Fatal(err)
		}
		s.clock.Advance(time.Minute)
	}
	test.Tap(s.d.buttons[app.NavRecords])
	s.d.poll()
	page := s.d.recordsP
	s.shows("ID", "Name", "Status", "Severity", "Updated", "State", "Columns", "2 records shown")
	if page.rows[0].Name != "Bravo" || page.sort.Selected != "Sort by update" || !page.reverse.Checked {
		t.Fatalf("default order: %+v %q %v", page.rows, page.sort.Selected, page.reverse.Checked)
	}
	s.tap("Columns")
	s.shows("Columns of the record table", "Severity", "Updated")
	if _, pinned := s.d.form.selects["name"]; pinned {
		t.Fatal("the name column can be hidden")
	}
	s.d.form.set("severity", "hide")
	s.d.form.set("id", "hide")
	s.d.form.save()
	s.shows("Saved the columns of the record table")
	if value, err := other.Settings.Get(ctx, settings.KeyRecordColumns); err != nil || value.Raw != "name,status,updated,state" {
		t.Fatalf("columns were not saved: %q %v", value.Raw, err)
	}
	header := page.header.(*fyne.Container).Objects
	for index, column := range settings.RecordColumns {
		hidden := column == "id" || column == "severity"
		if header[index].Visible() == hidden {
			t.Fatalf("header column %s: visible=%v", column, header[index].Visible())
		}
	}
	if widths := page.layout.widths(1000); widths[0] != 0 || widths[3] != 0 || widths[1] <= 0 {
		t.Fatalf("hidden columns still take room: %v", widths)
	}
	s.capture("columns")
	page.sort.SetSelected("Sort by name")
	page.reverse.SetChecked(false)
	if value, err := other.Settings.Get(ctx, settings.KeyRecordSort); err != nil || value.Raw != "name:asc" {
		t.Fatalf("sort was not saved: %q %v", value.Raw, err)
	}
	if page.rows[0].Name != "Alpha" {
		t.Fatalf("rows are not sorted by name: %+v", page.rows)
	}
	if _, err := other.Settings.Set(ctx, settings.KeyRecordSort, "name:desc"); err != nil {
		t.Fatal(err)
	}
	if _, err := other.Settings.Reset(ctx, settings.KeyRecordColumns); err != nil {
		t.Fatal(err)
	}
	s.d.poll()
	s.shows("Updated by another window", "Severity")
	if page.rows[0].Name != "Bravo" || !page.reverse.Checked || page.sort.Selected != "Sort by name" {
		t.Fatalf("the order chosen in another window is not shown: %+v", page.rows)
	}
	for index := range settings.RecordColumns {
		if !header[index].Visible() {
			t.Fatalf("column %d stayed hidden after a reset", index)
		}
	}
	page.list.Select(0)
	s.tap("Copy")
	s.shows("Copied record 2 to the clipboard")
	copied := s.d.fyneApp.Clipboard().Content()
	for _, part := range []string{"Record 2: Bravo", "Status: Open", "Severity: Low", "State: Enabled", "Note: rack Bravo"} {
		if !strings.Contains(copied, part) {
			t.Fatalf("clipboard lacks %q: %q", part, copied)
		}
	}
	s.env[settings.EnvName(settings.KeyRecordSort)] = "severity:asc"
	page.reverse.SetChecked(false)
	s.shows("Sort order saved, but the environment variable " + settings.EnvName(settings.KeyRecordSort) + " still takes precedence")
	if page.sort.Selected != "Sort by severity" || page.reverse.Checked {
		t.Fatalf("an overridden sort is not shown: %q %v", page.sort.Selected, page.reverse.Checked)
	}
}

func TestColumnLayoutSkipsHiddenColumns(t *testing.T) {
	layout := &columns{weights: []float32{0, 1, 0}, fixed: []float32{50, 100, 70}}
	full := layout.widths(400)
	if full[0] != 50 || full[2] != 70 || full[1] <= 100 {
		t.Fatalf("widths: %v", full)
	}
	layout.hidden = []bool{true, false, false}
	reduced := layout.widths(400)
	if reduced[0] != 0 || reduced[2] != 70 || reduced[1] <= full[1] {
		t.Fatalf("a hidden column must give its room to the flexible one: %v then %v", full, reduced)
	}
	cells := []fyne.CanvasObject{widget.NewLabel("a"), widget.NewLabel("b"), widget.NewLabel("c")}
	layout.Layout(cells, fyne.NewSize(400, 30))
	if cells[0].Visible() || !cells[1].Visible() || cells[1].Position().X != 0 {
		t.Fatalf("layout: %v %v %v", cells[0].Visible(), cells[1].Visible(), cells[1].Position())
	}
	layout.hidden = nil
	layout.Layout(cells, fyne.NewSize(400, 30))
	if !cells[0].Visible() || cells[1].Position().X <= 50 {
		t.Fatal("a column that is shown again stays hidden")
	}
	if layout.MinSize(cells).Width < 220 {
		t.Fatalf("minimum width: %v", layout.MinSize(cells))
	}
}

func TestHostRunsTheChooserAndTheDashboardInOneWindow(t *testing.T) {
	host := NewHost(func() fyne.App { return test.NewApp() })
	type answer struct {
		result launcher.PromptResult
		err    error
	}
	answers := make(chan answer, 1)
	go func() {
		result, err := host.Chooser(launcher.ModeTUI)
		answers <- answer{result, err}
	}()
	host.Loop()
	if host.window.Title() != "How do you want to open "+meta.AppName+"?" {
		t.Fatalf("chooser title: %q", host.window.Title())
	}
	if _, own := host.fyneApp.Settings().Theme().(skin); !own {
		t.Fatal("the chooser is drawn with the toolkit's default look")
	}
	if icon := appIcon(); icon.Name() != "icon.png" || len(icon.Content()) == 0 {
		t.Fatal("the application has no icon")
	}
	var dashboardButton *widget.Button
	walk(host.window.Content(), func(object fyne.CanvasObject) {
		if button, ok := object.(*widget.Button); ok && button.Text == "Dashboard (graphical window)" {
			dashboardButton = button
		}
	})
	if dashboardButton == nil {
		t.Fatal("the chooser has no dashboard button")
	}
	test.Tap(dashboardButton)
	if got := <-answers; got.err != nil || got.result != (launcher.PromptResult{Mode: launcher.ModeGUI}) {
		t.Fatalf("chooser answer: %+v", got)
	}
	home := t.TempDir()
	application, err := app.Open(context.Background(), app.Options{
		Environment: userconfig.Environment{Getenv: func(string) string { return "" }, HomeDir: func() (string, error) { return home, nil }, GOOS: "linux"},
		Now:         testkit.NewClock().Now,
	})
	if err != nil {
		t.Fatal(err)
	}
	defer application.Close()
	ctx, cancel := context.WithCancel(context.Background())
	cancel()
	switchTo, err := host.Dashboard(ctx, Options{App: application, Getenv: func(string) string { return "" }})
	if err != nil || switchTo != "" {
		t.Fatalf("dashboard: %q %v", switchTo, err)
	}
	var shown []string
	walk(host.window.Content(), func(object fyne.CanvasObject) {
		if label, ok := object.(*widget.Label); ok && label.Visible() {
			shown = append(shown, label.Text)
		}
	})
	if !strings.Contains(strings.Join(shown, "\n"), "context canceled") {
		t.Fatalf("a failure while the window was being built is not shown: %q", shown)
	}
	if host.window.Title() != meta.AppName || host.window.MainMenu() == nil {
		t.Fatalf("the dashboard did not take over the window: %q", host.window.Title())
	}
	host.Stop()
	host.Stop()
	if _, err := host.Chooser(launcher.ModeGUI); err == nil || !strings.Contains(err.Error(), "closing") {
		t.Fatalf("a stopped host accepted work: %v", err)
	}
	if _, err := host.Dashboard(context.Background(), Options{App: application}); err == nil {
		t.Fatal("a stopped host opened a dashboard")
	}
	idle := NewHost(func() fyne.App {
		t.Fatal("a host without work created an application")
		return nil
	})
	idle.Stop()
	idle.Loop()
}
