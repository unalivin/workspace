package app

import (
	"context"
	"fmt"
	"sort"
	"strings"

	bundle "github.com/NexzaDev/certwatch"
	"github.com/NexzaDev/certwatch/internal/guide"
	"github.com/NexzaDev/certwatch/internal/meta"
	"github.com/NexzaDev/certwatch/internal/settings"
)

type NavItem struct {
	ID     string
	Label  string
	Icon   string
	Hint   string
	Bottom bool
}

const (
	NavOverview      = "overview"
	NavNotifications = "notifications"
	NavSettings      = "settings"
	NavAbout         = "about"
)

func Navigation() []NavItem {
	return []NavItem{
		{ID: NavOverview, Label: "Overview", Icon: "layout-dashboard", Hint: "What expires soon and what failed validation"},
		{ID: ViewCertificates, Label: "Certificates", Icon: "badge-check", Hint: "Inventory with issuer, names, validity and history"},
		{ID: ViewEndpoints, Label: "Endpoints", Icon: "globe", Hint: "Monitored hosts and their last observation"},
		{ID: ViewTimeline, Label: "Expiry Timeline", Icon: "calendar-clock", Hint: "Certificates in the order in which they expire"},
		{ID: ViewChains, Label: "Chains", Icon: "link-2", Hint: "Chain validation and issuer relationships"},
		{ID: ViewConfiguration, Label: "Configuration", Icon: "sliders-horizontal", Hint: "Observed TLS versions and findings"},
		{ID: NavNotifications, Label: "Alerts", Icon: "bell", Hint: "Alert history, rules and thresholds"},
		{ID: ViewReports, Label: "Reports", Icon: "file-text", Hint: "Generated reports"},
		{ID: NavAbout, Label: "About", Icon: "info", Hint: "Guide, shortcuts, diagnostics and licenses", Bottom: true},
		{ID: NavSettings, Label: "Settings", Icon: "settings-2", Hint: "Appearance, checks, data and notifier", Bottom: true},
	}
}

type Shortcut struct {
	Keys     string
	Terminal string
	Action   string
}

func Shortcuts() []Shortcut {
	return []Shortcut{
		{Keys: "Ctrl/Cmd+1 to 9 and 0", Terminal: "1 to 9 and 0", Action: "Open a category"},
		{Keys: "Ctrl/Cmd+F", Terminal: "/", Action: "Search"},
		{Keys: "Ctrl/Cmd+,", Terminal: ",", Action: "Open Settings"},
		{Keys: "F1", Terminal: "F1 or ?", Action: "Open the usage guide"},
		{Keys: "Ctrl/Cmd+N", Terminal: "n", Action: "Add an endpoint or generate a report"},
		{Keys: "Enter", Terminal: "Enter", Action: "Open the selected row"},
		{Keys: "Up and Down", Terminal: "Up, Down, j, k", Action: "Move between rows"},
		{Keys: "Ctrl/Cmd+R", Terminal: "r", Action: "Refresh now"},
		{Keys: "Ctrl/Cmd+B", Terminal: "b", Action: "Collapse or expand the sidebar"},
		{Keys: "Delete", Terminal: "d", Action: "Remove the selected entry after confirmation"},
		{Keys: "Escape", Terminal: "Esc", Action: "Close a panel or cancel"},
		{Keys: "Ctrl/Cmd+Q", Terminal: "q or Ctrl+C", Action: "Quit"},
	}
}

func UsageGuide() guide.Guide {
	return guide.Parse(bundle.UsageGuide)
}

func LicenseText() string {
	return strings.TrimSpace(bundle.License)
}

func NoticesText() string {
	return strings.TrimSpace(bundle.Notices)
}

type Diagnostics struct {
	Build          meta.BuildInfo
	DatabasePath   string
	DatabaseSource string
	ConfigDir      string
	SchemaVersion  int
	JournalMode    string
	ForeignKeys    bool
	FullTextSearch bool
	Integrity      string
	Endpoints      int
	Certificates   int
	FrontEnd       string
	Changed        []string
	Overridden     []string
	Problems       []string
}

func (a *App) Diagnostics(ctx context.Context, gui bool) Diagnostics {
	report := Diagnostics{Build: meta.Build(gui), DatabasePath: a.location.Path, DatabaseSource: a.location.Source, ConfigDir: a.location.ConfigDir, FrontEnd: a.location.Config.FrontEnd, Integrity: "ok"}
	if report.FrontEnd == "" {
		report.FrontEnd = "ask every time"
	}
	note := func(what string, err error) {
		if err != nil {
			report.Problems = append(report.Problems, what+": "+err.Error())
		}
	}
	var err error
	report.SchemaVersion, err = a.Store.SchemaVersion(ctx)
	note("schema version", err)
	report.JournalMode, err = a.Store.JournalMode(ctx)
	note("journal mode", err)
	report.ForeignKeys, err = a.Store.ForeignKeysEnabled(ctx)
	note("foreign keys", err)
	report.FullTextSearch, err = a.Store.HasFullTextSearch(ctx)
	note("full-text search", err)
	if err := a.Store.IntegrityCheck(ctx); err != nil {
		report.Integrity = "failed"
		note("integrity check", err)
	}
	note("foreign key check", a.Store.ForeignKeyCheck(ctx))
	report.Endpoints, err = a.Inventory.EndpointCount(ctx)
	if overview, overviewErr := a.Inventory.Overview(ctx); overviewErr == nil {
		report.Certificates = overview.Certificates
	}
	note("record count", err)
	values, err := a.Settings.List(ctx)
	note("settings", err)
	for _, value := range values {
		if value.Unknown {
			continue
		}
		if value.OverriddenBy != "" {
			report.Overridden = append(report.Overridden, value.Key)
		} else if value.HasStored && value.Raw != value.Default {
			report.Changed = append(report.Changed, value.Key)
		}
		if value.OverrideError != "" {
			report.Problems = append(report.Problems, value.Key+": "+value.OverrideError)
		}
	}
	sort.Strings(report.Changed)
	sort.Strings(report.Overridden)
	return report
}

func yesNo(value bool) string {
	if value {
		return "yes"
	}
	return "no"
}

func (d Diagnostics) Lines() [][2]string {
	revision := d.Build.Revision
	if revision == "" {
		revision = "unknown"
	}
	if d.Build.Modified {
		revision += " (modified)"
	}
	return [][2]string{
		{"Version", d.Build.Version},
		{"Revision", revision},
		{"Go", d.Build.GoVersion},
		{"Platform", d.Build.OS + "/" + d.Build.Arch},
		{"Dashboard in this build", yesNo(d.Build.GUI)},
		{"Database", d.DatabasePath},
		{"Database location set by", d.DatabaseSource},
		{"Configuration directory", d.ConfigDir},
		{"Schema version", fmt.Sprint(d.SchemaVersion)},
		{"Journal mode", d.JournalMode},
		{"Foreign keys", yesNo(d.ForeignKeys)},
		{"Full-text search", yesNo(d.FullTextSearch)},
		{"Integrity check", d.Integrity},
		{"Endpoints", fmt.Sprint(d.Endpoints)},
		{"Tracked certificates", fmt.Sprint(d.Certificates)},
		{"Remembered front end", d.FrontEnd},
		{"Settings changed from default", listOrNone(d.Changed)},
		{"Settings overridden", listOrNone(d.Overridden)},
		{"Problems", listOrNone(d.Problems)},
	}
}

func listOrNone(items []string) string {
	if len(items) == 0 {
		return "none"
	}
	return strings.Join(items, ", ")
}

type IssueReport struct {
	Title string
	Body  string
	URL   string
}

func (d Diagnostics) IssueReport() IssueReport {
	revision := d.Build.Revision
	if revision == "" {
		revision = "unknown"
	}
	var body strings.Builder
	body.WriteString("### What happened\n\n\n### What you expected\n\n\n### Steps to reproduce\n\n\n### Environment\n\n")
	fmt.Fprintf(&body, "- %s %s\n", meta.AppName, d.Build.Version)
	fmt.Fprintf(&body, "- Operating system: %s\n", d.Build.OS)
	fmt.Fprintf(&body, "- Architecture: %s\n", d.Build.Arch)
	fmt.Fprintf(&body, "- Build: %s, revision %s, dashboard %s\n", d.Build.GoVersion, revision, yesNo(d.Build.GUI))
	fmt.Fprintf(&body, "- Schema version: %d, integrity check: %s\n", d.SchemaVersion, d.Integrity)
	fmt.Fprintf(&body, "- Settings changed from default: %s\n", listOrNone(d.Changed))
	fmt.Fprintf(&body, "- Settings overridden: %s\n", listOrNone(d.Overridden))
	report := IssueReport{Title: "Bug: ", Body: body.String()}
	report.URL = meta.NewIssueURL(report.Title, report.Body)
	return report
}

type Link struct {
	Label   string
	URL     string
	Allowed bool
	Reason  string
}

func ExternalLink(label, url string) Link {
	link := Link{Label: label, URL: url, Allowed: meta.IsRepositoryURL(url)}
	if !link.Allowed {
		link.Reason = "Only links under " + meta.RepositoryURL + " can be opened from the application."
	}
	return link
}

const PrivacyNote = "The application connects only to the endpoints you added, after listing each connection. It has no telemetry and no update check."

func SourceLink() Link {
	return ExternalLink("Source Code", meta.SourceURL())
}

func AdvisoryLink() Link {
	return ExternalLink("Report a vulnerability privately", meta.AdvisoryURL())
}

func SettingGroups(values []settings.Value) []string {
	var groups []string
	seen := map[string]bool{}
	for _, value := range values {
		if value.Unknown || seen[value.Group] {
			continue
		}
		seen[value.Group] = true
		groups = append(groups, value.Group)
	}
	return groups
}
