package cli

import (
	"context"
	"errors"
	"flag"
	"fmt"
	"os/signal"
	"strconv"
	"strings"
	"syscall"

	"github.com/NexzaDev/templateapp/internal/meta"
	"github.com/NexzaDev/templateapp/internal/notify"
	"github.com/NexzaDev/templateapp/internal/theme"
)

func notificationsCommand() *command {
	return &command{name: "notifications", summary: "Read notifications and manage their rules", sub: []*command{
		{name: "list", summary: "List notifications, newest first", flags: func(set *flag.FlagSet) {
			set.Bool("unread", false, "only unread notifications")
			set.String("severity", "", "only this severity")
			set.String("event", "", "only this event type")
			set.Int("limit", 200, "largest number of rows")
			set.String("format", "table", "table or json")
		}, run: runNotificationsList},
		{name: "read", summary: "Mark one notification, or all of them, as read", args: "<id|all>", run: runNotificationsRead},
		{name: "rules", summary: "Show or change the rule of each event type", sub: []*command{
			{name: "list", summary: "List the rule of every event type", run: runRulesList},
			{name: "set", summary: "Change the rule of an event type", args: "<event>", flags: func(set *flag.FlagSet) {
				set.String("enabled", "", "true or false")
				set.String("min-severity", "", "lowest severity that raises a notification")
				set.String("quiet-start", "", "start of quiet hours as HH:MM, empty for none")
				set.String("quiet-end", "", "end of quiet hours as HH:MM, empty for none")
				set.String("dedup-minutes", "", "minutes in which a repeated event is not raised again, 0 for off")
			}, run: runRulesSet},
			{name: "reset", summary: "Return the rule of an event type to its defaults", args: "<event>", run: runRulesReset},
		}},
	}}
}

func runNotificationsList(ctx context.Context, in *invocation) error {
	if err := noArguments(in); err != nil {
		return err
	}
	limit, _ := strconv.Atoi(in.text("limit"))
	list, err := in.app.Notices.List(ctx, notify.Filter{UnreadOnly: in.truth("unread"), Severity: theme.Severity(in.text("severity")), EventType: in.text("event"), Limit: limit})
	if err != nil {
		return usagef("%v", err)
	}
	switch in.text("format") {
	case "json":
		type row struct {
			ID       int64  `json:"id"`
			Event    string `json:"event_type"`
			Severity string `json:"severity"`
			Title    string `json:"title"`
			Kind     string `json:"record_kind"`
			RecordID int64  `json:"record_id"`
			Created  string `json:"created_at"`
			Read     bool   `json:"read"`
		}
		out := make([]row, 0, len(list))
		for _, item := range list {
			out = append(out, row{item.ID, item.EventType, string(item.Severity), item.Title, item.RecordKind, item.RecordID, item.CreatedAt.UTC().Format("2006-01-02T15:04:05Z"), !item.Unread()})
		}
		return writeJSON(in.out(), out)
	case "table":
	default:
		return usagef("unknown format %q: use table or json", in.text("format"))
	}
	if len(list) == 0 {
		in.printf("No notifications. They appear when a record needs attention or is completed.\n")
		return nil
	}
	rows := make([][]string, 0, len(list))
	for _, item := range list {
		state, target := "unread", "-"
		if !item.Unread() {
			state = "read"
		}
		if item.RecordID != 0 {
			target = fmt.Sprintf("%s %d", item.RecordKind, item.RecordID)
		}
		rows = append(rows, []string{strconv.FormatInt(item.ID, 10), in.app.FormatTime(ctx, item.CreatedAt), item.Severity.Glyph() + " " + item.Severity.Label(), state, item.Title, target})
	}
	in.table([]string{"ID", "WHEN", "SEVERITY", "STATE", "TITLE", "LINK"}, rows)
	return nil
}

func runNotificationsRead(ctx context.Context, in *invocation) error {
	if len(in.args) != 1 {
		return usagef("give one notification identifier or the word all")
	}
	if in.args[0] == "all" {
		changed, err := in.app.Notices.MarkAllRead(ctx)
		if err != nil {
			return err
		}
		in.printf("Marked %d notifications as read\n", changed)
		return nil
	}
	id, err := strconv.ParseInt(in.args[0], 10, 64)
	if err != nil || id <= 0 {
		return usagef("%q is not a notification identifier: use the number from the ID column or the word all", in.args[0])
	}
	if err := in.app.Notices.MarkRead(ctx, id); err != nil {
		return err
	}
	in.printf("Marked notification %d as read\n", id)
	return nil
}

func quietText(rule notify.Rule) string {
	if rule.QuietStart == "" {
		return "none"
	}
	return rule.QuietStart + " to " + rule.QuietEnd
}

func runRulesList(ctx context.Context, in *invocation) error {
	if err := noArguments(in); err != nil {
		return err
	}
	rules, err := in.app.Notices.Rules(ctx)
	if err != nil {
		return err
	}
	labels := map[string]string{}
	for _, event := range notify.EventTypes() {
		labels[event.ID] = event.Label
	}
	rows := make([][]string, 0, len(rules))
	for _, rule := range rules {
		source := "default"
		if rule.Customized {
			source = "changed"
		}
		rows = append(rows, []string{rule.EventType, labels[rule.EventType], enabledText(rule.Enabled), rule.MinSeverity.Label(), quietText(rule), strconv.Itoa(rule.DedupMinutes), source})
	}
	in.table([]string{"EVENT", "MEANING", "ENABLED", "MIN SEVERITY", "QUIET HOURS", "DEDUP MINUTES", "SOURCE"}, rows)
	return nil
}

func eventArgument(in *invocation) (string, error) {
	if len(in.args) != 1 {
		var names []string
		for _, event := range notify.EventTypes() {
			names = append(names, event.ID)
		}
		return "", usagef("give one event type: %s", strings.Join(names, ", "))
	}
	return in.args[0], nil
}

func ruleError(err error) error {
	var invalid *notify.RuleError
	if errors.As(err, &invalid) {
		return usagef("%s", err.Error())
	}
	return err
}

func runRulesSet(ctx context.Context, in *invocation) error {
	event, err := eventArgument(in)
	if err != nil {
		return err
	}
	rule, err := in.app.Notices.Rule(ctx, event)
	if err != nil {
		return ruleError(err)
	}
	changed := false
	if in.flagGiven("enabled") {
		switch strings.ToLower(in.text("enabled")) {
		case "true", "yes", "on", "1":
			rule.Enabled = true
		case "false", "no", "off", "0":
			rule.Enabled = false
		default:
			return usagef("--enabled needs true or false, found %q", in.text("enabled"))
		}
		changed = true
	}
	if in.flagGiven("min-severity") {
		rule.MinSeverity, changed = theme.Severity(strings.ToLower(in.text("min-severity"))), true
	}
	if in.flagGiven("quiet-start") {
		rule.QuietStart, changed = in.text("quiet-start"), true
	}
	if in.flagGiven("quiet-end") {
		rule.QuietEnd, changed = in.text("quiet-end"), true
	}
	if in.flagGiven("dedup-minutes") {
		minutes, err := strconv.Atoi(in.text("dedup-minutes"))
		if err != nil {
			return usagef("--dedup-minutes needs a whole number, found %q", in.text("dedup-minutes"))
		}
		rule.DedupMinutes, changed = minutes, true
	}
	if !changed {
		return usagef("nothing to change: give at least one of --enabled, --min-severity, --quiet-start, --quiet-end or --dedup-minutes")
	}
	if err := in.app.Notices.SetRule(ctx, rule); err != nil {
		return ruleError(err)
	}
	in.printf("Rule for %s: enabled %s, minimum severity %s, quiet hours %s, de-duplication %d minutes\n", rule.EventType, enabledText(rule.Enabled), rule.MinSeverity.Label(), quietText(rule), rule.DedupMinutes)
	return nil
}

func runRulesReset(ctx context.Context, in *invocation) error {
	event, err := eventArgument(in)
	if err != nil {
		return err
	}
	if err := in.app.Notices.ResetRule(ctx, event); err != nil {
		return ruleError(err)
	}
	in.printf("Rule for %s is back to its defaults\n", event)
	return nil
}

func notifierCommand() *command {
	dryRun := func(set *flag.FlagSet) {
		set.Bool("dry-run", false, "show what would change without changing it")
	}
	return &command{name: "notifier", summary: "Show system notifications while the application is closed (opt-in)", sub: []*command{
		{name: "run", summary: "Check for new notifications on an interval until stopped", flags: func(set *flag.FlagSet) {
			set.Bool("once", false, "check one time and exit")
		}, run: runNotifier},
		{name: "install", summary: "Start the notifier when you sign in (per-user entry, no administrator rights)", flags: dryRun, noApp: true, run: runNotifierInstall},
		{name: "uninstall", summary: "Remove the sign-in entry", flags: dryRun, noApp: true, run: runNotifierUninstall},
		{name: "status", summary: "Show whether the sign-in entry exists", noApp: true, run: runNotifierStatus},
	}}
}

func runNotifier(ctx context.Context, in *invocation) error {
	if err := noArguments(in); err != nil {
		return err
	}
	if in.env.Deliverer == nil {
		return errors.New("this build cannot show system notifications")
	}
	runner, err := in.app.Notifier(in.env.Deliverer, "")
	if err != nil {
		return err
	}
	if err := in.app.Close(); err != nil {
		return err
	}
	if in.truth("once") {
		shown, err := runner.Tick(ctx)
		if err != nil {
			return err
		}
		in.printf("Checked once: %d new notifications shown\n", shown)
		return nil
	}
	runner.Log = func(line string) { in.note("notifier: %s", line) }
	in.note("%s notifier is running, checking every %s. It reads the database and makes no network requests. Press Ctrl+C to stop.", meta.AppName, notify.ClampInterval(runner.Interval))
	stoppable, stop := signal.NotifyContext(ctx, syscall.SIGINT, syscall.SIGTERM)
	defer stop()
	return runner.Run(stoppable)
}

func autostart(in *invocation) (notify.Autostart, error) {
	if in.env.Autostart == nil {
		return notify.Autostart{}, errors.New("this build cannot manage the sign-in entry")
	}
	return in.env.Autostart(), nil
}

func runNotifierInstall(_ context.Context, in *invocation) error {
	entry, err := autostart(in)
	if err != nil {
		return err
	}
	path, err := entry.Install(in.truth("dry-run"))
	if err != nil {
		return err
	}
	if in.truth("dry-run") {
		in.printf("Dry run: would write %s to start \"%s notifier run\" at sign-in. Nothing was changed.\n", path, entry.Executable)
		return nil
	}
	in.printf("Wrote %s. The notifier starts the next time you sign in. Start it now with \"%s notifier run\".\n", path, meta.Slug)
	return nil
}

func runNotifierUninstall(_ context.Context, in *invocation) error {
	entry, err := autostart(in)
	if err != nil {
		return err
	}
	removed, path, err := entry.Uninstall(in.truth("dry-run"))
	if err != nil {
		return err
	}
	switch {
	case !removed:
		in.printf("The notifier is not set to start at sign-in (%s does not exist).\n", path)
	case in.truth("dry-run"):
		in.printf("Dry run: would remove %s. Nothing was changed.\n", path)
	default:
		in.printf("Removed %s. A notifier that is running now keeps running until you sign out or stop it.\n", path)
	}
	return nil
}

func runNotifierStatus(_ context.Context, in *invocation) error {
	entry, err := autostart(in)
	if err != nil {
		return err
	}
	installed, path, err := entry.Status()
	if err != nil {
		return err
	}
	if installed {
		in.printf("Start at sign-in: on (%s)\n", path)
	} else {
		in.printf("Start at sign-in: off (%s does not exist)\n", path)
	}
	in.printf("Display of system notifications depends on the desktop and is not verified by the automated tests.\n")
	return nil
}
