package cli

import (
	"context"
	"errors"
	"flag"
	"fmt"
	"strings"

	"github.com/NexzaDev/certwatch/internal/app"
	"github.com/NexzaDev/certwatch/internal/guide"
	"github.com/NexzaDev/certwatch/internal/meta"
	"github.com/NexzaDev/certwatch/internal/settings"
	"github.com/NexzaDev/certwatch/internal/theme"
	"github.com/NexzaDev/certwatch/internal/userconfig"
)

func configCommand() *command {
	return &command{name: "config", summary: "Show and change settings", sub: []*command{
		{name: "list", summary: "List every setting with its value and where the value comes from", flags: func(set *flag.FlagSet) {
			set.String("format", "table", "table or json")
		}, run: runConfigList},
		{name: "get", summary: "Print the value of one setting", args: "<key>", run: runConfigGet},
		{name: "set", summary: "Change a setting", args: "<key> <value>", run: runConfigSet},
		{name: "reset", summary: "Return one setting to its default", args: "<key>", run: runConfigReset},
		{name: "theme", summary: "Choose the color theme", sub: []*command{
			{name: "list", summary: "List the color themes", run: runThemeList},
			{name: "set", summary: "Choose a color theme", args: "<name>", run: runThemeSet},
			{name: "show", summary: "Show the theme, mode and colors in use", run: runThemeShow},
		}},
		{name: "launcher", summary: "Choose which front end opens by default", sub: []*command{
			{name: "show", summary: "Show the remembered front end", run: runLauncherShow},
			{name: "set", summary: "Remember a front end", args: "<gui|tui>", run: runLauncherSet},
			{name: "reset", summary: "Forget the choice and ask again", run: runLauncherReset},
		}},
	}}
}

func settingError(err error) error {
	var invalid *settings.ValidationError
	if errors.As(err, &invalid) {
		return usagef("%s", err.Error())
	}
	return err
}

func sourceText(value settings.Value) string {
	if value.OverriddenBy != "" {
		return "overridden by " + value.OverriddenBy
	}
	return string(value.Source)
}

func displayValue(value settings.Value) string {
	if value.Raw == "" {
		return "(empty)"
	}
	return value.Raw
}

func runConfigList(ctx context.Context, in *invocation) error {
	if err := noArguments(in); err != nil {
		return err
	}
	values, err := in.app.Settings.List(ctx)
	if err != nil {
		return err
	}
	switch in.text("format") {
	case "json":
		type row struct {
			Key          string `json:"key"`
			Value        string `json:"value"`
			Default      string `json:"default"`
			Source       string `json:"source"`
			OverriddenBy string `json:"overridden_by,omitempty"`
			Problem      string `json:"problem,omitempty"`
		}
		out := make([]row, 0, len(values))
		for _, value := range values {
			out = append(out, row{value.Key, value.Raw, value.Default, string(value.Source), value.OverriddenBy, value.OverrideError})
		}
		return writeJSON(in.out(), out)
	case "table":
	default:
		return usagef("unknown format %q: use table or json", in.text("format"))
	}
	rows := make([][]string, 0, len(values))
	for _, value := range values {
		rows = append(rows, []string{value.Key, displayValue(value), sourceText(value), value.Label})
	}
	in.table([]string{"KEY", "VALUE", "SOURCE", "MEANING"}, rows)
	for _, value := range values {
		if value.OverrideError != "" {
			in.note("note: %s: %s", value.Key, value.OverrideError)
		}
	}
	return nil
}

func runConfigGet(ctx context.Context, in *invocation) error {
	if len(in.args) != 1 {
		return usagef("give one setting key: run \"%s config list\" to see them", meta.Slug)
	}
	value, err := in.app.Settings.Get(ctx, in.args[0])
	if err != nil {
		return settingError(err)
	}
	in.printf("%s\n", value.Raw)
	if value.OverriddenBy != "" {
		in.note("note: overridden by %s, the stored value is %q", value.OverriddenBy, value.Stored)
	}
	return nil
}

func runConfigSet(ctx context.Context, in *invocation) error {
	if len(in.args) != 2 {
		return usagef("give a key and a value, for example: %s config set %s compact", meta.Slug, settings.KeyDensity)
	}
	value, err := in.app.Settings.Set(ctx, in.args[0], in.args[1])
	if err != nil {
		return settingError(err)
	}
	in.printf("%s = %s\n", value.Key, displayValue(settings.Value{Raw: value.Stored}))
	if value.OverriddenBy != "" {
		in.note("note: saved, but the %s still takes precedence with %q", value.OverriddenBy, value.Raw)
	}
	return nil
}

func runConfigReset(ctx context.Context, in *invocation) error {
	if len(in.args) != 1 {
		return usagef("give one setting key: run \"%s config list\" to see them", meta.Slug)
	}
	value, err := in.app.Settings.Reset(ctx, in.args[0])
	if err != nil {
		return settingError(err)
	}
	in.printf("%s is back to its default: %s\n", value.Key, displayValue(settings.Value{Raw: value.Default}))
	if value.OverriddenBy != "" {
		in.note("note: the %s still takes precedence with %q", value.OverriddenBy, value.Raw)
	}
	return nil
}

func runThemeList(ctx context.Context, in *invocation) error {
	if err := noArguments(in); err != nil {
		return err
	}
	current, err := in.app.Settings.Get(ctx, settings.KeyTheme)
	if err != nil {
		return err
	}
	var rows [][]string
	for _, name := range theme.Names() {
		marker := ""
		if name == current.Raw {
			marker = "in use"
		}
		light := theme.Resolve(theme.Options{Name: name, Mode: theme.ModeLight})
		dark := theme.Resolve(theme.Options{Name: name, Mode: theme.ModeDark})
		rows = append(rows, []string{name, theme.Label(name), light.Palette.Accent.Hex(), dark.Palette.Accent.Hex(), marker})
	}
	in.table([]string{"NAME", "LABEL", "LIGHT ACCENT", "DARK ACCENT", ""}, rows)
	return nil
}

func runThemeSet(ctx context.Context, in *invocation) error {
	if len(in.args) != 1 {
		return usagef("give one theme name: %s", strings.Join(theme.Names(), ", "))
	}
	value, err := in.app.Settings.Set(ctx, settings.KeyTheme, in.args[0])
	if err != nil {
		return settingError(err)
	}
	in.printf("Color theme: %s (%s)\n", value.Stored, theme.Label(value.Stored))
	if value.OverriddenBy != "" {
		in.note("note: saved, but the %s still takes precedence with %q", value.OverriddenBy, value.Raw)
	}
	return nil
}

func runThemeShow(ctx context.Context, in *invocation) error {
	if err := noArguments(in); err != nil {
		return err
	}
	values, err := in.app.Settings.Effective(ctx)
	if err != nil {
		return err
	}
	mode := values[settings.KeyMode]
	dark := mode.Raw == "dark"
	tokens, err := in.app.Appearance(ctx, dark)
	if err != nil {
		return err
	}
	shown := "light"
	if tokens.Dark {
		shown = "dark"
	}
	modeText := mode.Raw
	if mode.Raw == "system" {
		modeText = "system (the terminal cannot ask the system, light colors are listed)"
	}
	profile := theme.DetectProfile(in.env.Environment.Getenv, values[settings.KeyMonochrome].Bool())
	in.table([]string{"ITEM", "VALUE"}, [][]string{
		{"Theme", tokens.Name + " (" + tokens.Label + "), " + sourceText(values[settings.KeyTheme])},
		{"Mode", modeText},
		{"Colors listed", shown},
		{"Terminal colors", profile.String()},
		{"Density", values[settings.KeyDensity].Raw},
		{"Text size", values[settings.KeyTextScale].Raw + " %"},
		{"Reduced motion", values[settings.KeyReducedMotion].Raw},
		{"Background", tokens.Palette.Background.Hex()},
		{"Surface", tokens.Palette.Surface.Hex()},
		{"Text", fmt.Sprintf("%s (contrast %.1f:1 on surface)", tokens.Palette.Text.Hex(), theme.Contrast(tokens.Palette.Text, tokens.Palette.Surface))},
		{"Accent", fmt.Sprintf("%s (contrast %.1f:1 on surface)", tokens.Palette.Accent.Hex(), theme.Contrast(tokens.Palette.Accent, tokens.Palette.Surface))},
	})
	for _, warning := range tokens.Warnings {
		in.note("note: %s", warning)
	}
	return nil
}

func frontEndLabel(mode string) string {
	switch mode {
	case userconfig.FrontEndGUI:
		return "gui (dashboard window)"
	case userconfig.FrontEndTUI:
		return "tui (terminal UI)"
	}
	return "not set, the application asks at start"
}

func runLauncherShow(_ context.Context, in *invocation) error {
	if err := noArguments(in); err != nil {
		return err
	}
	in.printf("Open with: %s\n", frontEndLabel(in.app.FrontEnd()))
	if value := in.env.Environment.Getenv(meta.EnvName("ui")); value != "" {
		in.note("note: overridden by environment variable %s=%s", meta.EnvName("ui"), value)
	}
	return nil
}

func runLauncherSet(_ context.Context, in *invocation) error {
	if len(in.args) != 1 || (in.args[0] != userconfig.FrontEndGUI && in.args[0] != userconfig.FrontEndTUI) {
		return usagef("give gui or tui")
	}
	if err := in.app.RememberFrontEnd(in.args[0]); err != nil {
		return err
	}
	in.printf("Open with: %s\n", frontEndLabel(in.args[0]))
	return nil
}

func runLauncherReset(_ context.Context, in *invocation) error {
	if err := noArguments(in); err != nil {
		return err
	}
	if err := in.app.RememberFrontEnd(""); err != nil {
		return err
	}
	in.printf("Open with: %s\n", frontEndLabel(""))
	return nil
}

func databaseCommand() *command {
	return &command{name: "db", summary: "Create, locate, move and check the database", sub: []*command{
		{name: "init", summary: "Create the database if it does not exist", run: func(_ context.Context, in *invocation) error {
			if err := noArguments(in); err != nil {
				return err
			}
			if in.app.CreatedDatabase() {
				in.printf("Database ready at %s\n", in.app.Location().Path)
			} else {
				in.printf("The database already exists at %s\n", in.app.Location().Path)
			}
			return nil
		}},
		{name: "path", summary: "Print the database path and what decides it", noApp: true, run: func(_ context.Context, in *invocation) error {
			if err := noArguments(in); err != nil {
				return err
			}
			location, err := app.Locate(in.env.appOptions(in.globals, false))
			if err != nil {
				return err
			}
			in.printf("%s\n", location.Path)
			state := "exists"
			if !location.Exists {
				state = "not created yet"
			}
			in.note("set by: %s, %s", sourceOfLocation(location), state)
			return nil
		}},
		{name: "move", summary: "Copy the database to a new location and use it, or switch to another database", args: "<path>", flags: func(set *flag.FlagSet) {
			set.Bool("switch", false, "use the database at the path as it is, or create an empty one there, instead of copying")
			set.Bool("yes", false, "do not ask for confirmation")
			set.Bool("dry-run", false, "show what would change without changing it")
		}, run: runDatabaseMove},
		{name: "check", summary: "Run the integrity and foreign-key checks", run: func(ctx context.Context, in *invocation) error {
			if err := noArguments(in); err != nil {
				return err
			}
			if err := in.app.Store.IntegrityCheck(ctx); err != nil {
				return err
			}
			if err := in.app.Store.ForeignKeyCheck(ctx); err != nil {
				return err
			}
			version, err := in.app.Store.SchemaVersion(ctx)
			if err != nil {
				return err
			}
			in.printf("Database is healthy: integrity check ok, foreign keys ok, schema version %d\n", version)
			return nil
		}},
	}}
}

func sourceOfLocation(location app.Location) string {
	if location.OverriddenBy != "" {
		return location.OverriddenBy
	}
	if location.Source == "saved" {
		return "saved location"
	}
	return "default location"
}

func runDatabaseMove(ctx context.Context, in *invocation) error {
	if len(in.args) != 1 {
		return usagef("give the new database path")
	}
	mode := app.MoveCopy
	if in.truth("switch") {
		mode = app.MoveSwitch
	}
	plan, err := in.app.PlanMove(in.args[0], mode)
	if err != nil {
		return usagef("%v", err)
	}
	if in.truth("dry-run") {
		in.printf("Dry run: %s Nothing was changed.\n", plan.Statement)
		return nil
	}
	if err := in.confirm(ctx, plan.Statement); err != nil {
		return err
	}
	if err := in.app.Move(ctx, plan); err != nil {
		return err
	}
	in.printf("Now using %s\n", in.app.Location().Path)
	return nil
}

func runDiagnostics(ctx context.Context, in *invocation) error {
	if err := noArguments(in); err != nil {
		return err
	}
	report := in.app.Diagnostics(ctx, in.env.OpenGUI != nil)
	rows := make([][]string, 0, 20)
	for _, line := range report.Lines() {
		rows = append(rows, []string{line[0], line[1]})
	}
	in.table([]string{"ITEM", "VALUE"}, rows)
	if report.Integrity != "ok" {
		return errors.New("the integrity check failed: restore the database from a backup or export the records and create a new database")
	}
	return nil
}

func runGuide(_ context.Context, in *invocation) error {
	usage := app.UsageGuide()
	if len(in.args) == 0 {
		in.printf("%s usage guide\n\nContents:\n", meta.AppName)
		for _, section := range usage.Contents() {
			in.printf("%s%s\n", strings.Repeat("  ", section.Level), section.Title)
		}
		in.printf("\nRead a topic with \"%s guide <words>\".\n", meta.Slug)
		return nil
	}
	hits := usage.Search(strings.Join(in.args, " "))
	if len(hits) == 0 {
		return fmt.Errorf("no section of the usage guide mentions %q: run \"%s guide\" for the contents", strings.Join(in.args, " "), meta.Slug)
	}
	for index, hit := range hits {
		if index > 0 {
			in.printf("\n")
		}
		section := usage.Sections[hit.Section]
		in.printf("%s\n%s\n\n%s\n", section.Title, strings.Repeat("=", len([]rune(section.Title))), guide.Plain(section.Body))
	}
	return nil
}

func runAbout(ctx context.Context, in *invocation) error {
	if err := noArguments(in); err != nil {
		return err
	}
	if in.truth("issue") {
		issue := in.app.Diagnostics(ctx, in.env.OpenGUI != nil).IssueReport()
		in.printf("Found a bug? Please report it on GitHub.\n\nThe report form is prefilled with exactly this text and nothing else:\n\n%s\nOpen this link in a browser to continue:\n%s\n", issue.Body, issue.URL)
		return nil
	}
	in.table([]string{"ITEM", "VALUE"}, [][]string{
		{"Application", meta.AppName + " " + meta.Version},
		{"About", meta.Tagline},
		{"License", meta.License},
		{"Source Code", app.SourceLink().URL},
		{"Report a bug", meta.RepositoryURL + "/issues/new"},
		{"Report a vulnerability", app.AdvisoryLink().URL},
	})
	in.printf("\n%s\n", app.PrivacyNote)
	in.printf("\nRun \"%s about --issue\" to preview a bug report. Nothing is opened or sent by this command.\n", meta.Slug)
	return nil
}

func runLicenses(_ context.Context, in *invocation) error {
	if err := noArguments(in); err != nil {
		return err
	}
	in.printf("%s\n\n%s\n", app.LicenseText(), app.NoticesText())
	return nil
}
