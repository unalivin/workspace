//go:build !nogui

package gui

// Screen identifiers keep navigation and accessibility labels stable across releases.
const (
	InventoryScreen = "Inventory"
	JobsScreen      = "Jobs"
	RunsScreen      = "Runs"
	ApprovalsScreen = "Approvals"
	BackupsScreen   = "Backups"
	SchedulesScreen = "Schedules"
	SettingsScreen  = "Settings"
	AuditScreen     = "Audit history"
	HealthScreen    = "Connection health"
	HelpScreen      = "Help"
)
