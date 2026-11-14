package cli

import (
	"bufio"
	"context"
	"errors"
	"flag"
	"fmt"
	"io"
	"sort"
	"strings"
	"text/tabwriter"
	"time"

	"github.com/NexzaDev/templateapp/internal/app"
	"github.com/NexzaDev/templateapp/internal/launcher"
	"github.com/NexzaDev/templateapp/internal/meta"
	"github.com/NexzaDev/templateapp/internal/notify"
	"github.com/NexzaDev/templateapp/internal/settings"
	"github.com/NexzaDev/templateapp/internal/userconfig"
)

const (
	ExitOK      = 0
	ExitFailure = 1
	ExitUsage   = 2
)

type FrontEnd func(ctx context.Context, application *app.App) (switchTo string, err error)

type Env struct {
	Args               []string
	Stdin              io.Reader
	Stdout             io.Writer
	Stderr             io.Writer
	Environment        userconfig.Environment
	Now                func() time.Time
	Session            launcher.Session
	Executable         string
	OpenGUI            FrontEnd
	OpenTUI            FrontEnd
	ShowChooser        func(defaultMode string) (launcher.PromptResult, error)
	OpenTerminalWindow func(executable string, arguments []string) error
	Deliverer          notify.Deliverer
	Autostart          func() notify.Autostart
}

type usageError struct{ message string }

func (e *usageError) Error() string { return e.message }

func usagef(format string, arguments ...any) error {
	return &usageError{message: fmt.Sprintf(format, arguments...)}
}

type globals struct {
	database  string
	overrides map[string]string
	ui        string
	ask       bool
	noPrompt  bool
	help      bool
	version   bool
}

type overrideFlag struct{ values map[string]string }

func (o *overrideFlag) String() string { return "" }

func (o *overrideFlag) Set(value string) error {
	key, raw, found := strings.Cut(value, "=")
	if !found || key == "" {
		return errors.New("use the form key=value")
	}
	if _, known := settings.Lookup(key); !known {
		return fmt.Errorf("unknown setting %q: run \"%s config list\" to see the keys", key, meta.Slug)
	}
	o.values[key] = raw
	return nil
}

type invocation struct {
	env     *Env
	globals globals
	flags   *flag.FlagSet
	args    []string
	app     *app.App
	command *command
}

type command struct {
	name     string
	summary  string
	args     string
	sub      []*command
	flags    func(*flag.FlagSet)
	run      func(ctx context.Context, in *invocation) error
	noApp    bool
	readOnly bool
	parent   *command
}

func (c *command) path() string {
	if c.parent == nil || c.parent.parent == nil {
		return c.name
	}
	return c.parent.path() + " " + c.name
}

func (c *command) find(name string) *command {
	for _, child := range c.sub {
		if child.name == name {
			return child
		}
	}
	return nil
}

func link(parent *command) *command {
	for _, child := range parent.sub {
		child.parent = parent
		link(child)
	}
	return parent
}

func (c *command) flagSet() *flag.FlagSet {
	set := flag.NewFlagSet(c.path(), flag.ContinueOnError)
	set.SetOutput(io.Discard)
	if c.flags != nil {
		c.flags(set)
	}
	return set
}

type boolFlag interface{ IsBoolFlag() bool }

func permute(set *flag.FlagSet, arguments []string) ([]string, []string) {
	var flags, positional []string
	for index := 0; index < len(arguments); index++ {
		argument := arguments[index]
		if argument == "--" {
			positional = append(positional, arguments[index+1:]...)
			break
		}
		if !strings.HasPrefix(argument, "-") || argument == "-" {
			positional = append(positional, argument)
			continue
		}
		flags = append(flags, argument)
		name := strings.TrimLeft(argument, "-")
		if strings.Contains(name, "=") {
			continue
		}
		defined := set.Lookup(name)
		if defined == nil {
			continue
		}
		if boolean, ok := defined.Value.(boolFlag); ok && boolean.IsBoolFlag() {
			continue
		}
		if index+1 < len(arguments) {
			index++
			flags = append(flags, arguments[index])
		}
	}
	return flags, positional
}

func (in *invocation) out() io.Writer { return in.env.Stdout }

func (in *invocation) printf(format string, arguments ...any) {
	fmt.Fprintf(in.env.Stdout, format, arguments...)
}

func (in *invocation) note(format string, arguments ...any) {
	fmt.Fprintf(in.env.Stderr, format+"\n", arguments...)
}

func (in *invocation) text(name string) string {
	return in.flags.Lookup(name).Value.String()
}

func (in *invocation) flagGiven(name string) bool {
	given := false
	in.flags.Visit(func(f *flag.Flag) { given = given || f.Name == name })
	return given
}

func (in *invocation) truth(name string) bool {
	return in.text(name) == "true"
}

func (in *invocation) table(header []string, rows [][]string) {
	writer := tabwriter.NewWriter(in.env.Stdout, 0, 4, 2, ' ', 0)
	fmt.Fprintln(writer, strings.Join(header, "\t"))
	for _, row := range rows {
		fmt.Fprintln(writer, strings.Join(row, "\t"))
	}
	_ = writer.Flush()
}

func (in *invocation) interactive() bool {
	return in.env.Session.StdinTerminal && !in.env.Session.CI()
}

func (in *invocation) confirm(ctx context.Context, statement string) error {
	if in.flags.Lookup("yes") != nil && in.truth("yes") {
		return nil
	}
	if in.app != nil {
		if value, err := in.app.Settings.Get(ctx, settings.KeyConfirm); err == nil && !value.Bool() {
			return nil
		}
	}
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

func root() *command {
	return link(&command{name: meta.Slug, sub: []*command{
		overviewCommand(),
		recordsCommand(),
		demoCommand(),
		notificationsCommand(),
		notifierCommand(),
		configCommand(),
		databaseCommand(),
		{name: "diagnostics", summary: "Show build, database and settings status", run: runDiagnostics},
		{name: "guide", summary: "Read or search the usage guide", args: "[search terms]", run: runGuide, noApp: true},
		{name: "about", summary: "Show version, source and bug report links", flags: func(set *flag.FlagSet) {
			set.Bool("issue", false, "print the bug report preview and its link")
		}, run: runAbout},
		{name: "licenses", summary: "Show the license and third-party notices", run: runLicenses, noApp: true},
		{name: "version", summary: "Print the version", run: func(_ context.Context, in *invocation) error {
			in.printf("%s %s\n", meta.Slug, meta.Version)
			return nil
		}, noApp: true},
	}})
}

func globalFlagSet(values *globals) *flag.FlagSet {
	set := flag.NewFlagSet(meta.Slug, flag.ContinueOnError)
	set.SetOutput(io.Discard)
	set.StringVar(&values.database, "db", "", "database file to use for this run")
	set.Var(&overrideFlag{values: values.overrides}, "set", "override a setting for this run, as key=value (repeatable)")
	set.StringVar(&values.ui, "ui", "", "open the dashboard (gui) or the terminal UI (tui)")
	set.BoolVar(&values.ask, "ask", false, "ask which front end to open even when a choice is remembered")
	set.BoolVar(&values.noPrompt, "no-prompt", false, "never ask, open the remembered or default front end")
	set.BoolVar(&values.help, "help", false, "show help")
	set.BoolVar(&values.help, "h", false, "show help")
	set.BoolVar(&values.version, "version", false, "print the version")
	return set
}

func describeFlags(set *flag.FlagSet, skip map[string]bool) []string {
	var lines []string
	set.VisitAll(func(defined *flag.Flag) {
		if skip[defined.Name] {
			return
		}
		name := "--" + defined.Name
		if boolean, ok := defined.Value.(boolFlag); !ok || !boolean.IsBoolFlag() {
			name += " <value>"
		}
		line := "  " + name + "\t" + defined.Usage
		if defined.DefValue != "" && defined.DefValue != "false" && defined.DefValue != "0" {
			line += " (default " + defined.DefValue + ")"
		}
		lines = append(lines, line)
	})
	return lines
}

func writeHelp(w io.Writer, target *command) {
	writer := tabwriter.NewWriter(w, 0, 4, 2, ' ', 0)
	if target.parent == nil {
		fmt.Fprintf(writer, "%s %s\n%s\n\n", meta.AppName, meta.Version, meta.Tagline)
		fmt.Fprintf(writer, "Usage:\n  %s\topen the dashboard or the terminal UI\n  %s <command> [arguments]\trun a command\n\nCommands:\n", meta.Slug, meta.Slug)
	} else {
		usage := meta.Slug + " " + target.path()
		if len(target.sub) > 0 {
			usage += " <command>"
		}
		if target.args != "" {
			usage += " " + target.args
		}
		if target.flags != nil {
			usage += " [flags]"
		}
		fmt.Fprintf(writer, "%s\n\nUsage:\n  %s\n", target.summary, usage)
		if len(target.sub) > 0 {
			fmt.Fprint(writer, "\nCommands:\n")
		}
	}
	for _, child := range target.sub {
		fmt.Fprintf(writer, "  %s\t%s\n", child.name, child.summary)
	}
	if lines := describeFlags(target.flagSet(), nil); target.parent != nil && len(lines) > 0 {
		fmt.Fprint(writer, "\nFlags:\n")
		for _, line := range lines {
			fmt.Fprintln(writer, line)
		}
	}
	fmt.Fprint(writer, "\nGlobal flags:\n")
	for _, line := range describeFlags(globalFlagSet(&globals{overrides: map[string]string{}}), map[string]bool{"h": true}) {
		fmt.Fprintln(writer, line)
	}
	if target.parent == nil {
		fmt.Fprintf(writer, "\nEnvironment:\n  %s\tgui or tui, same as --ui\n  %s\tdatabase file, same as --db\n  %s\tdirectory for the saved database path and front-end choice\n  NO_COLOR\tturn colors off in the terminal UI\n", meta.EnvName("ui"), meta.EnvName("db"), meta.EnvName("config_dir"))
		fmt.Fprintf(writer, "\nRun \"%s help <command>\" for details.\n", meta.Slug)
	}
	_ = writer.Flush()
}

func leaves(parent *command) []*command {
	var all []*command
	for _, child := range parent.sub {
		if len(child.sub) == 0 {
			all = append(all, child)
		}
		all = append(all, leaves(child)...)
	}
	return all
}

func CommandPaths() []string {
	var paths []string
	for _, leaf := range leaves(root()) {
		paths = append(paths, leaf.path())
	}
	sort.Strings(paths)
	return paths
}

func Reference() string {
	var builder strings.Builder
	builder.WriteString("| Command | Arguments and flags | What it does |\n|---|---|---|\n")
	for _, leaf := range leaves(root()) {
		var parts []string
		if leaf.args != "" {
			parts = append(parts, "`"+strings.ReplaceAll(leaf.args, "|", "\\|")+"`")
		}
		leaf.flagSet().VisitAll(func(defined *flag.Flag) {
			parts = append(parts, "`--"+defined.Name+"`")
		})
		if len(parts) == 0 {
			parts = []string{"none"}
		}
		builder.WriteString("| `" + meta.Slug + " " + leaf.path() + "` | " + strings.Join(parts, " ") + " | " + leaf.summary + " |\n")
	}
	return builder.String()
}

func (e *Env) appOptions(values globals, readOnly bool) app.Options {
	return app.Options{Environment: e.Environment, DatabasePath: values.database, Overrides: values.overrides, Now: e.Now, ReadOnly: readOnly}
}

func Run(ctx context.Context, env *Env) int {
	if env.Now == nil {
		env.Now = time.Now
	}
	values := globals{overrides: map[string]string{}}
	set := globalFlagSet(&values)
	if err := set.Parse(env.Args); err != nil {
		fmt.Fprintf(env.Stderr, "error: %v\nRun \"%s --help\" to see the flags.\n", err, meta.Slug)
		return ExitUsage
	}
	rest := set.Args()
	tree := root()
	if values.version {
		fmt.Fprintf(env.Stdout, "%s %s\n", meta.Slug, meta.Version)
		return ExitOK
	}
	if len(rest) > 0 && rest[0] == "help" {
		values.help, rest = true, rest[1:]
	}
	target := tree
	for len(rest) > 0 && len(target.sub) > 0 {
		if rest[0] == "--help" || rest[0] == "-h" {
			break
		}
		next := target.find(rest[0])
		if next == nil {
			fmt.Fprintf(env.Stderr, "error: unknown command %q\n%s\n", strings.TrimSpace(target.pathOrEmpty()+" "+rest[0]), suggestion(target, rest[0]))
			return ExitUsage
		}
		target, rest = next, rest[1:]
	}
	for _, argument := range rest {
		if argument == "--help" || argument == "-h" {
			values.help = true
		}
	}
	if values.help {
		writeHelp(env.Stdout, target)
		return ExitOK
	}
	if target == tree {
		return launch(ctx, env, values)
	}
	if len(target.sub) > 0 {
		writeHelp(env.Stderr, target)
		return ExitUsage
	}
	return report(env, execute(ctx, env, values, target, rest))
}

func (c *command) pathOrEmpty() string {
	if c.parent == nil {
		return ""
	}
	return c.path()
}

func suggestion(parent *command, word string) string {
	var names []string
	for _, child := range parent.sub {
		names = append(names, child.name)
		if strings.HasPrefix(child.name, word) || strings.HasPrefix(word, child.name) {
			return fmt.Sprintf("Did you mean %q?", strings.TrimSpace(parent.pathOrEmpty()+" "+child.name))
		}
	}
	return "Available: " + strings.Join(names, ", ")
}

func report(env *Env, err error) int {
	if err == nil {
		return ExitOK
	}
	fmt.Fprintf(env.Stderr, "error: %v\n", err)
	var usage *usageError
	if errors.As(err, &usage) {
		return ExitUsage
	}
	return ExitFailure
}

func execute(ctx context.Context, env *Env, values globals, target *command, arguments []string) error {
	set := target.flagSet()
	flags, positional := permute(set, arguments)
	if err := set.Parse(flags); err != nil {
		return usagef("%v: run \"%s help %s\" to see the flags", err, meta.Slug, target.path())
	}
	in := &invocation{env: env, globals: values, flags: set, args: append(positional, set.Args()...), command: target}
	if !target.noApp {
		opened, err := app.Open(ctx, env.appOptions(values, target.readOnly))
		if err != nil {
			return err
		}
		defer opened.Close()
		in.app = opened
		if opened.CreatedDatabase() {
			in.note("Created a new database at %s", opened.Location().Path)
		}
	}
	return target.run(ctx, in)
}

func launch(ctx context.Context, env *Env, values globals) int {
	location, err := app.Locate(env.appOptions(values, false))
	if err != nil {
		return report(env, err)
	}
	input := env.Session.Input(launcher.Input{
		UIFlag:       values.ui,
		Ask:          values.ask,
		NoPrompt:     values.noPrompt,
		EnvUI:        env.Environment.Getenv(meta.EnvName("ui")),
		Remembered:   location.Config.FrontEnd,
		GUIAvailable: env.OpenGUI != nil,
	})
	decision := launcher.Decide(input)
	mode, remember := "", false
	switch decision.Action {
	case launcher.ActionFail:
		return report(env, usagef("%s", decision.Reason))
	case launcher.ActionPrintHelp:
		fmt.Fprintf(env.Stderr, "%s: %s.\n\n", meta.AppName, decision.Reason)
		writeHelp(env.Stderr, root())
		return ExitOK
	case launcher.ActionOpenGUI:
		mode = launcher.ModeGUI
	case launcher.ActionOpenTUI:
		mode = launcher.ModeTUI
		if input.Headless && values.ui == "" {
			fmt.Fprintf(env.Stderr, "%s: %s.\n", meta.AppName, decision.Reason)
		}
	case launcher.ActionOpenTerminalWindow:
		return report(env, openTerminalWindow(env, values))
	case launcher.ActionPromptTerminal:
		result, err := launcher.PromptTerminal(env.Stdin, env.Stdout, decision.Default)
		if err != nil {
			return report(env, err)
		}
		if result.Quit {
			return ExitOK
		}
		mode, remember = result.Mode, result.Remember
	case launcher.ActionShowChooser:
		if env.ShowChooser == nil {
			return report(env, errors.New("this build cannot show the chooser window: pass --ui gui or --ui tui"))
		}
		result, err := env.ShowChooser(decision.Default)
		if err != nil {
			return report(env, err)
		}
		if result.Quit {
			return ExitOK
		}
		mode, remember = result.Mode, result.Remember
	}
	opened, err := app.Open(ctx, env.appOptions(values, false))
	if err != nil {
		return report(env, err)
	}
	defer opened.Close()
	if remember {
		if err := opened.RememberFrontEnd(mode); err != nil {
			fmt.Fprintf(env.Stderr, "warning: the choice could not be saved: %v\n", err)
		}
	}
	for mode != "" {
		var next string
		switch {
		case mode == launcher.ModeGUI && env.OpenGUI != nil:
			next, err = env.OpenGUI(ctx, opened)
		case mode == launcher.ModeTUI && !(input.StdinInteractive && input.TerminalOutput):
			return report(env, openTerminalWindow(env, values))
		case mode == launcher.ModeTUI && env.OpenTUI != nil:
			next, err = env.OpenTUI(ctx, opened)
		default:
			err = fmt.Errorf("this build cannot open the %s front end", mode)
		}
		if err != nil {
			return report(env, err)
		}
		mode = next
	}
	return ExitOK
}

func openTerminalWindow(env *Env, values globals) error {
	if env.OpenTerminalWindow == nil {
		return errors.New("no terminal window can be opened from here: start the application from a terminal with --ui tui")
	}
	arguments := []string{"--ui", "tui"}
	if values.database != "" {
		arguments = append(arguments, "--db", values.database)
	}
	keys := make([]string, 0, len(values.overrides))
	for key := range values.overrides {
		keys = append(keys, key)
	}
	sort.Strings(keys)
	for _, key := range keys {
		arguments = append(arguments, "--set", key+"="+values.overrides[key])
	}
	return env.OpenTerminalWindow(env.Executable, arguments)
}
