package launcher

import (
	"bufio"
	"fmt"
	"io"
	"strings"

	"github.com/unalivin/certwatch/internal/meta"
)

const (
	ModeGUI = "gui"
	ModeTUI = "tui"
)

type Action int

const (
	ActionRunCommand Action = iota
	ActionOpenGUI
	ActionOpenTUI
	ActionPromptTerminal
	ActionShowChooser
	ActionOpenTerminalWindow
	ActionPrintHelp
	ActionFail
)

func (a Action) String() string {
	return [...]string{"run command", "open dashboard", "open terminal UI", "prompt in terminal", "show chooser", "open terminal window", "print help", "fail"}[a]
}

type Input struct {
	UIFlag           string
	Ask              bool
	NoPrompt         bool
	HasSubcommand    bool
	EnvUI            string
	Remembered       string
	StdinInteractive bool
	TerminalOutput   bool
	CI               bool
	Headless         bool
	DesktopLaunch    bool
	GUIAvailable     bool
}

type Decision struct {
	Action  Action
	Default string
	Reason  string
}

func validMode(value string) bool {
	return value == ModeGUI || value == ModeTUI
}

func Decide(in Input) Decision {
	if in.HasSubcommand {
		return Decision{Action: ActionRunCommand, Reason: "an explicit subcommand never opens the chooser"}
	}
	flag := strings.ToLower(strings.TrimSpace(in.UIFlag))
	env := strings.ToLower(strings.TrimSpace(in.EnvUI))
	if flag != "" && !validMode(flag) {
		return Decision{Action: ActionFail, Reason: fmt.Sprintf("--ui %q is not valid: use --ui gui or --ui tui", in.UIFlag)}
	}
	if flag == "" && env != "" && !validMode(env) {
		return Decision{Action: ActionFail, Reason: fmt.Sprintf("%s=%q is not valid: use gui or tui", meta.EnvName("ui"), in.EnvUI)}
	}
	terminal := in.StdinInteractive && in.TerminalOutput
	explicit, origin := flag, "--ui"
	if explicit == "" {
		explicit, origin = env, meta.EnvName("ui")
	}
	switch explicit {
	case ModeGUI:
		if !in.GUIAvailable {
			return Decision{Action: ActionFail, Reason: origin + " asks for the dashboard but this build has no dashboard: use --ui tui or a build with the GUI"}
		}
		if in.Headless {
			return Decision{Action: ActionFail, Reason: origin + " asks for the dashboard but no display is available: use --ui tui"}
		}
		return Decision{Action: ActionOpenGUI, Reason: "requested with " + origin}
	case ModeTUI:
		if terminal {
			return Decision{Action: ActionOpenTUI, Reason: "requested with " + origin}
		}
		if in.DesktopLaunch && !in.Headless {
			return Decision{Action: ActionOpenTerminalWindow, Reason: "requested with " + origin + " from a desktop launch"}
		}
		return Decision{Action: ActionFail, Reason: origin + " asks for the terminal UI but this session is not an interactive terminal: run a subcommand instead"}
	}
	fallback := ModeTUI
	if in.GUIAvailable && !in.Headless {
		fallback = ModeGUI
	}
	preferred := fallback
	remembered := validMode(in.Remembered)
	if remembered && (in.Remembered == ModeTUI || fallback == ModeGUI) {
		preferred = in.Remembered
	}
	open := func(mode, reason string) Decision {
		if mode == ModeGUI {
			return Decision{Action: ActionOpenGUI, Default: mode, Reason: reason}
		}
		if terminal {
			return Decision{Action: ActionOpenTUI, Default: mode, Reason: reason}
		}
		if in.DesktopLaunch && !in.Headless {
			return Decision{Action: ActionOpenTerminalWindow, Default: mode, Reason: reason}
		}
		return Decision{Action: ActionPrintHelp, Default: mode, Reason: "this session is not interactive, so nothing was opened: run a subcommand or pass --ui"}
	}
	if in.Headless {
		return open(ModeTUI, "no display is available, opening the terminal UI")
	}
	if in.CI {
		if in.DesktopLaunch {
			return open(preferred, "automation environment, not prompting")
		}
		return Decision{Action: ActionPrintHelp, Default: preferred, Reason: "automation environment detected, so nothing was opened: run a subcommand or pass --ui"}
	}
	if in.DesktopLaunch {
		if !in.GUIAvailable {
			return open(ModeTUI, "this build has no dashboard")
		}
		if remembered && !in.Ask {
			return open(preferred, "remembered choice")
		}
		if in.NoPrompt {
			return open(preferred, "--no-prompt given")
		}
		return Decision{Action: ActionShowChooser, Default: preferred, Reason: "desktop launch"}
	}
	if !terminal {
		return Decision{Action: ActionPrintHelp, Default: preferred, Reason: "standard input is not interactive, so nothing was opened: run a subcommand or pass --ui"}
	}
	if in.NoPrompt {
		return open(preferred, "--no-prompt given")
	}
	if remembered && !in.Ask {
		return open(preferred, "remembered choice")
	}
	if !in.GUIAvailable {
		return open(ModeTUI, "this build has no dashboard")
	}
	return Decision{Action: ActionPromptTerminal, Default: preferred, Reason: "interactive terminal"}
}

type PromptResult struct {
	Mode     string
	Remember bool
	Quit     bool
}

func PromptTerminal(in io.Reader, out io.Writer, defaultMode string) (PromptResult, error) {
	if !validMode(defaultMode) {
		defaultMode = ModeGUI
	}
	marker := map[bool]string{true: " (default)", false: ""}
	fmt.Fprintf(out, "How do you want to open %s?\n", meta.AppName)
	fmt.Fprintf(out, "  1  Dashboard window%s\n", marker[defaultMode == ModeGUI])
	fmt.Fprintf(out, "  2  Terminal UI%s\n", marker[defaultMode == ModeTUI])
	fmt.Fprintln(out, "  r  Toggle \"remember my choice\"")
	fmt.Fprintln(out, "  q  Quit")
	reader := bufio.NewReader(io.LimitReader(in, 4096))
	remember := false
	for attempt := 0; attempt < 5; attempt++ {
		state := "off"
		if remember {
			state = "on"
		}
		fmt.Fprintf(out, "Choice [Enter = %s, remember: %s]: ", map[string]string{ModeGUI: "1", ModeTUI: "2"}[defaultMode], state)
		line, err := reader.ReadString('\n')
		answer := strings.ToLower(strings.TrimSpace(line))
		if err != nil && answer == "" {
			fmt.Fprintln(out)
			return PromptResult{Quit: true}, nil
		}
		switch answer {
		case "":
			return PromptResult{Mode: defaultMode, Remember: remember}, nil
		case "1", "d", "dashboard", ModeGUI:
			return PromptResult{Mode: ModeGUI, Remember: remember}, nil
		case "2", "t", "terminal", ModeTUI:
			return PromptResult{Mode: ModeTUI, Remember: remember}, nil
		case "r", "remember":
			remember = !remember
		case "q", "quit", "exit":
			return PromptResult{Quit: true}, nil
		default:
			fmt.Fprintln(out, "Enter 1, 2, r or q.")
		}
		if err != nil {
			return PromptResult{Quit: true}, nil
		}
	}
	return PromptResult{Quit: true}, fmt.Errorf("no valid choice after several attempts")
}
