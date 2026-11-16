package platform

import (
	"context"
	"errors"
	"fmt"
	"os/exec"
	"strings"
	"time"

	"github.com/unalivin/templateapp/internal/meta"
)

type Runner func(ctx context.Context, name string, arguments ...string) error

func systemRunner(ctx context.Context, name string, arguments ...string) error {
	path, err := exec.LookPath(name)
	if err != nil {
		return fmt.Errorf("%s was not found: %w", name, err)
	}
	return exec.CommandContext(ctx, path, arguments...).Start()
}

func BrowserCommand(goos, url string) (string, []string) {
	switch goos {
	case "windows":
		return "rundll32", []string{"url.dll,FileProtocolHandler", url}
	case "darwin":
		return "open", []string{url}
	default:
		return "xdg-open", []string{url}
	}
}

func OpenRepositoryURL(goos, url string, run Runner) error {
	if !meta.IsRepositoryURL(url) {
		return errors.New("only links under " + meta.RepositoryURL + " can be opened from the application")
	}
	if run == nil {
		run = systemRunner
	}
	ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
	defer cancel()
	name, arguments := BrowserCommand(goos, url)
	if err := run(ctx, name, arguments...); err != nil {
		return fmt.Errorf("the browser could not be started, copy the link instead: %w", err)
	}
	return nil
}

func shellQuote(word string) string {
	return "'" + strings.ReplaceAll(word, "'", `'\''`) + "'"
}

func TerminalCommands(goos, executable string, arguments []string) [][]string {
	switch goos {
	case "windows":
		return [][]string{append([]string{"cmd", "/c", "start", "", executable}, arguments...)}
	case "darwin":
		words := []string{shellQuote(executable)}
		for _, argument := range arguments {
			words = append(words, shellQuote(argument))
		}
		line := strings.NewReplacer(`\`, `\\`, `"`, `\"`).Replace(strings.Join(words, " "))
		return [][]string{{"osascript", "-e", `tell application "Terminal" to do script "` + line + `"`, "-e", `tell application "Terminal" to activate`}}
	default:
		var commands [][]string
		for _, terminal := range [][]string{{"x-terminal-emulator", "-e"}, {"gnome-terminal", "--"}, {"konsole", "-e"}, {"xfce4-terminal", "-x"}, {"alacritty", "-e"}, {"kitty"}, {"foot"}, {"xterm", "-e"}} {
			commands = append(commands, append(append(append([]string{}, terminal...), executable), arguments...))
		}
		return commands
	}
}

func OpenTerminalWindow(goos, executable string, arguments []string, run Runner) error {
	if run == nil {
		run = systemRunner
	}
	ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
	defer cancel()
	var failures []error
	for _, command := range TerminalCommands(goos, executable, arguments) {
		err := run(ctx, command[0], command[1:]...)
		if err == nil {
			return nil
		}
		failures = append(failures, err)
	}
	return fmt.Errorf("no terminal window could be opened, start the application from a terminal with --ui tui: %w", errors.Join(failures...))
}
