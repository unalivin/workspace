package launcher

import "strings"

type Session struct {
	GOOS           string
	Getenv         func(string) string
	StdinTerminal  bool
	StdoutTerminal bool
	StderrTerminal bool
	ConsoleOwners  int
}

var automationVariables = []string{"CI", "GITHUB_ACTIONS", "GITLAB_CI", "JENKINS_URL", "BUILDKITE", "TF_BUILD", "TEAMCITY_VERSION"}

func (s Session) CI() bool {
	for _, name := range automationVariables {
		value := strings.ToLower(s.Getenv(name))
		if value != "" && value != "false" && value != "0" {
			return true
		}
	}
	return false
}

func (s Session) Headless() bool {
	switch s.GOOS {
	case "windows":
		return false
	case "darwin":
		return s.Getenv("SSH_CONNECTION") != "" || s.Getenv("SSH_TTY") != ""
	default:
		return s.Getenv("DISPLAY") == "" && s.Getenv("WAYLAND_DISPLAY") == ""
	}
}

func (s Session) DesktopLaunch() bool {
	if s.Headless() || s.CI() {
		return false
	}
	if s.GOOS == "windows" {
		return s.ConsoleOwners <= 1
	}
	if s.StdinTerminal || s.StdoutTerminal || s.StderrTerminal {
		return false
	}
	return s.Getenv("TERM") == "" || s.Getenv("TERM") == "dumb"
}

func (s Session) Input(base Input) Input {
	base.StdinInteractive = s.StdinTerminal
	base.TerminalOutput = s.StdoutTerminal
	base.CI = s.CI()
	base.Headless = s.Headless()
	base.DesktopLaunch = s.DesktopLaunch()
	return base
}

func RequestsTerminal(arguments []string) bool {
	for index, argument := range arguments {
		if argument == "--" {
			return false
		}
		if argument == "--ui="+ModeTUI || (argument == "--ui" && index+1 < len(arguments) && arguments[index+1] == ModeTUI) {
			return true
		}
	}
	return false
}
