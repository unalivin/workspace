package notify

import (
	"context"
	"errors"
	"io"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/NexzaDev/certwatch/internal/meta"
	"github.com/NexzaDev/certwatch/internal/store"
	"github.com/NexzaDev/certwatch/internal/testkit"
	"github.com/NexzaDev/certwatch/internal/theme"
)

type recordingDeliverer struct {
	messages []string
	fail     error
}

func (d *recordingDeliverer) Deliver(_ context.Context, title, body string) error {
	if d.fail != nil {
		return d.fail
	}
	d.messages = append(d.messages, title+" | "+body)
	return nil
}

type fixture struct {
	writer    *Service
	clock     *testkit.Clock
	runner    *Runner
	deliverer *recordingDeliverer
	opens     int
	path      string
}

func newFixture(t *testing.T) *fixture {
	t.Helper()
	path := testkit.DatabasePath(t)
	clock := testkit.NewClock()
	f := &fixture{writer: New(testkit.OpenAt(t, path).DB(), clock.Now), clock: clock, deliverer: &recordingDeliverer{}, path: path}
	f.runner = &Runner{
		Interval:  time.Minute,
		StateDir:  t.TempDir(),
		Deliverer: f.deliverer,
		Open: func(ctx context.Context) (Source, io.Closer, error) {
			f.opens++
			readOnly, err := store.Open(ctx, path, store.Options{ReadOnly: true})
			if err != nil {
				return nil, nil, err
			}
			return New(readOnly.DB(), clock.Now), readOnly, nil
		},
	}
	return f
}

func (f *fixture) publish(t *testing.T, severity theme.Severity, title string, record int64) int64 {
	t.Helper()
	f.clock.Advance(time.Minute)
	outcome, id, err := f.writer.Publish(context.Background(), Event{Type: EventExpiring, Severity: severity, Title: title, RecordKind: "certificate", RecordID: record})
	if err != nil || outcome != Delivered {
		t.Fatalf("publish: %v, %v", outcome, err)
	}
	return id
}

func TestNotifierSkipsHistoryThenReportsNewItemsOnce(t *testing.T) {
	ctx := context.Background()
	f := newFixture(t)
	f.publish(t, theme.SeverityHigh, "old item", 1)
	if shown, err := f.runner.Tick(ctx); err != nil || shown != 0 || len(f.deliverer.messages) != 0 {
		t.Fatalf("first tick must only record the position: %d, %v, %v", shown, err, f.deliverer.messages)
	}
	if shown, err := f.runner.Tick(ctx); err != nil || shown != 0 {
		t.Fatalf("idle tick: %d, %v", shown, err)
	}
	f.publish(t, theme.SeverityHigh, "core switch needs attention", 2)
	if shown, err := f.runner.Tick(ctx); err != nil || shown != 1 {
		t.Fatalf("tick after one new item: %d, %v", shown, err)
	}
	if got := f.deliverer.messages; len(got) != 1 || got[0] != meta.AppName+" | 1 new alert" {
		t.Fatalf("default text must be minimal: %v", got)
	}
	if shown, _ := f.runner.Tick(ctx); shown != 0 || len(f.deliverer.messages) != 1 {
		t.Fatalf("an item was reported twice: %v", f.deliverer.messages)
	}
	f.publish(t, theme.SeverityLow, "low item", 3)
	f.publish(t, theme.SeverityCritical, "critical item", 4)
	f.publish(t, theme.SeverityMedium, "medium item", 5)
	f.runner.ShowTitles = true
	if shown, err := f.runner.Tick(ctx); err != nil || shown != 3 {
		t.Fatalf("tick after three new items: %d, %v", shown, err)
	}
	if got := f.deliverer.messages[1]; got != meta.AppName+" | 3 new alerts, most severe: critical item" {
		t.Fatalf("summary: %q", got)
	}
	if f.opens != 5 {
		t.Fatalf("the database must be opened once per tick, got %d", f.opens)
	}
}

func TestNotifierSkipsItemsAlreadyRead(t *testing.T) {
	ctx := context.Background()
	f := newFixture(t)
	if _, err := f.runner.Tick(ctx); err != nil {
		t.Fatal(err)
	}
	id := f.publish(t, theme.SeverityHigh, "seen in the app", 1)
	if err := f.writer.MarkRead(ctx, id); err != nil {
		t.Fatal(err)
	}
	if shown, err := f.runner.Tick(ctx); err != nil || shown != 0 || len(f.deliverer.messages) != 0 {
		t.Fatalf("a read item was shown: %d, %v", shown, err)
	}
	f.publish(t, theme.SeverityHigh, "new", 2)
	if shown, _ := f.runner.Tick(ctx); shown != 1 {
		t.Fatalf("the cursor did not move past the read item: %d", shown)
	}
}

func TestNotifierKeepsPositionWhenDeliveryFails(t *testing.T) {
	ctx := context.Background()
	f := newFixture(t)
	if _, err := f.runner.Tick(ctx); err != nil {
		t.Fatal(err)
	}
	f.publish(t, theme.SeverityHigh, "item", 1)
	f.deliverer.fail = errors.New("no notification service")
	if _, err := f.runner.Tick(ctx); err == nil || !strings.Contains(err.Error(), "cannot show the system notification") {
		t.Fatalf("delivery failure not reported: %v", err)
	}
	f.deliverer.fail = nil
	if shown, err := f.runner.Tick(ctx); err != nil || shown != 1 {
		t.Fatalf("the item was lost after a failed delivery: %d, %v", shown, err)
	}
}

func TestNotifierNeverWritesToTheDatabase(t *testing.T) {
	ctx := context.Background()
	f := newFixture(t)
	f.publish(t, theme.SeverityHigh, "item", 1)
	observer, err := store.Open(ctx, f.path, store.Options{ReadOnly: true})
	if err != nil {
		t.Fatal(err)
	}
	defer observer.Close()
	before, err := observer.DataVersion(ctx)
	if err != nil {
		t.Fatal(err)
	}
	for i := 0; i < 3; i++ {
		if _, err := f.runner.Tick(ctx); err != nil {
			t.Fatal(err)
		}
		f.runner.ShowTitles = !f.runner.ShowTitles
	}
	after, err := observer.DataVersion(ctx)
	if err != nil || after != before {
		t.Fatalf("the notifier changed the database: data version %d -> %d, %v", before, after, err)
	}
	if count, _ := f.writer.UnreadCount(ctx); count != 1 {
		t.Fatalf("the notifier marked items as read: %d unread", count)
	}
}

func TestNotifierRecoversFromDamagedState(t *testing.T) {
	ctx := context.Background()
	f := newFixture(t)
	f.publish(t, theme.SeverityHigh, "old", 1)
	for _, content := range []string{"{not json", `{"last_id":-5,"initialized":true}`, strings.Repeat("x", 9000)} {
		if err := os.WriteFile(filepath.Join(f.runner.StateDir, stateFileName), []byte(content), 0o600); err != nil {
			t.Fatal(err)
		}
		if shown, err := f.runner.Tick(ctx); err != nil || shown != 0 {
			t.Fatalf("damaged state %q: %d, %v", content[:9], shown, err)
		}
	}
	info, err := os.Stat(filepath.Join(f.runner.StateDir, stateFileName))
	if err != nil {
		t.Fatal(err)
	}
	if os.PathSeparator == '/' && info.Mode().Perm() != 0o600 {
		t.Fatalf("state file mode is %v", info.Mode().Perm())
	}
}

func TestNotifierReportsUnavailableDatabase(t *testing.T) {
	runner := &Runner{StateDir: t.TempDir(), Deliverer: &recordingDeliverer{}, Open: func(context.Context) (Source, io.Closer, error) {
		return nil, nil, errors.New("locked")
	}}
	if _, err := runner.Tick(context.Background()); err == nil || !strings.Contains(err.Error(), "read-only") {
		t.Fatalf("got %v", err)
	}
}

func TestRunUsesBoundedIntervalAndStopsOnCancel(t *testing.T) {
	if ClampInterval(time.Second) != MinInterval || ClampInterval(24*time.Hour) != MaxInterval || ClampInterval(time.Minute) != time.Minute {
		t.Fatal("interval is not clamped")
	}
	f := newFixture(t)
	f.runner.Interval = time.Millisecond
	var waits []time.Duration
	var logged []string
	ctx, cancel := context.WithCancel(context.Background())
	f.runner.Log = func(line string) { logged = append(logged, line) }
	f.runner.Wait = func(_ context.Context, d time.Duration) error {
		waits = append(waits, d)
		if len(waits) == 3 {
			cancel()
			return ctx.Err()
		}
		return nil
	}
	if err := f.runner.Run(ctx); err != nil {
		t.Fatal(err)
	}
	if len(waits) != 3 || waits[0] != MinInterval || f.opens != 3 {
		t.Fatalf("waits %v, opens %d", waits, f.opens)
	}
	f.runner.Open = func(context.Context) (Source, io.Closer, error) { return nil, nil, errors.New("gone") }
	f.runner.MaxTicks = 2
	f.runner.Wait = func(context.Context, time.Duration) error { return nil }
	if err := f.runner.Run(context.Background()); err != nil {
		t.Fatal(err)
	}
	if len(logged) != 2 || !strings.Contains(logged[0], "gone") {
		t.Fatalf("errors must be logged and survived: %v", logged)
	}
	f.runner.Wait = func(context.Context, time.Duration) error { return errors.New("timer broke") }
	if err := f.runner.Run(context.Background()); err == nil {
		t.Fatal("a wait failure was swallowed")
	}
}

func TestRunWithRealTimerStopsOnCancel(t *testing.T) {
	f := newFixture(t)
	ctx, cancel := context.WithTimeout(context.Background(), 50*time.Millisecond)
	defer cancel()
	done := make(chan error, 1)
	go func() { done <- f.runner.Run(ctx) }()
	select {
	case err := <-done:
		if err != nil {
			t.Fatal(err)
		}
	case <-time.After(5 * time.Second):
		t.Fatal("the notifier did not stop after cancellation")
	}
}

func TestSummary(t *testing.T) {
	if _, body := Summary(nil, true); body != "" {
		t.Fatalf("empty summary has a body: %q", body)
	}
	secret := Notification{Title: "token ghp_abcdefghijklmnopqrstuvwxyz0123456789", Severity: theme.SeverityHigh}
	if _, body := Summary([]Notification{secret}, true); strings.Contains(body, "ghp_") {
		t.Fatalf("secret-like text reached the summary: %q", body)
	}
	if _, body := Summary([]Notification{secret}, false); body != "1 new alert" {
		t.Fatalf("minimal summary: %q", body)
	}
}

func TestAutostartInstallUninstallStatus(t *testing.T) {
	for _, goos := range []string{"linux", "darwin", "windows"} {
		t.Run(goos, func(t *testing.T) {
			home := t.TempDir()
			executable := filepath.Join(home, "Apps and Tools", meta.Slug)
			entry := Autostart{GOOS: goos, HomeDir: home, Executable: executable}
			installed, path, err := entry.Status()
			if err != nil || installed {
				t.Fatalf("status before install: %v, %v", installed, err)
			}
			if rel, err := filepath.Rel(home, path); err != nil || strings.HasPrefix(rel, "..") {
				t.Fatalf("the entry %q is outside the user's home", path)
			}
			if dryPath, err := entry.Install(true); err != nil || dryPath != path {
				t.Fatalf("dry run: %q, %v", dryPath, err)
			}
			if _, err := os.Stat(path); !errors.Is(err, os.ErrNotExist) {
				t.Fatal("a dry run wrote the entry")
			}
			if removed, _, err := entry.Uninstall(false); err != nil || removed {
				t.Fatalf("uninstall without an entry: %v, %v", removed, err)
			}
			if _, err := entry.Install(false); err != nil {
				t.Fatal(err)
			}
			if _, err := entry.Install(false); err != nil {
				t.Fatalf("second install must succeed: %v", err)
			}
			content, err := os.ReadFile(path)
			if err != nil {
				t.Fatal(err)
			}
			written := executable
			if goos == "linux" {
				written = strings.ReplaceAll(executable, `\`, `\\`)
			}
			if !strings.Contains(string(content), written) || !strings.Contains(string(content), "notifier") || !strings.Contains(string(content), "run") {
				t.Fatalf("entry does not start the notifier:\n%s", content)
			}
			if installed, _, err := entry.Status(); err != nil || !installed {
				t.Fatalf("status after install: %v, %v", installed, err)
			}
			if removed, _, err := entry.Uninstall(true); err != nil || !removed {
				t.Fatalf("dry-run uninstall: %v, %v", removed, err)
			}
			if installed, _, _ := entry.Status(); !installed {
				t.Fatal("a dry-run uninstall removed the entry")
			}
			if removed, _, err := entry.Uninstall(false); err != nil || !removed {
				t.Fatalf("uninstall: %v, %v", removed, err)
			}
			if installed, _, _ := entry.Status(); installed {
				t.Fatal("the entry is still present")
			}
		})
	}
}

func rooted(parts ...string) string {
	directory, err := os.Getwd()
	if err != nil {
		panic(err)
	}
	return filepath.Join(append([]string{filepath.VolumeName(directory) + string(filepath.Separator)}, parts...)...)
}

func TestAutostartPathsFollowPlatformConventions(t *testing.T) {
	home := rooted("home", "u")
	linux, _ := Autostart{GOOS: "linux", HomeDir: home, Executable: rooted("opt", "app")}.Path()
	if linux != filepath.Join(home, ".config", "autostart", meta.Slug+"-notifier.desktop") {
		t.Fatalf("linux path %q", linux)
	}
	custom, _ := Autostart{GOOS: "linux", HomeDir: home, ConfigHome: rooted("cfg")}.Path()
	if custom != filepath.Join(rooted("cfg"), "autostart", meta.Slug+"-notifier.desktop") {
		t.Fatalf("XDG path %q", custom)
	}
	relative, _ := Autostart{GOOS: "linux", HomeDir: home, ConfigHome: "relative"}.Path()
	if relative != linux {
		t.Fatalf("a relative XDG_CONFIG_HOME must be ignored: %q", relative)
	}
	mac, _ := Autostart{GOOS: "darwin", HomeDir: home}.Path()
	if !strings.HasPrefix(mac, filepath.Join(home, "Library", "LaunchAgents")) || !strings.HasSuffix(mac, ".plist") {
		t.Fatalf("macOS path %q", mac)
	}
	windows, _ := Autostart{GOOS: "windows", AppData: "appdata"}.Path()
	if !strings.Contains(windows, "Startup") || !strings.HasPrefix(windows, "appdata") {
		t.Fatalf("windows path %q", windows)
	}
	fallback, _ := Autostart{GOOS: "windows", HomeDir: home}.Path()
	if !strings.HasPrefix(fallback, filepath.Join(home, "AppData", "Roaming")) {
		t.Fatalf("windows path without AppData %q", fallback)
	}
	if _, err := (Autostart{GOOS: "linux"}).Path(); err == nil {
		t.Fatal("a missing home directory was accepted")
	}
	if _, err := (Autostart{GOOS: "windows"}).Path(); err == nil {
		t.Fatal("a missing AppData and home directory was accepted")
	}
}

func TestAutostartRejectsUnsafeExecutablePaths(t *testing.T) {
	separator := string(filepath.Separator)
	unsafe := []string{"", filepath.Join("relative", "app")}
	for _, part := range []string{"app\nExec=evil", "\"quoted\"", "$(touch x)", "`id`", "%TEMP%"} {
		unsafe = append(unsafe, rooted("opt")+separator+part+separator+"app")
	}
	for _, executable := range unsafe {
		for _, goos := range []string{"linux", "darwin", "windows"} {
			entry := Autostart{GOOS: goos, HomeDir: t.TempDir(), Executable: executable}
			if _, err := entry.Content(); err == nil {
				t.Fatalf("%s accepted the executable path %q", goos, executable)
			}
			if _, err := entry.Install(false); err == nil {
				t.Fatalf("%s installed the executable path %q", goos, executable)
			}
		}
	}
	for _, goos := range []string{"linux", "darwin", "windows"} {
		if _, err := (Autostart{GOOS: goos, HomeDir: t.TempDir(), Executable: rooted("opt", "Apps and Tools", "app")}).Content(); err != nil {
			t.Fatalf("%s rejected a plain absolute path: %v", goos, err)
		}
	}
	plist, err := Autostart{GOOS: "darwin", HomeDir: rooted("Users", "u"), Executable: rooted("Applications", "A&B <x>", "app")}.Content()
	if err != nil || !strings.Contains(plist, "A&amp;B &lt;x&gt;") {
		t.Fatalf("plist is not escaped: %v\n%s", err, plist)
	}
	if content, err := (Autostart{GOOS: "windows", AppData: "x", Executable: `C:\Program Files\App\app.exe`}).Content(); err != nil || !strings.Contains(content, `"C:\Program Files\App\app.exe" notifier run`) {
		t.Fatalf("windows entry: %v\n%s", err, content)
	}
}

func TestCommandDelivererPassesTextAsDataNotCode(t *testing.T) {
	hostile := "\"; rm -rf ~; echo $(id) `id` 'x"
	for _, goos := range []string{"linux", "darwin", "windows"} {
		t.Run(goos, func(t *testing.T) {
			var ranName string
			var ranArgs, ranEnv []string
			deliverer := CommandDeliverer{GOOS: goos, LookPath: func(name string) (string, error) { return "/bin/" + name, nil }, Run: func(_ context.Context, name string, args, env []string) error {
				ranName, ranArgs, ranEnv = name, args, env
				return nil
			}}
			if err := deliverer.Deliver(context.Background(), meta.AppName, hostile); err != nil {
				t.Fatal(err)
			}
			if !strings.HasPrefix(ranName, "/bin/") {
				t.Fatalf("ran %q", ranName)
			}
			carriers := 0
			for _, value := range append(append([]string{}, ranArgs...), ranEnv...) {
				if strings.Contains(value, "rm -rf") {
					carriers++
					if value != hostile && value != "APP_NOTIFY_BODY="+hostile {
						t.Fatalf("the text is embedded in a larger argument: %q", value)
					}
				}
			}
			if carriers != 1 {
				t.Fatalf("the text must appear exactly once as its own argument, got %d in %q %q", carriers, ranArgs, ranEnv)
			}
		})
	}
	missing := CommandDeliverer{GOOS: "linux", LookPath: func(string) (string, error) { return "", errors.New("not found") }}
	if err := missing.Deliver(context.Background(), "a", "b"); err == nil || !strings.Contains(err.Error(), "notify-send was not found") {
		t.Fatalf("got %v", err)
	}
	if real := SystemDeliverer("linux"); real.LookPath == nil || real.Run == nil {
		t.Fatal("the system deliverer is incomplete")
	}
}
