package app

import "strings"

type Capability struct {
	ID        string
	Name      string
	Dashboard string
	Terminal  string
	CLI       string
}

const NotApplicable = "n/a"

func Capabilities() []Capability {
	return []Capability{
		{"database.create", "Create the database on first start", "First-start dialog", "First-start screen", "db init"},
		{"database.location", "Choose the database location", "Settings, Data, Database location", "Settings, Database location", "db move"},
		{"database.check", "Check database health", "About, Diagnostics", "About, Diagnostics", "db check"},
		{"overview.show", "See what needs attention", "Overview", "Overview", "overview"},
		{"record.list", "List, search, filter and sort records", "Records", "Records", "records list"},
		{"record.columns", "Choose the columns of the record table", "Records, Columns", "Records, c", "config set"},
		{"record.show", "Show one record", "Records, row detail panel", "Records, Enter", "records show"},
		{"record.add", "Add a record", "Records, Add record", "Records, n", "records add"},
		{"record.edit", "Edit a record", "Records, row detail panel, Edit", "Records, e", "records edit"},
		{"record.enable", "Enable a record", "Records, row detail panel, Enable", "Records, x", "records enable"},
		{"record.disable", "Disable a record", "Records, row detail panel, Disable", "Records, x", "records disable"},
		{"record.remove", "Remove a record", "Records, row detail panel, Remove", "Records, d", "records remove"},
		{"record.import", "Import records from CSV or JSON", "Records, Import", "Records, i", "records import"},
		{"record.export", "Export records to CSV or JSON", "Records, Export", "Records, o", "records export"},
		{"demo.load", "Load labeled demo data", "Overview, Load demo data", "Overview, l", "demo load"},
		{"demo.remove", "Remove demo data", "Settings, Data, Remove demo data", "Settings, Remove demo data", "demo remove"},
		{"notification.list", "List and filter notifications", "Notifications", "Notifications", "notifications list"},
		{"notification.read", "Mark notifications as read", "Notifications, Mark as read", "Notifications, m and M", "notifications read"},
		{"notification.rule.list", "See the rule of every event type", "Notifications, Rules", "Notifications, Rules", "notifications rules list"},
		{"notification.rule.set", "Change a notification rule", "Notifications, Rules", "Notifications, Rules, e", "notifications rules set"},
		{"notification.rule.reset", "Reset a notification rule", "Notifications, Rules, Reset", "Notifications, Rules, u", "notifications rules reset"},
		{"notifier.install", "Start the notifier at sign-in", "Settings, Notifications, Start at sign-in", "Settings, Notifier at sign-in", "notifier install"},
		{"notifier.uninstall", "Stop starting the notifier at sign-in", "Settings, Notifications, Start at sign-in", "Settings, Notifier at sign-in", "notifier uninstall"},
		{"notifier.status", "See whether the notifier starts at sign-in", "Settings, Notifications", "Settings, Notifier at sign-in", "notifier status"},
		{"notifier.run", "Run the background notifier", NotApplicable, NotApplicable, "notifier run"},
		{"setting.list", "See every setting and what overrides it", "Settings", "Settings", "config list"},
		{"setting.set", "Change a setting", "Settings", "Settings, Enter", "config set"},
		{"setting.reset", "Reset one setting", "Settings, Reset", "Settings, u", "config reset"},
		{"theme.list", "List color themes", "Settings, Appearance, Color theme", "Settings, Color theme", "config theme list"},
		{"theme.set", "Choose a color theme", "Settings, Appearance, Color theme", "Settings, Color theme", "config theme set"},
		{"theme.show", "See the theme in use", "Settings, Appearance", "Settings, Color theme", "config theme show"},
		{"launcher.show", "See the remembered front end", "Settings, Launcher", "Settings, Open with", "config launcher show"},
		{"launcher.set", "Choose the front end to open by default", "Settings, Launcher", "Settings, Open with", "config launcher set"},
		{"launcher.reset", "Ask again which front end to open", "Settings, Launcher", "Settings, Open with", "config launcher reset"},
		{"launcher.switch", "Reopen in the other front end", "Menu, Open terminal UI", "Menu, Open dashboard", "--ui"},
		{"guide.read", "Read and search the usage guide offline", "About, Usage guide", "About, Usage guide", "guide"},
		{"about.diagnostics", "Show diagnostics", "About, Diagnostics", "About, Diagnostics", "diagnostics"},
		{"about.licenses", "Show licenses and notices", "About, Licenses and notices", "About, Licenses", "licenses"},
		{"about.links", "Show the source and bug report links", "About, Overview", "About, Overview", "about"},
	}
}

func CapabilityTable() string {
	var builder strings.Builder
	builder.WriteString("| Capability | Dashboard | Terminal UI | CLI |\n|---|---|---|---|\n")
	for _, capability := range Capabilities() {
		cli := capability.CLI
		if cli != NotApplicable {
			cli = "`" + cli + "`"
		}
		builder.WriteString("| " + capability.Name + " | " + capability.Dashboard + " | " + capability.Terminal + " | " + cli + " |\n")
	}
	return builder.String()
}
