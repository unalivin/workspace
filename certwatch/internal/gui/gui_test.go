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
	"fyne.io/fyne/v2/canvas"
	"fyne.io/fyne/v2/container"
	"fyne.io/fyne/v2/test"
	fynetheme "fyne.io/fyne/v2/theme"
	"fyne.io/fyne/v2/widget"

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
	server   *testkit.TLSFixture
	pending  []func()
}

func (s *desk) second() *app.App {
	s.t.Helper()
	options := app.Options{
		Environment: userconfig.Environment{Getenv: func(key string) string { return s.env[key] }, HomeDir: func() (string, error) { return s.home, nil }, GOOS: "linux"},
		Now:         s.clock.Now,
	}
	if s.server != nil {
		options.Roots, options.Dial = s.server.Pool(), s.server.Dial
	}
	other, err := app.Open(context.Background(), options)
	if err != nil {
		s.t.Fatal(err)
	}
	s.t.Cleanup(func() { _ = other.Close() })
	return other
}

func newDesk(t *testing.T, change func(*Options)) *desk {
	t.Helper()
	return openDesk(t, &desk{t: t, home: t.TempDir(), env: map[string]string{}, clock: testkit.NewClock()}, change)
}

func newLiveDesk(t *testing.T, name string, days int) *desk {
	t.Helper()
	s := &desk{t: t, home: t.TempDir(), env: map[string]string{}, clock: testkit.NewClock()}
	s.clock.Set(time.Now().UTC().Truncate(time.Second))
	s.server = testkit.StartTLS(t, s.clock.Now(), name, days)
	openDesk(t, s, nil)
	s.start()
	return s
}

func (s *desk) runPending() {
	s.t.Helper()
	if len(s.pending) == 0 {
		s.t.Fatal("no background work was started")
	}
	work := s.pending
	s.pending = nil
	for _, item := range work {
		item()
	}
}

func (s *desk) addEndpoint(host string) inventory.Endpoint {
	s.t.Helper()
	created, err := s.d.app.Inventory.CreateEndpoint(context.Background(), inventory.EndpointInput{Host: host, Port: 443, Enabled: true}, true)
	if err != nil {
		s.t.Fatal(err)
	}
	s.d.refresh()
	return created
}

func (s *desk) open(view string) *viewPage {
	s.t.Helper()
	test.Tap(s.d.buttons[view])
	if s.d.current != view {
		s.t.Fatalf("%s did not open", view)
	}
	return s.d.views[view]
}

func (s *desk) key(name fyne.KeyName) {
	s.window.Canvas().OnTypedKey()(&fyne.KeyEvent{Name: name})
}

func openDesk(t *testing.T, s *desk, change func(*Options)) *desk {
	t.Helper()
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
	s.d.spawn = func(work func()) { s.pending = append(s.pending, work) }
	s.d.onMain = func(work func()) { work() }
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
		case *canvas.Text:
			builder.WriteString(typed.Text + "\n")
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
		case *canvas.Text:
			builder.WriteString(typed.Text + "\n")
		case *widget.Label:
			builder.WriteString(typed.Text + "\n")
		case *widget.Button:
			builder.WriteString(typed.Text + "\n")
		case *widget.Check:
			builder.WriteString(typed.Text + "\n")
		}
	})
	if s.d.busy != "" {
		builder.WriteString(s.d.busyText.Text + "\n")
	}
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
	s.shows("Welcome to "+meta.AppName, "A new database was created at", "connects only to hosts you add and confirm that you are authorized to test", "Load demo data")
	s.capture("first-start")
	s.d.confirm.accept()
	s.shows("Tracked certificates by time left", "Endpoint checks", "One square is one tracked certificate. A longer period includes the shorter ones", "Expired", "Expiring in 7 days", "Expiring in 30 days", "Expiring in 90 days", "Chain failures", "Name mismatches", "Failed checks", "Not checked yet",
		"Added 11 demo endpoints", "They are never contacted", "Demo data is loaded", "Soonest to expire", "legacy.demo.invalid (demo)", "expired 3 days ago", "Recent alerts",
		"9 tracked certificates · 11 endpoints, 1 disabled · 9 unread alerts")
	for id, want := range map[string]string{"expired": "1", "within7": "1", "within30": "2", "within90": "5", "chain": "3", "hostname": "1", "failed": "1", "never": "0"} {
		if got := s.d.overP.tiles[id].number.Text; got != want {
			t.Fatalf("tile %s shows %s, want %s", id, got, want)
		}
	}
	if s.d.buttons[app.NavNotifications].count != "9" {
		t.Fatalf("badge: %q", s.d.buttons[app.NavNotifications].count)
	}
	s.capture("overview")
	test.Tap(s.d.overP.tiles["within30"].open)
	certificates := s.d.views[app.ViewCertificates]
	if s.d.current != app.ViewCertificates || certificates.filter.Selected != "Expiring in 30 days" || len(certificates.data.Rows) != 2 {
		t.Fatalf("the tile opened %s with %q and %d rows", s.d.current, certificates.filter.Selected, len(certificates.data.Rows))
	}
	s.shows("mail.demo.invalid (demo)")
	test.Tap(s.d.buttons[app.NavOverview])
	test.Tap(s.d.overP.tiles["chain"].open)
	endpoints := s.d.views[app.ViewEndpoints]
	if s.d.current != app.ViewEndpoints || endpoints.filter.Selected != "Chain failures" || len(endpoints.data.Rows) != 3 {
		t.Fatalf("the tile opened %s with %q and %d rows", s.d.current, endpoints.filter.Selected, len(endpoints.data.Rows))
	}
	empty := newDesk(t, nil)
	empty.start()
	empty.shows("Nothing is monitored yet", "Add a host you are authorized to test", "Load demo data", "Add endpoint")
	empty.tap("Add endpoint")
	if empty.d.current != app.ViewEndpoints || empty.d.form == nil || empty.d.form.title != "Endpoints, Add endpoint" {
		t.Fatal("Add endpoint on the empty overview did not open the form")
	}
	empty.d.form.close()
	test.Tap(empty.d.buttons[app.NavOverview])
	empty.tap("Load demo data")
	empty.shows("Added 11 demo endpoints")
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
		if s.d.current != item.ID || s.d.title.Text != item.Label || !s.d.pages[item.ID].object().Visible() || !s.d.buttons[item.ID].selected {
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
	s.menu("Endpoints")
	if s.d.current != app.ViewEndpoints {
		t.Fatal("the View menu did not open Endpoints")
	}
	if len(app.Navigation()) != 10 || len(s.d.views) != len(app.Views()) {
		t.Fatalf("%d categories and %d view pages", len(app.Navigation()), len(s.d.views))
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

func TestEndpointWorkflowWithALiveCheck(t *testing.T) {
	s := newLiveDesk(t, "app.fixture.test", 20)
	ctx := context.Background()
	page := s.open(app.ViewEndpoints)
	s.shows("No endpoints yet", "The hosts you monitor and what the last observation found.")
	s.tap("Add endpoint")
	s.shows("Endpoints, Add endpoint", "Host", "Port", "Server name", "Note", "I am authorized to test this host", "Host name or IP address")
	s.d.form.save()
	s.shows("Not saved:", "Host is missing")
	s.d.form.set("host", "app.fixture.test")
	s.d.form.save()
	s.shows("Not saved:", "confirm that you are authorized to test this host")
	if len(s.server.Dialed()) != 0 || len(s.pending) != 0 {
		t.Fatal("adding an endpoint opened a connection")
	}
	s.d.form.set("note", "edge")
	s.d.form.set("authorized", "true")
	s.d.form.save()
	if s.d.form != nil {
		t.Fatal("the form stayed open after a successful save")
	}
	s.shows("Added endpoint 1: app.fixture.test:443", "1 endpoint", "not checked yet", "Authorization confirmed", "Check now", "Edit", "Disable", "Remove", "Copy")
	if !page.detail.Visible() || page.selected != 1 {
		t.Fatal("the new endpoint is not selected")
	}
	s.tap("Add endpoint")
	s.d.form.set("host", "app.fixture.test")
	s.d.form.set("authorized", "true")
	s.d.form.save()
	s.shows("Not saved:", "already monitored")
	if s.d.form.entries["host"].Text != "app.fixture.test" {
		t.Fatal("the form lost its input after an error")
	}
	s.d.form.close()
	s.tap("Check now")
	s.shows("Endpoints, Check now", "Check 1 endpoint over the network", "Network connections this opens:", "app.fixture.test:443: name lookup for app.fixture.test, then one TCP connection with a TLS handshake")
	s.d.confirm.cancel()
	if len(s.server.Dialed()) != 0 || len(s.pending) != 0 || s.d.busy != "" {
		t.Fatal("a cancelled check opened a connection")
	}
	s.tap("Check now")
	s.d.confirm.accept()
	s.shows("Check now: waiting for the answers")
	if !s.d.busyBox.Visible() {
		t.Fatal("a running check is not shown")
	}
	s.tap("Remove")
	s.shows("Check now is still running")
	if s.d.confirm != nil || len(s.pending) != 1 {
		t.Fatal("a second action started while a check was running")
	}
	s.runPending()
	s.shows("Result", "Checked 1 endpoint: 0 failed, 0 changed certificates, 1 new alert.", "app.fixture.test:443: ok (expires ")
	if s.d.busy != "" || s.d.busyBox.Visible() {
		t.Fatal("the busy line stayed after the check")
	}
	s.d.confirm.accept()
	s.shows("20 days", "TLS 1.3", "Fixture Root", "trust store", "Findings (0)", "Chain", "Change history")
	if dialed := s.server.Dialed(); len(dialed) != 1 || dialed[0] != "app.fixture.test:443" {
		t.Fatalf("connections: %v", dialed)
	}
	if s.d.buttons[app.NavNotifications].count != "1" {
		t.Fatalf("badge: %q", s.d.buttons[app.NavNotifications].count)
	}
	s.capture("endpoints")
	s.tap("Edit")
	s.shows("Endpoints, Edit")
	if s.d.form.entries["note"].Text != "edge" || s.d.form.entries["host"].Text != "app.fixture.test" {
		t.Fatal("the edit form does not start from the stored endpoint")
	}
	s.d.form.set("note", "edge two")
	s.d.form.save()
	s.shows("Saved endpoint 1: app.fixture.test:443", "edge two")
	s.tap("Disable")
	s.shows("Endpoint app.fixture.test:443 is now disabled")
	page.filter.SetSelected("Enabled")
	s.shows("No endpoint matches", "Clear the search box and choose \"All\" to see everything.")
	if page.detail.Visible() {
		t.Fatal("the detail panel shows an endpoint that is filtered out")
	}
	page.filter.SetSelected("All")
	page.list.Select(0)
	s.tap("Enable")
	s.shows("Endpoint app.fixture.test:443 is now enabled")
	page.search.SetText("zzz")
	s.shows("No endpoint matches")
	page.search.SetText("")
	if len(page.data.Rows) != 1 {
		t.Fatalf("rows after clearing the search: %d", len(page.data.Rows))
	}
	certificates := s.open(app.ViewCertificates)
	if len(certificates.data.Rows) != 1 {
		t.Fatalf("certificates: %d", len(certificates.data.Rows))
	}
	certificates.list.Select(0)
	s.shows("app.fixture.test", "Subject alternative names (1)", "SHA-256", "Save as PEM", "Stop tracking")
	s.capture("certificates")
	timeline := s.open(app.ViewTimeline)
	if timeline.filter.Selected != "Next 90 days" || len(timeline.data.Rows) != 1 {
		t.Fatalf("timeline: %q with %d rows", timeline.filter.Selected, len(timeline.data.Rows))
	}
	position := -1
	for index, column := range timeline.data.Columns {
		if column.ID == timelineID {
			position = index
		}
	}
	if position < 0 {
		t.Fatal("the timeline has no bar column")
	}
	if fraction := barFraction(timeline.data.Rows[0].Cells[position]); fraction <= 0.1 || fraction >= 0.4 {
		t.Fatalf("a certificate with 20 of 90 days left fills %.2f of its bar", fraction)
	}
	s.shows("1 of 1 tracked certificate expires within 90 days")
	timeline.filter.SetSelected("Next 30 days")
	if fraction := barFraction(timeline.data.Rows[0].Cells[position]); fraction <= 0.5 {
		t.Fatalf("the zoom did not change the bar: %.2f", fraction)
	}
	s.capture("timeline")
	chains := s.open(app.ViewChains)
	chains.list.Select(0)
	s.shows("valid", "leaf", "root", "trust store")
	configuration := s.open(app.ViewConfiguration)
	configuration.list.Select(0)
	s.shows("TLS 1.3", "not probed")
	s.tap("Probe TLS versions")
	s.shows("Configuration, Probe TLS versions", "Network connections this opens:")
	s.d.confirm.accept()
	s.runPending()
	if s.d.confirm != nil {
		s.d.confirm.accept()
	}
	s.shows("TLS 1.2, TLS 1.3")
	s.open(app.ViewEndpoints)
	page.list.Select(0)
	s.tap("Remove")
	s.shows("Endpoints, Remove", "Remove endpoint app.fixture.test:443 and its 2 observations.")
	s.d.confirm.cancel()
	if _, err := s.d.app.Inventory.Endpoint(ctx, 1); err != nil {
		t.Fatal("cancelling removed the endpoint")
	}
	s.key(fyne.KeyDelete)
	s.shows("Endpoints, Remove")
	s.d.confirm.accept()
	s.shows("Removed endpoint app.fixture.test:443.", "No endpoints yet")
	if _, err := s.d.app.Inventory.Endpoint(ctx, 1); !errors.Is(err, inventory.ErrNotFound) {
		t.Fatalf("the endpoint still exists: %v", err)
	}
	if page.detail.Visible() {
		t.Fatal("the detail panel of a removed endpoint is still open")
	}
}

func TestNetworkChecksAskEvenWhenConfirmationsAreOff(t *testing.T) {
	s := newLiveDesk(t, "quick.fixture.test", 200)
	s.addEndpoint("quick.fixture.test")
	if _, err := s.d.app.Settings.Set(context.Background(), settings.KeyConfirm, "false"); err != nil {
		t.Fatal(err)
	}
	s.d.refresh()
	page := s.open(app.ViewEndpoints)
	s.tap("Check all")
	s.shows("Endpoints, Check all", "Network connections this opens:", "quick.fixture.test:443")
	if len(s.pending) != 0 || len(s.server.Dialed()) != 0 {
		t.Fatal("a network check started without confirmation")
	}
	s.d.confirm.cancel()
	page.list.Select(0)
	s.key(fyne.KeyEscape)
	if page.detail.Visible() || page.selected != 0 {
		t.Fatal("Escape did not close the detail panel")
	}
	s.key(fyne.KeyDelete)
	if s.d.confirm != nil {
		t.Fatal("Delete acted without a selected row")
	}
	page.list.Select(0)
	s.tap("Remove")
	if s.d.confirm != nil {
		t.Fatal("removal asked although confirmations are off")
	}
	s.shows("Removed endpoint quick.fixture.test:443.")
	s.d.housekeep(app.Housekeeping{Report: app.CheckReport{Checked: 2, Failed: 1}}, nil)
	s.shows("Automatic check: checked 2 endpoints: 1 failed")
	s.d.housekeep(app.Housekeeping{Alerts: 3}, nil)
	s.shows("3 new expiry alerts")
	s.d.housekeep(app.Housekeeping{}, errors.New("disk full"))
	s.shows("The automatic check stopped early: disk full")
}

func TestConflictingEditKeepsTheForm(t *testing.T) {
	s := newDesk(t, nil)
	s.start()
	ctx := context.Background()
	created := s.addEndpoint("shared.example.org")
	page := s.open(app.ViewEndpoints)
	page.list.Select(0)
	s.tap("Edit")
	s.d.form.set("note", "mine")
	other := s.second()
	changed := created.Input()
	changed.Note = "from the other window"
	if _, err := other.Inventory.UpdateEndpoint(ctx, created.ID, created.Revision, changed, false); err != nil {
		t.Fatal(err)
	}
	s.d.poll()
	if s.d.form == nil || s.d.form.entries["note"].Text != "mine" {
		t.Fatal("an external change discarded unsaved input")
	}
	s.d.form.save()
	s.shows("Not saved:", "another window changed shared.example.org:443 while you were editing")
	stored, _ := s.d.app.Inventory.Endpoint(ctx, created.ID)
	if stored.Note != "from the other window" {
		t.Fatalf("the other window's change was overwritten: %+v", stored)
	}
	s.d.form.close()
	s.tap("Edit")
	if s.d.form.entries["note"].Text != "from the other window" {
		t.Fatal("the form did not load the new version")
	}
	s.d.form.set("note", "from both")
	s.d.form.save()
	s.shows("Saved endpoint 1")
}

func TestExternalChangesKeepSelectionAndInput(t *testing.T) {
	s := newDesk(t, nil)
	s.start()
	ctx := context.Background()
	other := s.second()
	for _, name := range []string{"alpha.example.org", "beta.example.org", "gamma.example.org"} {
		if _, err := other.Inventory.CreateEndpoint(ctx, inventory.EndpointInput{Host: name, Port: 443, Enabled: true}, true); err != nil {
			t.Fatal(err)
		}
	}
	page := s.open(app.ViewEndpoints)
	s.d.poll()
	s.shows("3 endpoints", "Updated by another window")
	page.list.Select(1)
	chosen := page.selected
	s.d.poll()
	test.Tap(s.d.buttons[app.NavSettings])
	textScale := s.d.settingP.rows[settings.KeyTextScale]
	textScale.entry.SetText("12")
	if _, err := other.Inventory.CreateEndpoint(ctx, inventory.EndpointInput{Host: "aaa.example.org", Port: 443, Enabled: true}, true); err != nil {
		t.Fatal(err)
	}
	if _, err := other.Settings.Set(ctx, settings.KeyTheme, "green"); err != nil {
		t.Fatal(err)
	}
	s.d.poll()
	if page.selected != chosen || page.shown.Title != "beta.example.org:443" || len(page.data.Rows) != 4 {
		t.Fatalf("the selection moved to %d (%s)", page.selected, page.shown.Title)
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

func TestImportExportAndReports(t *testing.T) {
	s := newDesk(t, nil)
	s.start()
	s.clock.Set(time.Now().UTC().Truncate(time.Second))
	ctx := context.Background()
	page := s.open(app.ViewEndpoints)
	source := filepath.Join(s.home, "in.csv")
	if err := os.WriteFile(source, []byte("host,port,note\nalpha.example.org,,first\nbad host,,x\n"), 0o600); err != nil {
		t.Fatal(err)
	}
	s.tap("Import list")
	s.shows("Endpoints, Import list", "File", "I am authorized to test this host")
	s.d.form.save()
	s.shows("Not saved:", "File is missing")
	s.d.form.set("file", source)
	s.d.form.set("authorized", "true")
	s.d.form.save()
	s.shows("Endpoints, Import list", "would import 1 endpoint, 0 already monitored, 1 rejected", "alpha.example.org:443", "row 3")
	if count, _ := s.d.app.Inventory.EndpointCount(ctx); count != 0 {
		t.Fatal("the preview already imported rows")
	}
	s.d.confirm.accept()
	s.shows("Imported 1 endpoint, 0 already monitored, 1 rejected.")
	s.d.confirm.accept()
	if len(page.data.Rows) != 1 {
		t.Fatalf("rows after the import: %d", len(page.data.Rows))
	}
	target := filepath.Join(s.home, "out.json")
	s.tap("Export list")
	s.shows("Endpoints, Export list", "Overwrite an existing file")
	s.d.form.set("file", target)
	s.d.form.save()
	s.shows("Wrote 1 endpoint to " + target)
	s.tap("Export list")
	s.d.form.set("file", target)
	s.d.form.save()
	s.shows("Not saved:", "already exists")
	s.d.form.set("overwrite", "true")
	s.d.form.save()
	if content, err := os.ReadFile(target); err != nil || !strings.Contains(string(content), "alpha.example.org") {
		t.Fatalf("export: %q, %v", content, err)
	}
	certificates := s.open(app.ViewCertificates)
	s.shows("No certificates yet")
	s.tap("Import PEM file")
	s.d.form.set("file", testkit.PEMFile(t, s.clock.Now(), "file.fixture.test", 5))
	s.d.form.save()
	s.shows("Certificates, Import PEM file", "would import 1 certificate, 0 already tracked", "file.fixture.test")
	s.d.confirm.accept()
	s.shows("Imported 1 certificate, 0 already tracked.")
	s.d.confirm.accept()
	certificates.list.Select(0)
	s.shows("file.fixture.test", "5 days", "imported")
	saved := filepath.Join(s.home, "one.pem")
	s.tap("Save as PEM")
	s.d.form.set("file", saved)
	s.d.form.save()
	s.shows("Wrote the certificate file.fixture.test")
	if written, err := os.ReadFile(saved); err != nil || !strings.HasPrefix(string(written), "-----BEGIN CERTIFICATE-----") {
		t.Fatalf("saved certificate: %v", err)
	}
	reports := s.open(app.ViewReports)
	s.shows("No reports yet")
	s.tap("Generate report")
	s.shows("Reports, Generate report", "Content", "Format")
	s.d.form.set("kind", inventory.ReportExpiry)
	s.d.form.set("format", inventory.FormatCSV)
	s.d.form.save()
	s.shows("Generated report 1: Certificates expiring within 90 days, 1 row.", "file.fixture.test", "Suggested file name")
	if reports.selected != 1 || !reports.detail.Visible() {
		t.Fatal("the new report is not open")
	}
	s.capture("reports")
	file := filepath.Join(s.home, "expiry.csv")
	s.tap("Save as file")
	s.d.form.set("file", file)
	s.d.form.save()
	s.shows("Wrote the report to " + file)
	if written, err := os.ReadFile(file); err != nil || !strings.Contains(string(written), "file.fixture.test") {
		t.Fatalf("saved report: %v", err)
	}
	s.tap("Copy")
	s.shows("Copied ")
	if copied := s.d.fyneApp.Clipboard().Content(); !strings.Contains(copied, "file.fixture.test") || !strings.Contains(copied, "Suggested file name") {
		t.Fatalf("clipboard: %q", copied)
	}
	s.tap("Remove")
	s.shows("Reports, Remove", "Saved files stay.")
	s.d.confirm.accept()
	s.shows("Removed report 1.", "No reports yet")
	s.open(app.ViewCertificates)
	certificates.list.Select(0)
	s.tap("Stop tracking")
	s.shows("Stop tracking the imported certificate file.fixture.test")
	s.d.confirm.accept()
	s.shows("Stopped tracking file.fixture.test.", "No certificates yet")
}

func TestAlertsAndRules(t *testing.T) {
	s := newDesk(t, nil)
	s.d.confirm.accept()
	test.Tap(s.d.buttons[app.NavNotifications])
	s.shows("9 shown, 9 unread", "Mark as read", "Mark all as read", "Open source")
	s.tap("Mark as read")
	s.shows("Select an alert first")
	s.d.inboxP.list.Select(0)
	s.capture("alerts")
	s.tap("Mark as read")
	s.shows("Marked the alert as read", "9 shown, 8 unread")
	s.d.inboxP.unreadOnly.SetChecked(true)
	s.shows("8 shown, 8 unread")
	s.tap("Mark all as read")
	s.shows("Marked 8 alerts as read", "No unread alerts")
	s.d.inboxP.unreadOnly.SetChecked(false)
	for index, item := range s.d.inboxP.items {
		view := app.ViewEndpoints
		if item.RecordKind == inventory.KindCertificate {
			view = app.ViewCertificates
		}
		test.Tap(s.d.buttons[app.NavNotifications])
		s.d.inboxP.list.Select(index)
		s.tap("Open source")
		if page := s.d.views[view]; s.d.current != view || page.selected != item.RecordID || !page.detail.Visible() {
			t.Fatalf("alert %q opened %s with row %d", item.Title, s.d.current, s.d.views[view].selected)
		}
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
	s.shows("23 demo rows", "Database location", "Start at sign-in", "Open with")
	s.tap("Remove demo data")
	s.shows("This removes 23 demo endpoints and certificates with their alerts. Entries you added yourself stay.")
	s.d.confirm.accept()
	s.shows("Removed 23 demo endpoints and certificates", "0 demo rows")
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
	s.shows(meta.AppName+" "+meta.Version, "Publisher: "+meta.Publisher, "License: "+meta.License, "Source Code", meta.RepositoryURL, "Found a bug? Please report it on GitHub.", "connects only to the endpoints you added", "no telemetry and no update check")
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
	s.d.beginNamed(app.ViewReports, "generate")
	s.d.form.save()
	text := "\nFirst-start dialog\nrow detail panel\n"
	for _, view := range app.Views() {
		page := s.open(view.ID)
		page.list.Select(0)
		if !page.detail.Visible() {
			t.Fatalf("%s has no row detail panel", view.ID)
		}
		text += s.pageText()
	}
	text += s.texts(false)
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

func TestEveryViewActionHasAButtonAndACapability(t *testing.T) {
	s := newDesk(t, nil)
	s.d.confirm.accept()
	paths := map[string]string{}
	for _, capability := range app.Capabilities() {
		paths[capability.CLI] = capability.Dashboard
	}
	for _, view := range app.Views() {
		page := s.open(view.ID)
		if len(page.data.Rows) == 0 && view.ID != app.ViewReports {
			t.Fatalf("%s shows no demo rows", view.ID)
		}
		if len(page.data.Rows) > 0 {
			page.list.Select(0)
			if !page.detail.Visible() || page.shown.Title == "" {
				t.Fatalf("%s: selecting a row did not open the detail panel", view.ID)
			}
		}
		for _, action := range view.Actions {
			if want := view.Title + ", " + action.Label; paths[view.ID+" "+action.ID] != want {
				t.Errorf("%s %s: the capability table says %q, the button is %q", view.ID, action.ID, paths[view.ID+" "+action.ID], want)
			}
			if _, known := actionIcons[action.ID]; !known {
				t.Errorf("%s %s has no icon", view.ID, action.ID)
			}
			if action.Row && len(page.data.Rows) == 0 {
				continue
			}
			if action.Row {
				for index := 0; index < len(page.data.Rows) && !page.shown.Usable(action.ID); index++ {
					page.list.Select(index)
				}
				if !page.shown.Usable(action.ID) {
					t.Fatalf("%s %s: no demo row offers the action", view.ID, action.ID)
				}
				for other, button := range page.rowButtons {
					if button.Visible() != page.shown.Usable(other) {
						t.Errorf("%s: the button %q is shown for a row it does not apply to", view.ID, button.Text)
					}
				}
			}
			s.d.notice.SetText("")
			s.tap(action.Label)
			switch {
			case s.d.form != nil:
				if s.d.form.title != view.Title+", "+action.Label {
					t.Errorf("%s %s opened the form %q", view.ID, action.ID, s.d.form.title)
				}
				s.d.form.close()
			case s.d.confirm != nil:
				s.d.confirm.cancel()
			case len(s.pending) > 0:
				s.runPending()
				if s.d.confirm != nil {
					s.d.confirm.cancel()
				}
			case s.d.notice.Text == "":
				t.Errorf("%s: %s did nothing", view.ID, action.Label)
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
			if look.Size(fynetheme.SizeNameText) != tokens.Type.Body || look.Size(fynetheme.SizeNameInputRadius) != tokens.Radius.Control {
				t.Fatalf("%s %s: the toolkit sizes do not come from the tokens", name, mode)
			}
			if look.Font(fyne.TextStyle{}) == nil || look.Icon(fynetheme.IconNameCancel) == nil {
				t.Fatal("the skin has no font or icon")
			}
		}
	}
	compact := skin{tokens: theme.Resolve(theme.Options{Name: "slate", Mode: theme.ModeLight, Density: "compact", TextScale: 100})}
	regular := skin{tokens: theme.Resolve(theme.Options{Name: "slate", Mode: theme.ModeLight, Density: "comfortable", TextScale: 100})}
	if compact.Size(fynetheme.SizeNameInnerPadding) >= regular.Size(fynetheme.SizeNameInnerPadding) || compact.tokens.Spacing.RowHeight >= regular.tokens.Spacing.RowHeight {
		t.Fatal("compact density does not reduce padding and row height")
	}
	if regular.Font(fyne.TextStyle{Bold: true}).Name() != "NotoSans-Medium.ttf" || regular.Font(fyne.TextStyle{}).Name() == "NotoSans-Medium.ttf" {
		t.Fatal("emphasis must use the medium weight of the one text family")
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
	if s.d.bell.Text != "9 unread" || s.d.bellHint() != "Alerts: 9 unread" {
		t.Fatalf("bell %q, hint %q", s.d.bell.Text, s.d.bellHint())
	}
	test.Tap(&s.d.bell.Button)
	if s.d.current != app.NavNotifications {
		t.Fatal("the bell did not open Alerts")
	}
	test.Tap(s.d.chip)
	if s.d.current != app.NavSettings {
		t.Fatal("the demo chip did not open Settings")
	}
	s.shows("Remove demo data")
	s.d.search.SetText("   ")
	s.d.search.OnSubmitted(s.d.search.Text)
	s.shows("Type one or more words")
	s.d.search.SetText("portal")
	s.d.search.OnSubmitted(s.d.search.Text)
	endpoints := s.d.views[app.ViewEndpoints]
	if s.d.current != app.ViewEndpoints || endpoints.search.Text != "portal" || len(endpoints.data.Rows) != 1 {
		t.Fatalf("search opened %s with %q and %d rows", s.d.current, endpoints.search.Text, len(endpoints.data.Rows))
	}
	s.shows("1 endpoint, ", "certificates and")
	s.d.search.SetText("Private Lab")
	s.d.search.OnSubmitted(s.d.search.Text)
	certificates := s.d.views[app.ViewCertificates]
	if s.d.current != app.ViewCertificates || certificates.search.Text != "Private Lab" || len(certificates.data.Rows) == 0 {
		t.Fatalf("search opened %s with %q and %d rows", s.d.current, certificates.search.Text, len(certificates.data.Rows))
	}
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

func TestEndpointColumnsFollowTheSettingAndBarsAreDrawn(t *testing.T) {
	s := newDesk(t, nil)
	s.d.confirm.accept()
	ctx := context.Background()
	page := s.open(app.ViewEndpoints)
	if len(page.data.Columns) != len(settings.EndpointColumns) {
		t.Fatalf("the default table shows %d of %d columns", len(page.data.Columns), len(settings.EndpointColumns))
	}
	first := page.list
	if _, err := s.d.app.Settings.Set(ctx, settings.KeyEndpointColumns, "status,tls"); err != nil {
		t.Fatal(err)
	}
	s.d.refresh()
	if page.columns != "endpoint,status,tls" || page.list == first || len(page.shape.fixed) != 3 {
		t.Fatalf("the table was not rebuilt for the chosen columns: %s", page.columns)
	}
	s.shows("Endpoint", "Status", "TLS")
	if strings.Contains(s.pageText(), "Last check") {
		t.Fatal("a hidden column still has a header")
	}
	if _, err := s.d.app.Settings.Set(ctx, settings.KeyEndpointSort, "expires:asc"); err != nil {
		t.Fatal(err)
	}
	s.d.refresh()
	if !strings.HasPrefix(page.data.Rows[0].Cells[0], "legacy.demo.invalid:443") {
		t.Fatalf("sorted by expiry, the first row is %q", page.data.Rows[0].Cells[0])
	}
	page.list.Select(0)
	s.tap("Copy")
	s.shows("Copied legacy.demo.invalid:443 to the clipboard")
	if copied := s.d.fyneApp.Clipboard().Content(); !strings.Contains(copied, "legacy.demo.invalid:443") || !strings.Contains(copied, "Authorization confirmed: ") || !strings.Contains(copied, "Findings") {
		t.Fatalf("clipboard: %q", copied)
	}
	timeline := s.open(app.ViewTimeline)
	bar := s.d.newBar()
	bar.Resize(fyne.NewSize(200, 20))
	position := -1
	for index, column := range timeline.data.Columns {
		if column.ID == timelineID {
			position = index
		}
	}
	row := timeline.data.Rows[len(timeline.data.Rows)-1]
	s.d.fillBar(bar, timeline.data, row, row.Cells[position])
	bar.Layout.Layout(bar.Objects, bar.Size())
	if width := bar.Objects[1].Size().Width; width < 150 || bar.Objects[0].Size().Width != 200 {
		t.Fatalf("a certificate near the end of the window fills %v of 200", width)
	}
	if len(timeline.data.Marks) == 0 || !bar.Objects[2].Visible() {
		t.Fatal("the alert thresholds are not marked on the bar")
	}
	if barFraction("no bar") != 0 || barFraction("▕██░░▏") != 0.5 {
		t.Fatal("bar fractions are read wrongly")
	}
	for _, severity := range append([]theme.Severity{""}, theme.Severities...) {
		if s.d.severityIcon(severity) == nil {
			t.Fatalf("severity %q has no icon", severity)
		}
	}
}

func TestTallySquaresScaleToTheRoom(t *testing.T) {
	for _, c := range []struct{ largest, capacity, want int }{{0, 10, 1}, {10, 10, 1}, {11, 10, 2}, {480, 10, 50}, {7, 0, 10}, {250000, 10, 25000}} {
		if got := ledgerUnit(c.largest, c.capacity); got != c.want {
			t.Errorf("%d items in room for %d squares: one square is %d, want %d", c.largest, c.capacity, got, c.want)
		}
	}
}

func TestColumnLayoutSkipsHiddenColumns(t *testing.T) {
	layout := &columns{weights: []float32{0, 1, 0}, fixed: []float32{50, 100, 70}}
	full := layout.widths(400)
	if full[0] != 50 || full[2] != 70 || full[1] <= 100 {
		t.Fatalf("widths: %v", full)
	}
	if tight := layout.widths(110 + 2*fynetheme.Padding()); tight[0] != 25 || tight[1] != 50 || tight[2] != 35 {
		t.Fatalf("columns must shrink together when the row is too narrow: %v", tight)
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
	if minimum := layout.MinSize(cells).Width; minimum < 220*narrowest || minimum >= 220 {
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
