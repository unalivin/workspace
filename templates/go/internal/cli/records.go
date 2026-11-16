package cli

import (
	"context"
	"encoding/json"
	"errors"
	"flag"
	"fmt"
	"io"
	"os"
	"strconv"
	"strings"

	"github.com/unalivin/templateapp/internal/app"
	"github.com/unalivin/templateapp/internal/meta"
	"github.com/unalivin/templateapp/internal/records"
	"github.com/unalivin/templateapp/internal/theme"
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

func enabledText(enabled bool) string {
	if enabled {
		return "yes"
	}
	return "no"
}

type recordJSON struct {
	ID       int64  `json:"id"`
	Name     string `json:"name"`
	Status   string `json:"status"`
	Severity string `json:"severity"`
	Note     string `json:"note"`
	Enabled  bool   `json:"enabled"`
	Demo     bool   `json:"demo"`
	Revision int64  `json:"revision"`
	Created  string `json:"created_at"`
	Updated  string `json:"updated_at"`
}

func toJSON(record records.Record) recordJSON {
	return recordJSON{ID: record.ID, Name: record.Name, Status: record.Status, Severity: string(record.Severity), Note: record.Note, Enabled: record.Enabled, Demo: record.Demo, Revision: record.Revision, Created: record.CreatedAt.UTC().Format("2006-01-02T15:04:05Z"), Updated: record.UpdatedAt.UTC().Format("2006-01-02T15:04:05Z")}
}

func writeJSON(w io.Writer, value any) error {
	encoder := json.NewEncoder(w)
	encoder.SetIndent("", "  ")
	return encoder.Encode(value)
}

func recordInputFlags(set *flag.FlagSet) {
	set.String("name", "", "name, 1 to 200 characters")
	set.String("status", "", "open, in_progress or done")
	set.String("severity", "", "info, low, medium, high or critical")
	set.String("note", "", "free text, up to 4000 characters")
}

func overviewCommand() *command {
	return &command{name: "overview", summary: "Show what needs attention", flags: func(set *flag.FlagSet) {
		set.String("format", "table", "table or json")
	}, run: func(ctx context.Context, in *invocation) error {
		if err := noArguments(in); err != nil {
			return err
		}
		overview, err := in.app.Records.Overview(ctx)
		if err != nil {
			return err
		}
		switch in.text("format") {
		case "json":
			type row struct {
				Status   string `json:"status"`
				Severity string `json:"severity"`
				Total    int    `json:"total"`
				Demo     int    `json:"demo"`
			}
			out := struct {
				Open      int   `json:"open_records"`
				Urgent    int   `json:"urgent_records"`
				Disabled  int   `json:"disabled_records"`
				Demo      int   `json:"demo_records"`
				Unread    int   `json:"unread_notifications"`
				Breakdown []row `json:"breakdown"`
			}{overview.Open, overview.Urgent, overview.Disabled, overview.Demo, overview.UnreadNotifications, []row{}}
			for _, item := range overview.Breakdown {
				out.Breakdown = append(out.Breakdown, row{item.Status, string(item.Severity), item.Total, item.Demo})
			}
			return writeJSON(in.out(), out)
		case "table":
		default:
			return usagef("unknown format %q: use table or json", in.text("format"))
		}
		in.table([]string{"MEASURE", "COUNT"}, [][]string{
			{"Open records", strconv.Itoa(overview.Open)},
			{"Open records with high or critical severity", strconv.Itoa(overview.Urgent)},
			{"Disabled records", strconv.Itoa(overview.Disabled)},
			{"Demo records", strconv.Itoa(overview.Demo)},
			{"Unread notifications", strconv.Itoa(overview.UnreadNotifications)},
		})
		if overview.Total() == 0 && overview.Disabled == 0 {
			in.printf("\nNo records yet. Add one with \"%s records add --name <name>\" or load samples with \"%s demo load\".\n", meta.Slug, meta.Slug)
			return nil
		}
		in.printf("\nOpen and in-progress records by severity\n")
		var rows [][]string
		for index := len(theme.Severities) - 1; index >= 0; index-- {
			level := theme.Severities[index]
			rows = append(rows, []string{level.Glyph() + " " + level.Label(), strconv.Itoa(overview.BySeverity(level))})
		}
		in.table([]string{"SEVERITY", "COUNT"}, rows)
		if overview.Demo > 0 {
			in.printf("\nDemo data is loaded: %d sample records are labeled \"Demo:\".\n", overview.Demo)
		}
		return nil
	}}
}

func recordsCommand() *command {
	return &command{name: "records", summary: "Manage records", sub: []*command{
		{name: "list", summary: "List, search, filter and sort records", flags: func(set *flag.FlagSet) {
			set.String("search", "", "words to find in names and notes")
			set.String("status", "", "only this status")
			set.String("severity", "", "only this severity")
			set.Bool("all", false, "include disabled records")
			set.String("sort", "name", "name, status, severity or updated")
			set.Bool("desc", false, "sort in descending order")
			set.Int("limit", 500, "largest number of rows")
			set.String("format", "table", "table, json or csv")
		}, run: runRecordsList},
		{name: "show", summary: "Show one record", args: "<id>", flags: func(set *flag.FlagSet) {
			set.String("format", "table", "table or json")
		}, run: runRecordsShow},
		{name: "add", summary: "Add a record", flags: func(set *flag.FlagSet) {
			recordInputFlags(set)
			set.Bool("disabled", false, "add the record in the disabled state")
		}, run: runRecordsAdd},
		{name: "edit", summary: "Change fields of a record", args: "<id>", flags: recordInputFlags, run: runRecordsEdit},
		{name: "enable", summary: "Enable a record", args: "<id>", run: func(ctx context.Context, in *invocation) error { return runRecordsToggle(ctx, in, true) }},
		{name: "disable", summary: "Disable a record", args: "<id>", run: func(ctx context.Context, in *invocation) error { return runRecordsToggle(ctx, in, false) }},
		{name: "remove", summary: "Remove a record", args: "<id>", flags: func(set *flag.FlagSet) {
			set.Bool("yes", false, "do not ask for confirmation")
			set.Bool("dry-run", false, "show what would be removed without removing it")
		}, run: runRecordsRemove},
		{name: "import", summary: "Import records from a CSV or JSON file, or from standard input with -", args: "<file|->", flags: func(set *flag.FlagSet) {
			set.String("format", "", "csv or json, by default taken from the file extension")
			set.Bool("dry-run", false, "validate and report without storing anything")
		}, run: runRecordsImport},
		{name: "export", summary: "Export records as CSV or JSON", flags: func(set *flag.FlagSet) {
			set.String("format", "csv", "csv or json")
			set.String("output", "", "file to write, by default standard output")
			set.Bool("force", false, "overwrite the output file if it exists")
		}, run: runRecordsExport},
	}}
}

func recordRow(in *invocation, ctx context.Context, record records.Record) []string {
	return []string{strconv.FormatInt(record.ID, 10), record.Name, records.StatusGlyph(record.Status) + " " + records.StatusLabel(record.Status), record.Severity.Glyph() + " " + record.Severity.Label(), enabledText(record.Enabled), in.app.FormatTime(ctx, record.UpdatedAt)}
}

func runRecordsList(ctx context.Context, in *invocation) error {
	if err := noArguments(in); err != nil {
		return err
	}
	limit, _ := strconv.Atoi(in.text("limit"))
	list, err := in.app.Records.List(ctx, records.Query{Search: in.text("search"), Status: in.text("status"), Severity: theme.Severity(in.text("severity")), IncludeDisabled: in.truth("all"), Sort: in.text("sort"), Descending: in.truth("desc"), Limit: limit})
	if err != nil {
		return usageIfInvalid(err)
	}
	switch in.text("format") {
	case "json":
		out := make([]recordJSON, 0, len(list))
		for _, record := range list {
			out = append(out, toJSON(record))
		}
		return writeJSON(in.out(), out)
	case "csv":
		return records.Export(in.out(), records.FormatCSV, list)
	case "table":
	default:
		return usagef("unknown format %q: use table, json or csv", in.text("format"))
	}
	if len(list) == 0 {
		if in.text("search") != "" || in.text("status") != "" || in.text("severity") != "" {
			in.printf("No records match. Remove a filter or add --all to include disabled records.\n")
		} else {
			in.printf("No records yet. Add one with \"%s records add --name <name>\".\n", meta.Slug)
		}
		return nil
	}
	rows := make([][]string, 0, len(list))
	for _, record := range list {
		rows = append(rows, recordRow(in, ctx, record))
	}
	in.table([]string{"ID", "NAME", "STATUS", "SEVERITY", "ENABLED", "UPDATED"}, rows)
	return nil
}

func usageIfInvalid(err error) error {
	var invalid *records.ValidationError
	if errors.As(err, &invalid) {
		return usagef("%s", err.Error())
	}
	return err
}

func printRecord(ctx context.Context, in *invocation, record records.Record) {
	demo := ""
	if record.Demo {
		demo = " (demo data)"
	}
	in.table([]string{"FIELD", "VALUE"}, [][]string{
		{"ID", strconv.FormatInt(record.ID, 10)},
		{"Name", record.Name + demo},
		{"Status", records.StatusGlyph(record.Status) + " " + records.StatusLabel(record.Status)},
		{"Severity", record.Severity.Glyph() + " " + record.Severity.Label()},
		{"Enabled", enabledText(record.Enabled)},
		{"Revision", strconv.FormatInt(record.Revision, 10)},
		{"Created", in.app.FormatTime(ctx, record.CreatedAt)},
		{"Updated", in.app.FormatTime(ctx, record.UpdatedAt)},
		{"Note", strings.ReplaceAll(record.Note, "\n", " / ")},
	})
}

func runRecordsShow(ctx context.Context, in *invocation) error {
	id, err := parseID(in, "record")
	if err != nil {
		return err
	}
	record, err := in.app.Records.Get(ctx, id)
	if err != nil {
		return err
	}
	switch in.text("format") {
	case "json":
		return writeJSON(in.out(), toJSON(record))
	case "table":
		printRecord(ctx, in, record)
		return nil
	}
	return usagef("unknown format %q: use table or json", in.text("format"))
}

func runRecordsAdd(ctx context.Context, in *invocation) error {
	if err := noArguments(in); err != nil {
		return err
	}
	if !in.flagGiven("name") {
		return usagef("a name is required: add --name <name>")
	}
	created, err := in.app.Records.Create(ctx, records.Input{Name: in.text("name"), Status: in.text("status"), Severity: theme.Severity(in.text("severity")), Note: in.text("note"), Enabled: !in.truth("disabled")})
	if err != nil {
		return usageIfInvalid(err)
	}
	in.printf("Added record %d: %s\n", created.ID, created.Name)
	return nil
}

func runRecordsEdit(ctx context.Context, in *invocation) error {
	id, err := parseID(in, "record")
	if err != nil {
		return err
	}
	current, err := in.app.Records.Get(ctx, id)
	if err != nil {
		return err
	}
	change := current.Input()
	changed := false
	for name, target := range map[string]*string{"name": &change.Name, "status": &change.Status, "note": &change.Note} {
		if in.flagGiven(name) {
			*target, changed = in.text(name), true
		}
	}
	if in.flagGiven("severity") {
		change.Severity, changed = theme.Severity(in.text("severity")), true
	}
	if !changed {
		return usagef("nothing to change: give at least one of --name, --status, --severity or --note")
	}
	updated, err := in.app.Records.Update(ctx, id, current.Revision, change)
	if err != nil {
		return usageIfInvalid(err)
	}
	in.printf("Updated record %d (revision %d)\n", updated.ID, updated.Revision)
	return nil
}

func runRecordsToggle(ctx context.Context, in *invocation, enabled bool) error {
	id, err := parseID(in, "record")
	if err != nil {
		return err
	}
	updated, err := in.app.Records.SetEnabled(ctx, id, enabled)
	if err != nil {
		return err
	}
	state := "disabled"
	if updated.Enabled {
		state = "enabled"
	}
	in.printf("Record %d is %s\n", updated.ID, state)
	return nil
}

func runRecordsRemove(ctx context.Context, in *invocation) error {
	id, err := parseID(in, "record")
	if err != nil {
		return err
	}
	record, err := in.app.Records.Get(ctx, id)
	if err != nil {
		return err
	}
	statement := fmt.Sprintf("This removes record %d (%s) permanently.", record.ID, record.Name)
	if in.truth("dry-run") {
		in.printf("Dry run: %s Nothing was changed.\n", statement)
		return nil
	}
	if err := in.confirm(ctx, statement); err != nil {
		return err
	}
	if err := in.app.Records.Delete(ctx, id); err != nil {
		return err
	}
	in.printf("Removed record %d\n", id)
	return nil
}

func formatFromName(name, explicit string) (string, error) {
	format, err := app.TransferFormat(name, explicit)
	var invalid *app.FormatError
	if errors.As(err, &invalid) {
		if explicit == "" {
			return "", usagef("the format cannot be told from the file name: add --format csv or --format json")
		}
		return "", usagef("%s", invalid.Message)
	}
	return format, err
}

func runRecordsImport(ctx context.Context, in *invocation) error {
	if len(in.args) != 1 {
		return usagef("give one file to import, or - to read standard input")
	}
	format, err := formatFromName(in.args[0], in.text("format"))
	if err != nil {
		return err
	}
	var source io.Reader = in.env.Stdin
	if in.args[0] != "-" {
		file, err := os.Open(in.args[0])
		if err != nil {
			return fmt.Errorf("cannot open the import file: %w", err)
		}
		defer file.Close()
		source = file
	}
	result, err := in.app.Records.Import(ctx, source, format, in.truth("dry-run"))
	if err != nil {
		return err
	}
	in.printf("%s\n", capitalize(result.Summary()))
	for _, problem := range result.Problems {
		in.note("rejected %s", problem.Error())
	}
	if len(result.Problems) > 0 && result.Created+result.Updated+result.Skipped == 0 {
		return errors.New("no row could be imported: fix the rows listed above")
	}
	return nil
}

func capitalize(text string) string {
	if text == "" {
		return text
	}
	return strings.ToUpper(text[:1]) + text[1:]
}

func runRecordsExport(ctx context.Context, in *invocation) error {
	if err := noArguments(in); err != nil {
		return err
	}
	output := in.text("output")
	if output == "" {
		format, err := formatFromName("", in.text("format"))
		if err != nil {
			return err
		}
		content, _, err := in.app.ExportBytes(ctx, format)
		if err != nil {
			return err
		}
		_, err = in.out().Write(content)
		return err
	}
	count, err := in.app.ExportFile(ctx, output, in.text("format"), in.truth("force"))
	var exists *app.ExistsError
	var invalid *app.FormatError
	switch {
	case errors.As(err, &exists):
		return usagef("%s already exists: choose another name or add --force to overwrite it", output)
	case errors.As(err, &invalid):
		return usagef("%s", invalid.Message)
	case err != nil:
		return err
	}
	in.printf("Exported %d records to %s\n", count, output)
	return nil
}

func demoCommand() *command {
	return &command{name: "demo", summary: "Load or remove labeled demo data", sub: []*command{
		{name: "load", summary: "Add sample records labeled as demo data", run: func(ctx context.Context, in *invocation) error {
			if err := noArguments(in); err != nil {
				return err
			}
			added, err := in.app.Records.LoadDemo(ctx)
			if err != nil {
				return err
			}
			if added == 0 {
				in.printf("Demo data is already loaded.\n")
				return nil
			}
			in.printf("Added %d demo records. Their names start with \"Demo:\".\n", added)
			return nil
		}},
		{name: "remove", summary: "Remove every demo record", flags: func(set *flag.FlagSet) {
			set.Bool("yes", false, "do not ask for confirmation")
			set.Bool("dry-run", false, "show how many records would be removed")
		}, run: func(ctx context.Context, in *invocation) error {
			if err := noArguments(in); err != nil {
				return err
			}
			count, err := in.app.Records.RemoveDemo(ctx, true)
			if err != nil {
				return err
			}
			statement := fmt.Sprintf("This removes %d demo records. Records you added yourself stay.", count)
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
			removed, err := in.app.Records.RemoveDemo(ctx, false)
			if err != nil {
				return err
			}
			in.printf("Removed %d demo records\n", removed)
			return nil
		}},
	}}
}
