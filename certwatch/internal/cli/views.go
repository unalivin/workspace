package cli

import (
	"bufio"
	"context"
	"encoding/csv"
	"encoding/json"
	"errors"
	"flag"
	"fmt"
	"io"
	"os"
	"os/signal"
	"strconv"
	"strings"
	"time"

	"github.com/NexzaDev/certwatch/internal/app"
	"github.com/NexzaDev/certwatch/internal/inventory"
	"github.com/NexzaDev/certwatch/internal/meta"
	"github.com/NexzaDev/certwatch/internal/settings"
	"github.com/NexzaDev/certwatch/internal/words"
)

func parseID(in *invocation, what string) (int64, error) {
	if len(in.args) != 1 {
		return 0, usagef("give exactly one %s identifier: run \"%s\" to see them", what, meta.Slug+" "+in.command.parent.path()+" list")
	}
	id, err := strconv.ParseInt(in.args[0], 10, 64)
	if err != nil || id <= 0 {
		return 0, usagef("%q is not a %s identifier: use the number from the ID column", in.args[0], what)
	}
	return id, nil
}

func noArguments(in *invocation) error {
	if len(in.args) > 0 {
		return usagef("unexpected argument %q", in.args[0])
	}
	return nil
}

func writeJSON(w io.Writer, value any) error {
	encoder := json.NewEncoder(w)
	encoder.SetIndent("", "  ")
	return encoder.Encode(value)
}

func capitalize(text string) string {
	if text == "" {
		return text
	}
	return strings.ToUpper(text[:1]) + text[1:]
}

func usageIfInvalid(err error) error {
	var invalid *inventory.ValidationError
	var usage *app.UsageError
	var format *app.FormatError
	var setting *settings.ValidationError
	switch {
	case errors.Is(err, inventory.ErrNotAuthorized):
		return usagef("%s: repeat the command with --authorized", err.Error())
	case errors.As(err, &invalid), errors.As(err, &usage), errors.As(err, &format), errors.As(err, &setting):
		return usagef("%s", err.Error())
	}
	return err
}

func tableJSON(table app.Table) map[string]any {
	columns := make([]string, len(table.Columns))
	for i, column := range table.Columns {
		columns[i] = column.ID
	}
	rows := make([]map[string]any, 0, len(table.Rows))
	for _, row := range table.Rows {
		cells := map[string]string{}
		for i, cell := range row.Cells {
			cells[columns[i]] = cell
		}
		rows = append(rows, map[string]any{"id": row.ID, "severity": string(row.Severity), "demo": row.Demo, "cells": cells})
	}
	return map[string]any{"columns": columns, "rows": rows, "summary": table.Footer}
}

func (in *invocation) printTable(table app.Table, withID bool) {
	if len(table.Rows) == 0 {
		in.printf("%s\n", table.Empty)
		return
	}
	var header []string
	if withID {
		header = append(header, "ID")
	}
	for _, column := range table.Columns {
		header = append(header, strings.ToUpper(column.Title))
	}
	rows := make([][]string, 0, len(table.Rows))
	for _, row := range table.Rows {
		var cells []string
		if withID {
			cells = append(cells, strconv.FormatInt(row.ID, 10))
		}
		for _, cell := range row.Cells {
			if cell == "" {
				cell = "-"
			}
			cells = append(cells, cell)
		}
		rows = append(rows, cells)
	}
	in.table(header, rows)
}

func runViewList(view app.View) func(ctx context.Context, in *invocation) error {
	return func(ctx context.Context, in *invocation) error {
		if err := noArguments(in); err != nil {
			return err
		}
		name := "filter"
		if view.ID == app.ViewTimeline {
			name = "zoom"
		}
		filter := in.text(name)
		table, err := in.app.Table(ctx, view.ID, app.ViewQuery{Search: in.text("search"), Filter: filter})
		if err != nil {
			return usageIfInvalid(err)
		}
		switch in.text("format") {
		case "json":
			return writeJSON(in.out(), tableJSON(table))
		case "csv":
			writer := csv.NewWriter(in.out())
			header := []string{"id"}
			for _, column := range table.Columns {
				header = append(header, column.ID)
			}
			_ = writer.Write(header)
			for _, row := range table.Rows {
				cells := []string{strconv.FormatInt(row.ID, 10)}
				for _, cell := range row.Cells {
					cells = append(cells, inventory.GuardCell(cell))
				}
				_ = writer.Write(cells)
			}
			writer.Flush()
			return writer.Error()
		case "table":
		default:
			return usagef("unknown format %q: use table, json or csv", in.text("format"))
		}
		in.printTable(table, true)
		if len(table.Rows) > 0 && table.Footer != "" {
			in.printf("\n%s\n", capitalize(table.Footer))
		}
		return nil
	}
}

func detailJSON(detail app.Detail) map[string]any {
	sections := make([]map[string]any, 0, len(detail.Sections))
	for _, section := range detail.Sections {
		item := map[string]any{"title": section.Title}
		if len(section.Facts) > 0 {
			facts := make([]map[string]string, 0, len(section.Facts))
			for _, fact := range section.Facts {
				facts = append(facts, map[string]string{"label": fact[0], "value": fact[1]})
			}
			item["facts"] = facts
		}
		if section.Table != nil {
			item["table"] = tableJSON(*section.Table)
		}
		if section.Text != "" {
			item["text"] = section.Text
		}
		sections = append(sections, item)
	}
	return map[string]any{"title": detail.Title, "status": detail.Subtitle, "severity": string(detail.Severity), "sections": sections}
}

func (in *invocation) printDetail(detail app.Detail) {
	in.printf("%s\n%s\n", detail.Title, detail.Subtitle)
	for _, section := range detail.Sections {
		in.printf("\n%s\n", section.Title)
		if len(section.Facts) > 0 {
			rows := make([][]string, 0, len(section.Facts))
			for _, fact := range section.Facts {
				rows = append(rows, []string{"  " + fact[0], fact[1]})
			}
			in.table([]string{"  FIELD", "VALUE"}, rows)
		}
		if section.Table != nil {
			in.printTable(*section.Table, false)
		}
		if section.Text != "" {
			for _, line := range strings.Split(section.Text, "\n") {
				in.printf("  %s\n", line)
			}
		}
	}
}

func runViewShow(view app.View) func(ctx context.Context, in *invocation) error {
	return func(ctx context.Context, in *invocation) error {
		id, err := parseID(in, "entry")
		if err != nil {
			return err
		}
		detail, err := in.app.Detail(ctx, view.ID, id)
		if err != nil {
			return usageIfInvalid(err)
		}
		switch in.text("format") {
		case "json":
			return writeJSON(in.out(), detailJSON(detail))
		case "table":
		default:
			return usagef("unknown format %q: use table or json", in.text("format"))
		}
		in.printDetail(detail)
		return nil
	}
}

func (in *invocation) ask(statement string) error {
	if !in.interactive() {
		return usagef("%s This needs confirmation: repeat the command with --yes, or use --dry-run to preview", statement)
	}
	fmt.Fprintf(in.env.Stderr, "%s\nType yes to continue: ", statement)
	line, _ := bufio.NewReader(io.LimitReader(in.env.Stdin, 256)).ReadString('\n')
	if strings.ToLower(strings.TrimSpace(line)) != "yes" {
		return errors.New("cancelled, nothing was changed")
	}
	return nil
}

func (in *invocation) approve(plan app.Plan, canPreview bool) (bool, error) {
	if canPreview && in.truth("dry-run") {
		in.printf("Dry run: %s\n", plan.Statement)
		for _, line := range plan.Network {
			in.printf("  network: %s\n", line)
		}
		for _, line := range plan.Lines {
			in.printf("  %s\n", line)
		}
		in.printf("Nothing was changed.\n")
		return false, nil
	}
	for _, line := range plan.Network {
		in.note("network: %s", line)
	}
	if !plan.Confirm || (in.flags.Lookup("yes") != nil && in.truth("yes")) {
		return true, nil
	}
	return true, in.ask(plan.Statement)
}

func previewable(action app.Action) bool {
	return action.Network || action.Danger || action.Preview
}

func actionFlags(action app.Action) func(*flag.FlagSet) {
	return func(set *flag.FlagSet) {
		for _, field := range action.Fields {
			help := strings.TrimSuffix(field.Help, ".")
			if help == "" {
				help = field.Label
			}
			if !strings.HasPrefix(help, "TCP") {
				help = strings.ToLower(help[:1]) + help[1:]
			}
			if field.Kind == app.FieldChoice {
				ids := make([]string, len(field.Options))
				for i, option := range field.Options {
					ids[i] = option.ID
				}
				help = strings.Join(ids, ", ")
			}
			if field.Kind == app.FieldBool && field.Default != "true" {
				set.Bool(field.ID, false, help)
				continue
			}
			set.String(field.ID, "", help)
		}
		if action.Network || action.Danger {
			set.Bool("yes", false, "do not ask for confirmation")
		}
		if previewable(action) {
			set.Bool("dry-run", false, "show what would happen without doing it")
		}
	}
}

func runViewAction(view app.View, action app.Action) func(ctx context.Context, in *invocation) error {
	return func(ctx context.Context, in *invocation) error {
		request := app.Request{View: view.ID, Action: action.ID, Values: map[string]string{}}
		if action.Row {
			id, err := parseID(in, "entry")
			if err != nil {
				return err
			}
			request.ID = id
		} else if err := noArguments(in); err != nil {
			return err
		}
		for _, field := range action.Fields {
			if in.flagGiven(field.ID) {
				request.Values[field.ID] = in.text(field.ID)
			}
		}
		plan, err := in.app.Plan(ctx, request)
		if err != nil {
			return usageIfInvalid(err)
		}
		proceed, err := in.approve(plan, previewable(action))
		if err != nil || !proceed {
			return err
		}
		outcome, err := in.app.Run(ctx, request)
		if err != nil {
			return usageIfInvalid(err)
		}
		in.printf("%s\n", outcome.Message)
		for _, line := range outcome.Lines {
			in.printf("  %s\n", line)
		}
		return nil
	}
}

func viewCommand(view app.View) *command {
	filters := make([]string, len(view.Filters))
	for i, filter := range view.Filters {
		filters[i] = filter.ID
	}
	name := strings.ToLower(view.Title)
	if view.ID == app.ViewTimeline {
		name = "expiry timeline"
	}
	result := &command{name: view.ID, summary: capitalize(strings.TrimSuffix(view.Help, ".")), sub: []*command{
		{name: "list", summary: "List the " + name + " table", flags: func(set *flag.FlagSet) {
			set.String("search", "", "text to find")
			if view.ID == app.ViewTimeline {
				set.String("zoom", filters[0], "days to look ahead: "+strings.Join(filters, ", "))
			} else {
				set.String("filter", filters[0], strings.Join(filters, ", "))
			}
			set.String("format", "table", "table, json or csv")
		}, run: runViewList(view)},
		{name: "show", summary: "Show one entry in full", args: "<id>", flags: func(set *flag.FlagSet) {
			set.String("format", "table", "table or json")
		}, run: runViewShow(view)},
	}}
	for _, action := range view.Actions {
		item := &command{name: action.ID, summary: strings.TrimSuffix(action.Help, "."), flags: actionFlags(action), run: runViewAction(view, action)}
		if action.Row {
			item.args = "<id>"
		}
		result.sub = append(result.sub, item)
	}
	return result
}

func overviewCommand() *command {
	return &command{name: "overview", summary: "Show what expires soon and what failed validation", flags: func(set *flag.FlagSet) {
		set.String("format", "table", "table or json")
	}, run: func(ctx context.Context, in *invocation) error {
		if err := noArguments(in); err != nil {
			return err
		}
		overview, err := in.app.Overview(ctx)
		if err != nil {
			return err
		}
		switch in.text("format") {
		case "json":
			counts := map[string]int{"endpoints": overview.Counts.Endpoints, "disabled_endpoints": overview.Counts.DisabledEndpoints, "tracked_certificates": overview.Counts.Certificates, "findings": overview.Counts.Findings, "unread_alerts": overview.Counts.UnreadAlerts, "demo_rows": overview.Counts.DemoRows}
			for _, tile := range overview.Tiles {
				counts[tile.ID] = tile.Value
			}
			return writeJSON(in.out(), map[string]any{"counts": counts, "soonest": tableJSON(overview.Soonest), "alerts": tableJSON(overview.Alerts)})
		case "table":
		default:
			return usagef("unknown format %q: use table or json", in.text("format"))
		}
		var rows [][]string
		for _, tile := range overview.Tiles {
			glyph := ""
			if tile.Severity != "" {
				glyph = tile.Severity.Glyph()
			}
			label := fmt.Sprintf("%-2s %s", glyph, tile.Label)
			rows = append(rows, []string{label, strconv.Itoa(tile.Value), fmt.Sprintf("%s %s list --filter %s", meta.Slug, tile.View, tile.Filter)})
		}
		rows = append(rows, []string{"Tracked certificates", strconv.Itoa(overview.Counts.Certificates), meta.Slug + " certificates list"},
			[]string{"Endpoints", strconv.Itoa(overview.Counts.Endpoints), meta.Slug + " endpoints list"},
			[]string{"Unread alerts", strconv.Itoa(overview.Counts.UnreadAlerts), meta.Slug + " alerts list --unread"})
		in.table([]string{"MEASURE", "COUNT", "DETAILS"}, rows)
		if overview.Empty {
			in.printf("\nNothing is monitored yet. Add a host you are authorized to test with \"%s endpoints add --host <host> --authorized\", or load samples with \"%s demo load\".\n", meta.Slug, meta.Slug)
			return nil
		}
		in.printf("\nSoonest to expire\n")
		in.printTable(overview.Soonest, true)
		in.printf("\nRecent alerts\n")
		in.printTable(overview.Alerts, true)
		if overview.Demo {
			in.printf("\nDemo data is loaded. Demo hosts end in .demo.invalid and are never contacted.\n")
		}
		return nil
	}}
}

func watchCommand() *command {
	return &command{name: "watch", summary: "Check enabled endpoints at the configured interval until interrupted", flags: func(set *flag.FlagSet) {
		set.Bool("once", false, "check every endpoint that is due once, then exit")
		set.Int("interval", 0, "minutes between checks, by default the setting "+settings.KeyCheckInterval)
		set.Bool("yes", false, "do not ask for confirmation")
		set.Bool("dry-run", false, "list the connections that would be made, then exit")
	}, run: func(ctx context.Context, in *invocation) error {
		if err := noArguments(in); err != nil {
			return err
		}
		interval, err := in.app.CheckInterval(ctx)
		if err != nil {
			return err
		}
		if in.flagGiven("interval") {
			minutes, _ := strconv.Atoi(in.text("interval"))
			if minutes < 1 || minutes > 10080 {
				return usagef("--interval must be 1 to 10080 minutes")
			}
			interval = time.Duration(minutes) * time.Minute
		}
		once := in.truth("once")
		if interval <= 0 && !once {
			return usagef("automatic checks are off: give --interval <minutes>, set %s, or use --once", settings.KeyCheckInterval)
		}
		targets, err := in.app.Inventory.Endpoints(ctx, inventory.Query{Filter: "enabled"})
		if err != nil {
			return err
		}
		probe := false
		if value, err := in.app.Settings.Get(ctx, settings.KeyProbeProtocols); err == nil {
			probe = value.Bool()
		}
		network := app.Disclosure(targets, probe)
		statement := fmt.Sprintf("Check %s over the network every %s until interrupted.", words.Count(len(network), "enabled endpoint", "enabled endpoints"), interval)
		if once {
			statement = fmt.Sprintf("Check the enabled endpoints that are due over the network once (%d enabled).", len(network))
		}
		proceed, err := in.approve(app.Plan{Statement: statement, Network: network, Confirm: true}, true)
		if err != nil || !proceed {
			return err
		}
		pass := func() error {
			report, err := in.app.CheckDue(ctx, interval)
			if err != nil {
				return err
			}
			if alerts, err := in.app.Inventory.EvaluateExpiry(ctx, mustPolicy(ctx, in.app)); err == nil {
				report.Alerts += alerts
			}
			in.printf("%s %s\n", in.app.Now().UTC().Format(time.RFC3339), report.Summary())
			for _, line := range report.Lines {
				in.printf("  %s: %s (%s)\n", line.Address, line.Outcome, line.Detail)
			}
			return nil
		}
		if once {
			return pass()
		}
		ctx, stop := signal.NotifyContext(ctx, os.Interrupt)
		defer stop()
		for {
			if err := pass(); err != nil {
				if ctx.Err() != nil {
					return nil
				}
				return err
			}
			if !in.env.sleep(ctx, time.Minute) {
				in.printf("Stopped.\n")
				return nil
			}
		}
	}}
}

func mustPolicy(ctx context.Context, application *app.App) inventory.Policy {
	policy, err := application.Policy(ctx)
	if err != nil {
		return inventory.Policy{}
	}
	return policy
}

func demoCommand() *command {
	return &command{name: "demo", summary: "Load or remove labeled demo data", sub: []*command{
		{name: "load", summary: "Add sample endpoints and certificates labeled as demo data", run: func(ctx context.Context, in *invocation) error {
			if err := noArguments(in); err != nil {
				return err
			}
			added, err := in.app.LoadDemo(ctx)
			if err != nil {
				return err
			}
			if added == 0 {
				in.printf("Demo data is already loaded.\n")
				return nil
			}
			in.printf("Added %d demo endpoints with generated certificates. Their hosts end in .demo.invalid and are never contacted.\n", added)
			return nil
		}},
		{name: "remove", summary: "Remove every demo endpoint, certificate and alert", flags: func(set *flag.FlagSet) {
			set.Bool("yes", false, "do not ask for confirmation")
			set.Bool("dry-run", false, "show how many rows would be removed")
		}, run: func(ctx context.Context, in *invocation) error {
			if err := noArguments(in); err != nil {
				return err
			}
			count, err := in.app.Inventory.RemoveDemo(ctx, true)
			if err != nil {
				return err
			}
			statement := fmt.Sprintf("This removes %s with their alerts. Entries you added yourself stay.", words.Count(count, "demo endpoint or certificate", "demo endpoints and certificates"))
			if in.truth("dry-run") {
				in.printf("Dry run: %s Nothing was changed.\n", statement)
				return nil
			}
			if count == 0 {
				in.printf("There is no demo data to remove.\n")
				return nil
			}
			if err := in.confirm(ctx, statement); err != nil {
				return err
			}
			removed, err := in.app.Inventory.RemoveDemo(ctx, false)
			if err != nil {
				return err
			}
			in.printf("Removed %s\n", words.Count(removed, "demo endpoint or certificate", "demo endpoints and certificates"))
			return nil
		}},
	}}
}

func enabledText(enabled bool) string {
	if enabled {
		return "yes"
	}
	return "no"
}
