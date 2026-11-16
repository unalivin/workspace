//go:build !nogui

package main

import (
	"context"
	"os"
	"runtime"

	"fyne.io/fyne/v2"
	fyneapp "fyne.io/fyne/v2/app"

	"github.com/unalivin/templateapp/internal/app"
	"github.com/unalivin/templateapp/internal/cli"
	"github.com/unalivin/templateapp/internal/gui"
	"github.com/unalivin/templateapp/internal/launcher"
	"github.com/unalivin/templateapp/internal/meta"
	"github.com/unalivin/templateapp/internal/platform"
)

func run(ctx context.Context, env *cli.Env) int {
	host := gui.NewHost(func() fyne.App { return fyneapp.NewWithID(meta.AppID) })
	inTerminal := env.Session.StdinTerminal && env.Session.StdoutTerminal
	hideOwnConsole := func() {
		if env.Session.DesktopLaunch() && env.Session.StdinTerminal {
			setConsoleVisible(false)
		}
	}
	env.ShowChooser = func(defaultMode string) (launcher.PromptResult, error) {
		hideOwnConsole()
		return host.Chooser(defaultMode)
	}
	env.OpenGUI = func(ctx context.Context, application *app.App) (string, error) {
		hideOwnConsole()
		return host.Dashboard(ctx, gui.Options{
			App:        application,
			Getenv:     os.Getenv,
			Autostart:  env.Autostart,
			OpenURL:    openURL,
			InTerminal: inTerminal,
			OpenTerminal: func() error {
				arguments := []string{"--ui", "tui", "--db", application.Location().Path}
				return platform.OpenTerminalWindow(runtime.GOOS, env.Executable, arguments, nil)
			},
		})
	}
	code := make(chan int, 1)
	go func() {
		code <- cli.Run(ctx, env)
		host.Stop()
	}()
	host.Loop()
	return <-code
}
