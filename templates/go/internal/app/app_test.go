package app

import (
	"context"
	"errors"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/unalivin/templateapp/internal/meta"
	"github.com/unalivin/templateapp/internal/notify"
	"github.com/unalivin/templateapp/internal/records"
	"github.com/unalivin/templateapp/internal/settings"
	"github.com/unalivin/templateapp/internal/testkit"
	"github.com/unalivin/templateapp/internal/theme"
	"github.com/unalivin/templateapp/internal/userconfig"
)

type sandbox struct {
	home  string
	env   map[string]string
	clock *testkit.Clock
}

func newSandbox(t *testing.T) *sandbox {
	t.Helper()
	return &sandbox{home: t.TempDir(), env: map[string]string{}, clock: testkit.NewClock()}
}

func (s *sandbox) options() Options {
	return Options{
		Environment: userconfig.Environment{Getenv: func(key string) string { return s.env[key] }, HomeDir: func() (string, error) { return s.home, nil }, GOOS: "linux"},
		Now:         s.clock.Now,
	}
}

func (s *sandbox) open(t *testing.T, change func(*Options)) *App {
	t.Helper()
	options := s.options()
	if change != nil {
		change(&options)
	}
	opened, err := Open(context.Background(), options)
	if err != nil {
		t.Fatalf("open: %v", err)
	}
	t.Cleanup(func() { _ = opened.Close() })
	return opened
}

func TestLocatePrecedence(t *testing.T) {
	s := newSandbox(t)
	defaultPath := filepath.Join(s.home, ".local", "share", meta.Slug, meta.Slug+".db")
	location, err := Locate(s.options())
	if err != nil || location.Path != defaultPath || location.Source != "default" || location.Exists || location.OverriddenBy != "" {
		t.Fatalf("default: %+v, %v", location, err)
	}
	if location.ConfigDir != filepath.Join(s.home, ".config", meta.Slug) {
		t.Fatalf("config dir: %s", location.ConfigDir)
	}
	saved := filepath.Join(s.home, "saved.db")
	if err := userconfig.Save(location.ConfigDir, userconfig.Config{DatabasePath: saved}); err != nil {
		t.Fatal(err)
	}
	if location, _ = Locate(s.options()); location.Path != saved || location.Source != "saved" {
		t.Fatalf("saved: %+v", location)
	}
	s.env[meta.EnvName("db")] = filepath.Join(s.home, "env.db")
	if location, _ = Locate(s.options()); location.Source != "environment" || !strings.Contains(location.OverriddenBy, meta.EnvName("db")) {
		t.Fatalf("environment: %+v", location)
	}
	options := s.options()
	options.DatabasePath = filepath.Join(s.home, "flag.db")
	if location, _ = Locate(options); location.Source != "flag" || location.OverriddenBy != "command-line flag --db" || location.Path != options.DatabasePath {
		t.Fatalf("flag: %+v", location)
	}
	if err := os.WriteFile(filepath.Join(location.ConfigDir, "config.json"), []byte("{broken"), 0o600); err != nil {
		t.Fatal(err)
	}
	if _, err := Locate(s.options()); err == nil || !strings.Contains(err.Error(), "not valid JSON") {
		t.Fatalf("broken config: %v", err)
	}
}

func TestLocateWithoutHomeDirectoryExplainsTheFix(t *testing.T) {
	options := Options{Environment: userconfig.Environment{Getenv: func(string) string { return "" }, HomeDir: func() (string, error) { return "", errors.New("no home") }, GOOS: "linux"}}
	if _, err := Locate(options); err == nil || !strings.Contains(err.Error(), meta.EnvName("config_dir")) {
		t.Fatalf("got %v", err)
	}
}

func TestOpenCreatesTheDatabaseOnceWithPrivatePermissions(t *testing.T) {
	s := newSandbox(t)
	first := s.open(t, nil)
	if !first.CreatedDatabase() || !first.Location().Exists {
		t.Fatal("the first start must report a new database")
	}
	info, err := os.Stat(first.Location().Path)
	if err != nil {
		t.Fatal(err)
	}
	if os.PathSeparator == '/' && info.Mode().Perm() != 0o600 {
		t.Fatalf("database mode %v", info.Mode().Perm())
	}
	if err := first.Close(); err != nil {
		t.Fatal(err)
	}
	second := s.open(t, nil)
	if second.CreatedDatabase() {
		t.Fatal("the second start reported a new database")
	}
}

func TestReadOnlyOpenNeedsAnExistingDatabase(t *testing.T) {
	s := newSandbox(t)
	options := s.options()
	options.ReadOnly = true
	if _, err := Open(context.Background(), options); err == nil || !strings.Contains(err.Error(), "does not exist yet") {
		t.Fatalf("got %v", err)
	}
	writer := s.open(t, nil)
	if _, err := writer.Records.Create(context.Background(), records.Input{Name: "seen", Enabled: true}); err != nil {
		t.Fatal(err)
	}
	reader := s.open(t, func(o *Options) { o.ReadOnly = true })
	if count, err := reader.Records.Count(context.Background()); err != nil || count != 1 {
		t.Fatalf("read-only count: %d, %v", count, err)
	}
	if _, err := reader.Records.Create(context.Background(), records.Input{Name: "blocked"}); err == nil {
		t.Fatal("a read-only application wrote a record")
	}
}

func TestOpenRejectsAForeignFile(t *testing.T) {
	s := newSandbox(t)
	path := filepath.Join(s.home, "notes.txt")
	if err := os.WriteFile(path, []byte(strings.Repeat("this is not a database\n", 50)), 0o600); err != nil {
		t.Fatal(err)
	}
	options := s.options()
	options.DatabasePath = path
	if _, err := Open(context.Background(), options); err == nil || !strings.Contains(err.Error(), "choose another file") {
		t.Fatalf("got %v", err)
	}
	content, _ := os.ReadFile(path)
	if !strings.HasPrefix(string(content), "this is not a database") {
		t.Fatal("the foreign file was modified")
	}
}

func TestRememberFrontEnd(t *testing.T) {
	s := newSandbox(t)
	opened := s.open(t, nil)
	if opened.FrontEnd() != "" {
		t.Fatalf("fresh install remembers %q", opened.FrontEnd())
	}
	if err := opened.RememberFrontEnd("web"); err == nil {
		t.Fatal("an unknown front end was stored")
	}
	if err := opened.RememberFrontEnd("tui"); err != nil {
		t.Fatal(err)
	}
	if location, _ := Locate(s.options()); location.Config.FrontEnd != "tui" {
		t.Fatalf("not saved: %+v", location.Config)
	}
	if err := opened.RememberFrontEnd(""); err != nil || opened.FrontEnd() != "" {
		t.Fatalf("reset: %v", err)
	}
}

func TestMoveDatabase(t *testing.T) {
	ctx := context.Background()
	s := newSandbox(t)
	opened := s.open(t, nil)
	original := opened.Location().Path
	if _, err := opened.Records.Create(ctx, records.Input{Name: "travels", Enabled: true}); err != nil {
		t.Fatal(err)
	}
	if _, err := opened.Settings.Set(ctx, settings.KeyTheme, "green"); err != nil {
		t.Fatal(err)
	}
	for name, target := range map[string]string{"empty": "", "same": original, "directory": s.home} {
		if _, err := opened.PlanMove(target, MoveCopy); err == nil {
			t.Fatalf("%s target was accepted", name)
		}
	}
	occupied := filepath.Join(s.home, "occupied.db")
	if err := os.WriteFile(occupied, []byte("x"), 0o600); err != nil {
		t.Fatal(err)
	}
	if _, err := opened.PlanMove(occupied, MoveCopy); err == nil || !strings.Contains(err.Error(), "would be overwritten") {
		t.Fatalf("copy over an existing file: %v", err)
	}
	copied := filepath.Join(s.home, "nested dir", "copy.db")
	plan, err := opened.PlanMove(copied, MoveCopy)
	if err != nil || !strings.Contains(plan.Statement, "Copy the current database") || !strings.Contains(plan.Statement, "stays untouched") {
		t.Fatalf("plan: %+v, %v", plan, err)
	}
	if _, err := os.Stat(copied); !errors.Is(err, os.ErrNotExist) {
		t.Fatal("planning created the file")
	}
	if err := opened.Move(ctx, plan); err != nil {
		t.Fatal(err)
	}
	if opened.Location().Path != copied || opened.Location().Source != "saved" {
		t.Fatalf("location after move: %+v", opened.Location())
	}
	if count, _ := opened.Records.Count(ctx); count != 1 {
		t.Fatalf("records after copy: %d", count)
	}
	if value, _ := opened.Settings.Get(ctx, settings.KeyTheme); value.Raw != "green" {
		t.Fatalf("settings after copy: %q", value.Raw)
	}
	if _, err := os.Stat(original); err != nil {
		t.Fatalf("the original file must stay: %v", err)
	}
	if err := opened.Move(ctx, plan); err == nil {
		t.Fatal("a stale plan was applied")
	}
	fresh := filepath.Join(s.home, "fresh.db")
	plan, err = opened.PlanMove(fresh, MoveSwitch)
	if err != nil || !strings.Contains(plan.Statement, "Create a new empty database") {
		t.Fatalf("plan: %+v, %v", plan, err)
	}
	if err := opened.Move(ctx, plan); err != nil {
		t.Fatal(err)
	}
	if count, _ := opened.Records.Count(ctx); count != 0 || !opened.CreatedDatabase() {
		t.Fatalf("a new database must be empty: %d records", count)
	}
	plan, err = opened.PlanMove(original, MoveSwitch)
	if err != nil || !strings.Contains(plan.Statement, "Use the existing database") {
		t.Fatalf("plan: %+v, %v", plan, err)
	}
	if err := opened.Move(ctx, plan); err != nil {
		t.Fatal(err)
	}
	if count, _ := opened.Records.Count(ctx); count != 1 {
		t.Fatalf("switching back lost data: %d", count)
	}
	plan, _ = opened.PlanMove(occupied, MoveSwitch)
	if err := opened.Move(ctx, plan); err == nil {
		t.Fatal("a file that is not a database was accepted")
	}
	if opened.Location().Path != original {
		t.Fatal("a failed move changed the location")
	}
	if count, err := opened.Records.Count(ctx); err != nil || count != 1 {
		t.Fatalf("the application is unusable after a failed move: %d, %v", count, err)
	}
	reopened := s.open(t, nil)
	if reopened.Location().Path != original {
		t.Fatalf("the saved location is %s", reopened.Location().Path)
	}
}

func TestMoveIsRefusedWhileTheLocationIsOverridden(t *testing.T) {
	s := newSandbox(t)
	s.env[meta.EnvName("db")] = filepath.Join(s.home, "env.db")
	opened := s.open(t, nil)
	if _, err := opened.PlanMove(filepath.Join(s.home, "other.db"), MoveCopy); err == nil || !strings.Contains(err.Error(), "environment variable") {
		t.Fatalf("got %v", err)
	}
}

func TestChangesMadeInOneFrontEndReachTheOther(t *testing.T) {
	ctx := context.Background()
	s := newSandbox(t)
	dashboard := s.open(t, nil)
	terminal := s.open(t, nil)
	for _, front := range []*App{dashboard, terminal} {
		if err := front.Acknowledge(ctx); err != nil {
			t.Fatal(err)
		}
	}
	if change, err := terminal.Poll(ctx); err != nil || change.Changed {
		t.Fatalf("an idle poll reported a change: %+v, %v", change, err)
	}
	if _, err := dashboard.Settings.Set(ctx, settings.KeyTheme, "purple"); err != nil {
		t.Fatal(err)
	}
	created, err := dashboard.Records.Create(ctx, records.Input{Name: "from the dashboard", Severity: theme.SeverityCritical, Enabled: true})
	if err != nil {
		t.Fatal(err)
	}
	if change, err := terminal.Poll(ctx); err != nil || !change.Changed {
		t.Fatalf("the terminal did not notice the change: %+v, %v", change, err)
	}
	if change, _ := terminal.Poll(ctx); change.Changed {
		t.Fatal("the same change was reported twice")
	}
	tokens, err := terminal.Appearance(ctx, false)
	if err != nil || tokens.Name != "purple" {
		t.Fatalf("theme in the terminal: %s, %v", tokens.Name, err)
	}
	seen, err := terminal.Records.Get(ctx, created.ID)
	if err != nil || seen.Name != "from the dashboard" {
		t.Fatalf("record in the terminal: %+v, %v", seen, err)
	}
	if unread, _ := terminal.Notices.UnreadCount(ctx); unread != 1 {
		t.Fatalf("notification in the terminal: %d", unread)
	}
	terminalEdit := seen.Input()
	terminalEdit.Note = "edited in the terminal"
	if _, err := terminal.Records.Update(ctx, seen.ID, seen.Revision, terminalEdit); err != nil {
		t.Fatal(err)
	}
	if change, _ := dashboard.Poll(ctx); !change.Changed {
		t.Fatal("the dashboard did not notice the terminal edit")
	}
	dashboardEdit := created.Input()
	dashboardEdit.Note = "edited in the dashboard"
	_, err = dashboard.Records.Update(ctx, created.ID, created.Revision, dashboardEdit)
	var conflict *records.ConflictError
	if !errors.As(err, &conflict) || conflict.Current.Note != "edited in the terminal" {
		t.Fatalf("the conflicting edit was not detected: %v", err)
	}
	if err := terminal.Notices.SetRule(ctx, notify.Rule{EventType: notify.EventRecordUrgent, Enabled: false, MinSeverity: theme.SeverityInfo, DedupMinutes: 5}); err != nil {
		t.Fatal(err)
	}
	if rule, _ := dashboard.Notices.Rule(ctx, notify.EventRecordUrgent); rule.Enabled || rule.DedupMinutes != 5 {
		t.Fatalf("rule in the dashboard: %+v", rule)
	}
}

func TestAppearanceFollowsSettingsAndOverrides(t *testing.T) {
	ctx := context.Background()
	s := newSandbox(t)
	opened := s.open(t, nil)
	tokens, err := opened.Appearance(ctx, true)
	if err != nil || tokens.Name != "slate" || !tokens.Dark || !tokens.Motion {
		t.Fatalf("defaults: %+v, %v", tokens, err)
	}
	for key, value := range map[string]string{settings.KeyTheme: "contrast", settings.KeyMode: "light", settings.KeyDensity: "compact", settings.KeyTextScale: "150", settings.KeyReducedMotion: "true", settings.KeyAccent: "#FFFF00"} {
		if _, err := opened.Settings.Set(ctx, key, value); err != nil {
			t.Fatal(err)
		}
	}
	tokens, _ = opened.Appearance(ctx, true)
	if tokens.Name != "contrast" || tokens.Dark || tokens.Motion || tokens.Radius != 0 || tokens.Type.Body != 19.5 || tokens.Spacing.RowHeight != 36 {
		t.Fatalf("stored settings not applied: %+v", tokens)
	}
	if len(tokens.Warnings) != 1 || !strings.Contains(tokens.Warnings[0], "adjusted") {
		t.Fatalf("a low-contrast accent must be adjusted with a warning: %v", tokens.Warnings)
	}
	overridden := s.open(t, func(o *Options) { o.Overrides = map[string]string{settings.KeyTheme: "blue"} })
	if tokens, _ := overridden.Appearance(ctx, false); tokens.Name != "blue" {
		t.Fatalf("flag override ignored: %s", tokens.Name)
	}
}

func TestFormatTime(t *testing.T) {
	ctx := context.Background()
	s := newSandbox(t)
	opened := s.open(t, nil)
	moment := time.Date(2026, 3, 2, 9, 30, 0, 0, time.UTC)
	if _, err := opened.Settings.Set(ctx, settings.KeyTimeZone, "utc"); err != nil {
		t.Fatal(err)
	}
	if got := opened.FormatTime(ctx, moment); got != "2026-03-02 09:30 UTC" {
		t.Fatalf("got %q", got)
	}
	if _, err := opened.Settings.Set(ctx, settings.KeyTimeZone, "local"); err != nil {
		t.Fatal(err)
	}
	if got := opened.FormatTime(ctx, moment); got != moment.In(time.Local).Format("2006-01-02 15:04") {
		t.Fatalf("got %q", got)
	}
}

func TestDiagnosticsAndIssueReportCarryNoPrivateData(t *testing.T) {
	ctx := context.Background()
	s := newSandbox(t)
	s.env[settings.EnvName(settings.KeyDensity)] = "compact"
	s.env[settings.EnvName(settings.KeyTextScale)] = "9000"
	opened := s.open(t, nil)
	if _, err := opened.Records.Create(ctx, records.Input{Name: "private record name", Note: "private note", Enabled: true}); err != nil {
		t.Fatal(err)
	}
	if _, err := opened.Settings.Set(ctx, settings.KeyAccent, "#123456"); err != nil {
		t.Fatal(err)
	}
	report := opened.Diagnostics(ctx, false)
	if report.SchemaVersion != 1 || report.JournalMode != "wal" || !report.ForeignKeys || !report.FullTextSearch || report.Integrity != "ok" || report.Records != 1 {
		t.Fatalf("report: %+v", report)
	}
	if strings.Join(report.Changed, ",") != settings.KeyAccent || strings.Join(report.Overridden, ",") != settings.KeyDensity {
		t.Fatalf("changed %v, overridden %v", report.Changed, report.Overridden)
	}
	if len(report.Problems) != 1 || !strings.Contains(report.Problems[0], settings.KeyTextScale) {
		t.Fatalf("problems: %v", report.Problems)
	}
	lines := report.Lines()
	labels := map[string]string{}
	for _, line := range lines {
		labels[line[0]] = line[1]
	}
	if labels["Version"] != meta.Version || labels["Dashboard in this build"] != "no" || labels["Database"] != opened.Location().Path || labels["Remembered front end"] != "ask every time" {
		t.Fatalf("lines: %v", lines)
	}
	issue := report.IssueReport()
	for _, private := range []string{s.home, "private", "#123456", opened.Location().Path} {
		if strings.Contains(issue.Body, private) || strings.Contains(issue.URL, private) {
			t.Fatalf("the issue report leaks %q:\n%s", private, issue.Body)
		}
	}
	for _, expected := range []string{meta.Version, "Operating system:", "Architecture:", "Build:", settings.KeyAccent, settings.KeyDensity} {
		if !strings.Contains(issue.Body, expected) {
			t.Fatalf("the issue report lacks %q:\n%s", expected, issue.Body)
		}
	}
	if !meta.IsRepositoryURL(issue.URL) || !strings.HasPrefix(issue.URL, meta.RepositoryURL+"/issues/new?") {
		t.Fatalf("issue URL %q", issue.URL)
	}
}

func TestExternalLinksStayInsideTheRepository(t *testing.T) {
	for _, link := range []Link{SourceLink(), AdvisoryLink()} {
		if !link.Allowed || link.Reason != "" || !strings.HasPrefix(link.URL, meta.RepositoryURL) {
			t.Fatalf("link %+v", link)
		}
	}
	for _, url := range []string{"https://example.org/", "http://github.com/unalivin/workspace", meta.RepositoryURL + "/../../other", "javascript:alert(1)", "file:///etc/passwd", ""} {
		if link := ExternalLink("x", url); link.Allowed || link.Reason == "" {
			t.Fatalf("link %q was allowed", url)
		}
	}
}

func TestNavigationShortcutsAndEmbeddedDocuments(t *testing.T) {
	items := Navigation()
	ids := map[string]bool{}
	for index, item := range items {
		if item.ID == "" || item.Label == "" || item.Icon == "" || item.Hint == "" || ids[item.ID] {
			t.Fatalf("navigation item %d is incomplete or repeated: %+v", index, item)
		}
		ids[item.ID] = true
	}
	if !items[len(items)-1].Bottom || !items[len(items)-2].Bottom || items[0].Bottom {
		t.Fatal("Settings and About belong at the bottom")
	}
	if !ids[NavNotifications] || items[2].Icon != "bell" {
		t.Fatal("the Notifications category must use the bell icon")
	}
	for _, shortcut := range Shortcuts() {
		if shortcut.Keys == "" || shortcut.Terminal == "" || shortcut.Action == "" {
			t.Fatalf("incomplete shortcut %+v", shortcut)
		}
	}
	if len(UsageGuide().Sections) == 0 {
		t.Fatal("the usage guide is not embedded")
	}
	if !strings.Contains(LicenseText(), "MIT License") || NoticesText() == "" {
		t.Fatal("license texts are not embedded")
	}
}

func TestCapabilityMatrixIsComplete(t *testing.T) {
	ids := map[string]bool{}
	for _, capability := range Capabilities() {
		if capability.ID == "" || ids[capability.ID] || capability.Name == "" {
			t.Fatalf("capability %+v is incomplete or repeated", capability)
		}
		ids[capability.ID] = true
		for column, value := range map[string]string{"Dashboard": capability.Dashboard, "Terminal UI": capability.Terminal, "CLI": capability.CLI} {
			if strings.TrimSpace(value) == "" {
				t.Errorf("capability %s has no entry for %s", capability.ID, column)
			}
		}
		if capability.CLI == NotApplicable {
			t.Errorf("capability %s must be available from the CLI", capability.ID)
		}
		if (capability.Dashboard == NotApplicable) != (capability.Terminal == NotApplicable) {
			t.Errorf("capability %s applies to only one of the two front ends", capability.ID)
		}
	}
	table := CapabilityTable()
	if strings.Count(table, "\n") != len(Capabilities())+2 || !strings.HasPrefix(table, "| Capability | Dashboard | Terminal UI | CLI |") {
		t.Fatalf("table shape:\n%s", table)
	}
	documented, err := os.ReadFile(filepath.Join("..", "..", "docs", "COMMANDS.md"))
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(strings.ReplaceAll(string(documented), "\r\n", "\n"), table) {
		t.Fatalf("docs/COMMANDS.md does not contain the current capability matrix; replace its table with:\n%s", table)
	}
}

type silentDeliverer struct{ count int }

func (d *silentDeliverer) Deliver(context.Context, string, string) error {
	d.count++
	return nil
}

func TestNotifierUsesSettingsAndReadsWithoutWriting(t *testing.T) {
	ctx := context.Background()
	s := newSandbox(t)
	opened := s.open(t, nil)
	if _, err := opened.Settings.Set(ctx, settings.KeyNotifierInterval, "120"); err != nil {
		t.Fatal(err)
	}
	deliverer := &silentDeliverer{}
	runner, err := opened.Notifier(deliverer, "")
	if err != nil {
		t.Fatal(err)
	}
	if runner.Interval != 2*time.Minute || runner.ShowTitles || runner.StateDir != opened.Location().ConfigDir {
		t.Fatalf("runner: %+v", runner)
	}
	if _, err := runner.Tick(ctx); err != nil {
		t.Fatal(err)
	}
	if _, err := opened.Records.Create(ctx, records.Input{Name: "urgent", Severity: theme.SeverityCritical, Enabled: true}); err != nil {
		t.Fatal(err)
	}
	if err := opened.Acknowledge(ctx); err != nil {
		t.Fatal(err)
	}
	if shown, err := runner.Tick(ctx); err != nil || shown != 1 || deliverer.count != 1 {
		t.Fatalf("tick: %d, %v", shown, err)
	}
	if change, _ := opened.Poll(ctx); change.Changed {
		t.Fatal("the notifier wrote to the database")
	}
}

func TestTablePreferencesMatchTheRecordTable(t *testing.T) {
	if strings.Join(settings.RecordSortKeys, ",") != strings.Join(records.SortKeys, ",") {
		t.Fatalf("sortable columns differ: settings %v, records %v", settings.RecordSortKeys, records.SortKeys)
	}
	for _, column := range settings.RecordColumns {
		if label := records.ColumnLabel(column); label == column || label == "" {
			t.Fatalf("column %s has no label", column)
		}
	}
	definition, found := settings.Lookup(settings.KeyRecordColumns)
	if !found || definition.Default != strings.Join(settings.RecordColumns, ",") || len(definition.Pinned) != 1 || definition.Pinned[0] != "name" {
		t.Fatalf("column setting: %+v", definition)
	}
}
