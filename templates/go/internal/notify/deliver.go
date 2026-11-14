package notify

import (
	"context"
	"fmt"
	"os/exec"
)

type CommandRunner func(ctx context.Context, name string, args []string, env []string) error

type CommandDeliverer struct {
	GOOS     string
	LookPath func(string) (string, error)
	Run      CommandRunner
}

const windowsScript = "Add-Type -AssemblyName System.Windows.Forms; " +
	"$n = New-Object System.Windows.Forms.NotifyIcon; " +
	"$n.Icon = [System.Drawing.SystemIcons]::Information; $n.Visible = $true; " +
	"$n.ShowBalloonTip(8000, $env:APP_NOTIFY_TITLE, $env:APP_NOTIFY_BODY, [System.Windows.Forms.ToolTipIcon]::Info); " +
	"Start-Sleep -Seconds 9; $n.Dispose()"

func SystemDeliverer(goos string) CommandDeliverer {
	return CommandDeliverer{GOOS: goos, LookPath: exec.LookPath, Run: func(ctx context.Context, name string, args []string, env []string) error {
		command := exec.CommandContext(ctx, name, args...)
		command.Env = append(command.Environ(), env...)
		return command.Run()
	}}
}

func (d CommandDeliverer) Command(title, body string) (string, []string, []string) {
	switch d.GOOS {
	case "windows":
		return "powershell", []string{"-NoProfile", "-NonInteractive", "-WindowStyle", "Hidden", "-Command", windowsScript}, []string{"APP_NOTIFY_TITLE=" + title, "APP_NOTIFY_BODY=" + body}
	case "darwin":
		return "osascript", []string{"-e", "on run argv", "-e", "display notification (item 2 of argv) with title (item 1 of argv)", "-e", "end run", "--", title, body}, nil
	default:
		return "notify-send", []string{"--app-name", title, "--", title, body}, nil
	}
}

func (d CommandDeliverer) Deliver(ctx context.Context, title, body string) error {
	name, args, env := d.Command(SafeTitle(title), SafeTitle(body))
	path, err := d.LookPath(name)
	if err != nil {
		return fmt.Errorf("%s was not found, so system notifications cannot be shown on this computer: %w", name, err)
	}
	return d.Run(ctx, path, args, env)
}
