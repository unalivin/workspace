package main

import (
	"context"
	"os"
	"os/signal"
	"runtime"
	"syscall"
	"time"

	"github.com/charmbracelet/lipgloss"
	"github.com/mattn/go-isatty"

	"github.com/NexzaDev/certwatch/internal/app"
	"github.com/NexzaDev/certwatch/internal/cli"
	"github.com/NexzaDev/certwatch/internal/launcher"
	"github.com/NexzaDev/certwatch/internal/notify"
	"github.com/NexzaDev/certwatch/internal/platform"
	"github.com/NexzaDev/certwatch/internal/tui"
	"github.com/NexzaDev/certwatch/internal/userconfig"
)

func isTerminal(file *os.File) bool {
	return isatty.IsTerminal(file.Fd()) || isatty.IsCygwinTerminal(file.Fd())
}

func autostart(environment userconfig.Environment, executable string) func() notify.Autostart {
	return func() notify.Autostart {
		home, _ := environment.HomeDir()
		return notify.Autostart{
			GOOS:       environment.GOOS,
			HomeDir:    home,
			ConfigHome: environment.Getenv("XDG_CONFIG_HOME"),
			AppData:    environment.Getenv("APPDATA"),
			Executable: executable,
		}
	}
}

func openURL(url string) error {
	return platform.OpenRepositoryURL(runtime.GOOS, url, nil)
}

func newEnv(detached bool) *cli.Env {
	environment := userconfig.SystemEnvironment()
	executable, err := os.Executable()
	if err != nil {
		executable = os.Args[0]
	}
	session := launcher.Session{
		GOOS:           runtime.GOOS,
		Getenv:         os.Getenv,
		StdinTerminal:  isTerminal(os.Stdin),
		StdoutTerminal: isTerminal(os.Stdout),
		StderrTerminal: isTerminal(os.Stderr),
		ConsoleOwners:  consoleOwners(),
	}
	if detached {
		session.StdinTerminal = false
		session.ConsoleOwners = 1
	}
	env := &cli.Env{
		Args:        os.Args[1:],
		Stdin:       os.Stdin,
		Stdout:      os.Stdout,
		Stderr:      os.Stderr,
		Environment: environment,
		Now:         time.Now,
		Session:     session,
		Executable:  executable,
		Deliverer:   notify.SystemDeliverer(runtime.GOOS),
		Autostart:   autostart(environment, executable),
		OpenTerminalWindow: func(program string, arguments []string) error {
			return platform.OpenTerminalWindow(runtime.GOOS, program, arguments, nil)
		},
	}
	env.OpenTUI = func(ctx context.Context, application *app.App) (string, error) {
		setConsoleVisible(true)
		return tui.Run(ctx, tui.Options{
			App:          application,
			Getenv:       os.Getenv,
			Dark:         lipgloss.HasDarkBackground,
			Autostart:    env.Autostart,
			HasDashboard: env.OpenGUI != nil,
			OpenURL:      openURL,
			Input:        os.Stdin,
			Output:       os.Stdout,
		})
	}
	return env
}

func main() {
	detached := prepareConsole(os.Args[1:])
	ctx, cancel := signal.NotifyContext(context.Background(), os.Interrupt, syscall.SIGTERM)
	code := run(ctx, newEnv(detached))
	cancel()
	os.Exit(code)
}
