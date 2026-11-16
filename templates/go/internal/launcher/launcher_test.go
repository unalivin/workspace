package launcher

import (
	"bytes"
	"strings"
	"testing"

	"github.com/unalivin/templateapp/internal/meta"
)

func TestDecide(t *testing.T) {
	terminal := Input{StdinInteractive: true, TerminalOutput: true, GUIAvailable: true}
	desktop := Input{DesktopLaunch: true, GUIAvailable: true}
	with := func(base Input, change func(*Input)) Input {
		change(&base)
		return base
	}
	cases := []struct {
		name        string
		in          Input
		action      Action
		defaultMode string
		reason      string
	}{
		{"subcommand always runs", with(terminal, func(i *Input) { i.HasSubcommand = true; i.UIFlag = "gui"; i.Ask = true }), ActionRunCommand, "", "subcommand"},
		{"subcommand runs in CI", Input{HasSubcommand: true, CI: true}, ActionRunCommand, "", ""},
		{"subcommand runs on desktop launch", with(desktop, func(i *Input) { i.HasSubcommand = true }), ActionRunCommand, "", ""},
		{"flag gui", with(terminal, func(i *Input) { i.UIFlag = "gui" }), ActionOpenGUI, "", "--ui"},
		{"flag gui upper case", with(terminal, func(i *Input) { i.UIFlag = " GUI " }), ActionOpenGUI, "", ""},
		{"flag tui", with(terminal, func(i *Input) { i.UIFlag = "tui" }), ActionOpenTUI, "", "--ui"},
		{"flag invalid", with(terminal, func(i *Input) { i.UIFlag = "web" }), ActionFail, "", "--ui gui or --ui tui"},
		{"flag wins over env", with(terminal, func(i *Input) { i.UIFlag = "tui"; i.EnvUI = "gui" }), ActionOpenTUI, "", "--ui"},
		{"flag wins over invalid env", with(terminal, func(i *Input) { i.UIFlag = "tui"; i.EnvUI = "bogus" }), ActionOpenTUI, "", ""},
		{"env gui", with(terminal, func(i *Input) { i.EnvUI = "gui" }), ActionOpenGUI, "", meta.EnvName("ui")},
		{"env tui", with(terminal, func(i *Input) { i.EnvUI = "tui" }), ActionOpenTUI, "", meta.EnvName("ui")},
		{"env invalid", with(terminal, func(i *Input) { i.EnvUI = "web" }), ActionFail, "", meta.EnvName("ui")},
		{"env wins over remembered", with(terminal, func(i *Input) { i.EnvUI = "tui"; i.Remembered = "gui" }), ActionOpenTUI, "", ""},
		{"flag gui without gui build", with(terminal, func(i *Input) { i.UIFlag = "gui"; i.GUIAvailable = false }), ActionFail, "", "no dashboard"},
		{"flag gui headless", with(terminal, func(i *Input) { i.UIFlag = "gui"; i.Headless = true }), ActionFail, "", "no display"},
		{"flag tui without terminal", Input{UIFlag: "tui", GUIAvailable: true}, ActionFail, "", "not an interactive terminal"},
		{"flag tui piped output", Input{UIFlag: "tui", StdinInteractive: true, GUIAvailable: true}, ActionFail, "", "not an interactive terminal"},
		{"flag tui from desktop opens a terminal window", with(desktop, func(i *Input) { i.UIFlag = "tui" }), ActionOpenTerminalWindow, "", "desktop"},
		{"terminal prompts by default", terminal, ActionPromptTerminal, ModeGUI, "interactive"},
		{"terminal prompt default follows remembered when asked", with(terminal, func(i *Input) { i.Remembered = "tui"; i.Ask = true }), ActionPromptTerminal, ModeTUI, ""},
		{"terminal remembered gui", with(terminal, func(i *Input) { i.Remembered = "gui" }), ActionOpenGUI, ModeGUI, "remembered"},
		{"terminal remembered tui", with(terminal, func(i *Input) { i.Remembered = "tui" }), ActionOpenTUI, ModeTUI, "remembered"},
		{"terminal remembered invalid is ignored", with(terminal, func(i *Input) { i.Remembered = "web" }), ActionPromptTerminal, ModeGUI, ""},
		{"terminal no prompt opens default", with(terminal, func(i *Input) { i.NoPrompt = true }), ActionOpenGUI, ModeGUI, "--no-prompt"},
		{"terminal no prompt opens remembered", with(terminal, func(i *Input) { i.NoPrompt = true; i.Remembered = "tui" }), ActionOpenTUI, ModeTUI, "--no-prompt"},
		{"terminal no prompt beats ask", with(terminal, func(i *Input) { i.NoPrompt = true; i.Ask = true; i.Remembered = "tui" }), ActionOpenTUI, ModeTUI, ""},
		{"terminal without gui build opens tui", with(terminal, func(i *Input) { i.GUIAvailable = false }), ActionOpenTUI, ModeTUI, "no dashboard"},
		{"terminal without gui build ignores remembered gui", with(terminal, func(i *Input) { i.GUIAvailable = false; i.Remembered = "gui" }), ActionOpenTUI, ModeTUI, ""},
		{"headless terminal opens tui with reason", with(terminal, func(i *Input) { i.Headless = true }), ActionOpenTUI, ModeTUI, "no display is available"},
		{"headless ignores remembered gui", with(terminal, func(i *Input) { i.Headless = true; i.Remembered = "gui" }), ActionOpenTUI, ModeTUI, "no display"},
		{"headless ignores ask", with(terminal, func(i *Input) { i.Headless = true; i.Ask = true }), ActionOpenTUI, ModeTUI, ""},
		{"headless without terminal prints help", Input{Headless: true, GUIAvailable: true}, ActionPrintHelp, ModeTUI, "not interactive"},
		{"ci never prompts", with(terminal, func(i *Input) { i.CI = true }), ActionPrintHelp, ModeGUI, "automation"},
		{"ci never prompts even with ask", with(terminal, func(i *Input) { i.CI = true; i.Ask = true }), ActionPrintHelp, ModeGUI, "automation"},
		{"ci honours explicit flag", with(terminal, func(i *Input) { i.CI = true; i.UIFlag = "tui" }), ActionOpenTUI, "", ""},
		{"non-interactive stdin never prompts", Input{TerminalOutput: true, GUIAvailable: true}, ActionPrintHelp, ModeGUI, "not interactive"},
		{"non-interactive stdin with ask never prompts", Input{TerminalOutput: true, GUIAvailable: true, Ask: true}, ActionPrintHelp, ModeGUI, ""},
		{"piped output never prompts", Input{StdinInteractive: true, GUIAvailable: true}, ActionPrintHelp, ModeGUI, ""},
		{"desktop launch shows chooser", desktop, ActionShowChooser, ModeGUI, "desktop"},
		{"desktop launch remembered gui", with(desktop, func(i *Input) { i.Remembered = "gui" }), ActionOpenGUI, ModeGUI, "remembered"},
		{"desktop launch remembered tui opens terminal window", with(desktop, func(i *Input) { i.Remembered = "tui" }), ActionOpenTerminalWindow, ModeTUI, "remembered"},
		{"desktop launch ask shows chooser with remembered default", with(desktop, func(i *Input) { i.Remembered = "tui"; i.Ask = true }), ActionShowChooser, ModeTUI, ""},
		{"desktop launch no prompt opens gui", with(desktop, func(i *Input) { i.NoPrompt = true }), ActionOpenGUI, ModeGUI, "--no-prompt"},
		{"desktop launch without gui build opens terminal window", with(desktop, func(i *Input) { i.GUIAvailable = false }), ActionOpenTerminalWindow, ModeTUI, "no dashboard"},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			got := Decide(tc.in)
			if got.Action != tc.action {
				t.Fatalf("action = %v, want %v (reason: %s)", got.Action, tc.action, got.Reason)
			}
			if tc.defaultMode != "" && got.Default != tc.defaultMode {
				t.Fatalf("default = %q, want %q", got.Default, tc.defaultMode)
			}
			if got.Reason == "" || !strings.Contains(got.Reason, tc.reason) {
				t.Fatalf("reason %q does not contain %q", got.Reason, tc.reason)
			}
		})
	}
}

func TestDecideNeverPromptsWithoutAnInteractiveSession(t *testing.T) {
	modes := []string{"", "gui", "tui", "bogus"}
	flags := []bool{false, true}
	for _, uiFlag := range modes {
		for _, env := range modes {
			for _, remembered := range modes {
				for _, ask := range flags {
					for _, noPrompt := range flags {
						for _, stdin := range flags {
							for _, stdout := range flags {
								for _, ci := range flags {
									for _, headless := range flags {
										for _, desktop := range flags {
											for _, gui := range flags {
												in := Input{UIFlag: uiFlag, EnvUI: env, Remembered: remembered, Ask: ask, NoPrompt: noPrompt, StdinInteractive: stdin, TerminalOutput: stdout, CI: ci, Headless: headless, DesktopLaunch: desktop, GUIAvailable: gui}
												got := Decide(in)
												if got.Reason == "" {
													t.Fatalf("no reason for %+v", in)
												}
												prompts := got.Action == ActionPromptTerminal || got.Action == ActionShowChooser
												if prompts && (ci || noPrompt || headless) {
													t.Fatalf("prompted in CI, headless or with --no-prompt: %+v", in)
												}
												if got.Action == ActionPromptTerminal && (!stdin || !stdout) {
													t.Fatalf("terminal prompt without an interactive terminal: %+v", in)
												}
												if got.Action == ActionOpenTUI && (!stdin || !stdout) {
													t.Fatalf("terminal UI without an interactive terminal: %+v", in)
												}
												if got.Action == ActionOpenGUI && (!gui || headless) {
													t.Fatalf("dashboard without a GUI build or display: %+v", in)
												}
												if got.Action == ActionShowChooser && (!gui || !desktop) {
													t.Fatalf("chooser outside a desktop launch with GUI: %+v", in)
												}
												if got.Action == ActionRunCommand {
													t.Fatalf("command action without a subcommand: %+v", in)
												}
											}
										}
									}
								}
							}
						}
					}
				}
			}
		}
	}
}

func TestPromptTerminal(t *testing.T) {
	cases := []struct {
		name        string
		input       string
		defaultMode string
		want        PromptResult
		wantErr     bool
	}{
		{"enter uses default gui", "\n", ModeGUI, PromptResult{Mode: ModeGUI}, false},
		{"enter uses default tui", "\n", ModeTUI, PromptResult{Mode: ModeTUI}, false},
		{"one opens dashboard", "1\n", ModeTUI, PromptResult{Mode: ModeGUI}, false},
		{"two opens terminal", "2\n", ModeGUI, PromptResult{Mode: ModeTUI}, false},
		{"words work", "Terminal\n", ModeGUI, PromptResult{Mode: ModeTUI}, false},
		{"remember then choose", "r\n2\n", ModeGUI, PromptResult{Mode: ModeTUI, Remember: true}, false},
		{"remember toggles twice", "r\nr\n1\n", ModeGUI, PromptResult{Mode: ModeGUI}, false},
		{"quit", "q\n", ModeGUI, PromptResult{Quit: true}, false},
		{"end of input quits", "", ModeGUI, PromptResult{Quit: true}, false},
		{"choice without newline", "2", ModeGUI, PromptResult{Mode: ModeTUI}, false},
		{"invalid then valid", "x\n9\n1\n", ModeTUI, PromptResult{Mode: ModeGUI}, false},
		{"too many invalid answers", "x\ny\nz\nw\nv\nu\n", ModeGUI, PromptResult{Quit: true}, true},
		{"invalid default falls back to dashboard", "\n", "web", PromptResult{Mode: ModeGUI}, false},
		{"oversized line", strings.Repeat("x", 10000) + "\n1\n", ModeGUI, PromptResult{Quit: true}, false},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			var out bytes.Buffer
			got, err := PromptTerminal(strings.NewReader(tc.input), &out, tc.defaultMode)
			if (err != nil) != tc.wantErr || got != tc.want {
				t.Fatalf("got %+v, %v; want %+v, error %v", got, err, tc.want, tc.wantErr)
			}
			text := out.String()
			for _, expected := range []string{"How do you want to open " + meta.AppName + "?", "1  Dashboard window", "2  Terminal UI"} {
				if !strings.Contains(text, expected) {
					t.Fatalf("prompt lacks %q:\n%s", expected, text)
				}
			}
		})
	}
}

func TestSessionDetection(t *testing.T) {
	env := func(values map[string]string) func(string) string {
		return func(key string) string { return values[key] }
	}
	cases := []struct {
		name     string
		session  Session
		ci       bool
		headless bool
		desktop  bool
	}{
		{"linux terminal with display", Session{GOOS: "linux", Getenv: env(map[string]string{"DISPLAY": ":0", "TERM": "xterm"}), StdinTerminal: true, StdoutTerminal: true, StderrTerminal: true}, false, false, false},
		{"linux wayland desktop launch", Session{GOOS: "linux", Getenv: env(map[string]string{"WAYLAND_DISPLAY": "wayland-0"})}, false, false, true},
		{"linux ssh without display", Session{GOOS: "linux", Getenv: env(map[string]string{"TERM": "xterm"}), StdinTerminal: true, StdoutTerminal: true, StderrTerminal: true}, false, true, false},
		{"linux cron job", Session{GOOS: "linux", Getenv: env(nil)}, false, true, false},
		{"linux pipe with display", Session{GOOS: "linux", Getenv: env(map[string]string{"DISPLAY": ":0", "TERM": "xterm"}), StderrTerminal: true}, false, false, false},
		{"linux redirected in a terminal session", Session{GOOS: "linux", Getenv: env(map[string]string{"DISPLAY": ":0", "TERM": "xterm-256color"})}, false, false, false},
		{"github actions", Session{GOOS: "linux", Getenv: env(map[string]string{"CI": "true", "GITHUB_ACTIONS": "true", "DISPLAY": ":99"})}, true, false, false},
		{"ci false is not ci", Session{GOOS: "linux", Getenv: env(map[string]string{"CI": "false", "DISPLAY": ":0"})}, false, false, true},
		{"macos finder launch", Session{GOOS: "darwin", Getenv: env(nil)}, false, false, true},
		{"macos ssh", Session{GOOS: "darwin", Getenv: env(map[string]string{"SSH_CONNECTION": "1", "TERM": "xterm"}), StdinTerminal: true, StdoutTerminal: true, StderrTerminal: true}, false, true, false},
		{"windows explorer launch without console", Session{GOOS: "windows", Getenv: env(nil)}, false, false, true},
		{"windows explorer launch with its own console", Session{GOOS: "windows", Getenv: env(nil), StdinTerminal: true, StdoutTerminal: true, StderrTerminal: true, ConsoleOwners: 1}, false, false, true},
		{"windows shared console", Session{GOOS: "windows", Getenv: env(nil), StdinTerminal: true, StdoutTerminal: true, StderrTerminal: true, ConsoleOwners: 2}, false, false, false},
		{"windows redirected in a shared console", Session{GOOS: "windows", Getenv: env(nil), ConsoleOwners: 3}, false, false, false},
		{"windows automation", Session{GOOS: "windows", Getenv: env(map[string]string{"CI": "true"}), ConsoleOwners: 1}, true, false, false},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			if got := tc.session.CI(); got != tc.ci {
				t.Fatalf("CI = %v, want %v", got, tc.ci)
			}
			if got := tc.session.Headless(); got != tc.headless {
				t.Fatalf("Headless = %v, want %v", got, tc.headless)
			}
			if got := tc.session.DesktopLaunch(); got != tc.desktop {
				t.Fatalf("DesktopLaunch = %v, want %v", got, tc.desktop)
			}
			in := tc.session.Input(Input{GUIAvailable: true, Remembered: "tui"})
			if in.CI != tc.ci || in.Headless != tc.headless || in.DesktopLaunch != tc.desktop || !in.GUIAvailable || in.Remembered != "tui" {
				t.Fatalf("Input did not carry detection results: %+v", in)
			}
		})
	}
}

func TestRequestsTerminal(t *testing.T) {
	cases := []struct {
		arguments []string
		want      bool
	}{
		{nil, false},
		{[]string{"--ui", "tui"}, true},
		{[]string{"--db", "x.db", "--ui=tui"}, true},
		{[]string{"--ui", "gui"}, false},
		{[]string{"--ui"}, false},
		{[]string{"records", "list"}, false},
		{[]string{"--", "--ui", "tui"}, false},
	}
	for _, item := range cases {
		if got := RequestsTerminal(item.arguments); got != item.want {
			t.Errorf("%v: got %v", item.arguments, got)
		}
	}
}
