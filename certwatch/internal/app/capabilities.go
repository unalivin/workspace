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
		{"endpoint.list", "List, search and filter endpoints", "Endpoints", "Endpoints", "endpoints list"},
		{"endpoint.columns", "Choose the columns and the sort order of the endpoint table", "Settings, Tables", "Settings, Tables", "config set"},
		{"endpoint.show", "Show one endpoint with findings, chain and history", "Endpoints, row detail panel", "Endpoints, Enter", "endpoints show"},
		{"endpoint.add", "Add an endpoint after confirming authorization", "Endpoints, Add endpoint", "Endpoints, n", "endpoints add"},
		{"endpoint.edit", "Edit an endpoint", "Endpoints, Edit", "Endpoints, e", "endpoints edit"},
		{"endpoint.enable", "Enable an endpoint", "Endpoints, Enable", "Endpoints, x", "endpoints enable"},
		{"endpoint.disable", "Disable an endpoint", "Endpoints, Disable", "Endpoints, X", "endpoints disable"},
		{"endpoint.remove", "Remove an endpoint with its history", "Endpoints, Remove", "Endpoints, d", "endpoints remove"},
		{"endpoint.check", "Check one endpoint over the network after confirmation", "Endpoints, Check now", "Endpoints, c", "endpoints check"},
		{"endpoint.check-all", "Check every enabled endpoint after confirmation", "Endpoints, Check all", "Endpoints, C", "endpoints check-all"},
		{"endpoint.import", "Import endpoints from CSV or JSON", "Endpoints, Import list", "Endpoints, i", "endpoints import"},
		{"endpoint.export", "Export endpoints to CSV or JSON", "Endpoints, Export list", "Endpoints, o", "endpoints export"},
		{"check.schedule", "Check endpoints automatically at an interval", "Settings, Checks, Automatic check interval", "Settings, Automatic check interval", "watch"},
		{"certificate.list", "List, search and filter certificates", "Certificates", "Certificates", "certificates list"},
		{"certificate.show", "Show one certificate with names, chain and change history", "Certificates, row detail panel", "Certificates, Enter", "certificates show"},
		{"certificate.import", "Track the certificates of a PEM file", "Certificates, Import PEM file", "Certificates, i", "certificates import"},
		{"certificate.export", "Save a certificate as a PEM file", "Certificates, Save as PEM", "Certificates, o", "certificates export"},
		{"certificate.stop-tracking", "Stop tracking an imported certificate", "Certificates, Stop tracking", "Certificates, d", "certificates stop-tracking"},
		{"timeline.show", "See the expiry timeline and change its zoom", "Expiry Timeline, Zoom", "Expiry Timeline, f", "timeline list"},
		{"timeline.detail", "Show one certificate of the timeline", "Expiry Timeline, row detail panel", "Expiry Timeline, Enter", "timeline show"},
		{"chain.list", "See the chain validation result of every endpoint", "Chains", "Chains", "chains list"},
		{"chain.show", "See who issued each certificate of a chain", "Chains, row detail panel", "Chains, Enter", "chains show"},
		{"configuration.list", "See negotiated and accepted TLS versions", "Configuration", "Configuration", "configuration list"},
		{"configuration.show", "See the findings of one endpoint", "Configuration, row detail panel", "Configuration, Enter", "configuration show"},
		{"configuration.probe", "Probe which TLS versions an endpoint accepts", "Configuration, Probe TLS versions", "Configuration, c", "configuration probe"},
		{"report.list", "List reports", "Reports", "Reports", "reports list"},
		{"report.show", "Read a report", "Reports, row detail panel", "Reports, Enter", "reports show"},
		{"report.generate", "Generate an inventory, expiry or findings report", "Reports, Generate report", "Reports, n", "reports generate"},
		{"report.export", "Save a report as a file", "Reports, Save as file", "Reports, o", "reports export"},
		{"report.remove", "Remove a report", "Reports, Remove", "Reports, d", "reports remove"},
		{"alert.thresholds", "Set the expiry thresholds", "Settings, Alerts, Expiry thresholds", "Settings, Expiry thresholds", "config set"},
		{"demo.load", "Load labeled demo data", "Overview, Load demo data", "Overview, l", "demo load"},
		{"demo.remove", "Remove demo data", "Settings, Data, Remove demo data", "Settings, Remove demo data", "demo remove"},
		{"notification.list", "List and filter alerts", "Alerts", "Alerts", "alerts list"},
		{"notification.read", "Mark alerts as read", "Alerts, Mark as read", "Alerts, m and M", "alerts read"},
		{"notification.rule.list", "See the rule of every event type", "Alerts, Rules", "Alerts, Rules", "alerts rules list"},
		{"notification.rule.set", "Change an alert rule", "Alerts, Rules", "Alerts, Rules, e", "alerts rules set"},
		{"notification.rule.reset", "Reset an alert rule", "Alerts, Rules, Reset", "Alerts, Rules, u", "alerts rules reset"},
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
