package notify

import (
	"encoding/xml"
	"errors"
	"fmt"
	"io/fs"
	"os"
	"path/filepath"
	"strings"
	"unicode"

	"github.com/unalivin/templateapp/internal/meta"
)

type Autostart struct {
	GOOS       string
	HomeDir    string
	ConfigHome string
	AppData    string
	Executable string
}

func (a Autostart) Path() (string, error) {
	if a.HomeDir == "" && a.GOOS != "windows" {
		return "", errors.New("the home directory is unknown, so the autostart entry cannot be placed")
	}
	switch a.GOOS {
	case "windows":
		base := a.AppData
		if base == "" {
			if a.HomeDir == "" {
				return "", errors.New("neither AppData nor the home directory is known, so the autostart entry cannot be placed")
			}
			base = filepath.Join(a.HomeDir, "AppData", "Roaming")
		}
		return filepath.Join(base, "Microsoft", "Windows", "Start Menu", "Programs", "Startup", meta.Slug+"-notifier.cmd"), nil
	case "darwin":
		return filepath.Join(a.HomeDir, "Library", "LaunchAgents", a.label()+".plist"), nil
	default:
		base := a.ConfigHome
		if !filepath.IsAbs(base) {
			base = filepath.Join(a.HomeDir, ".config")
		}
		return filepath.Join(base, "autostart", meta.Slug+"-notifier.desktop"), nil
	}
}

func (a Autostart) label() string {
	return "io.github.unalivin." + meta.Slug + ".notifier"
}

func (a Autostart) checkExecutable() error {
	if a.Executable == "" || !filepath.IsAbs(a.Executable) && !(a.GOOS == "windows" && len(a.Executable) > 2 && a.Executable[1] == ':') {
		return fmt.Errorf("the executable path %q must be absolute", a.Executable)
	}
	for _, r := range a.Executable {
		if unicode.IsControl(r) || strings.ContainsRune("\"`$%", r) {
			return fmt.Errorf("the executable path %q contains a character that cannot be used in an autostart entry: move the application to a simpler path", a.Executable)
		}
	}
	return nil
}

func (a Autostart) Content() (string, error) {
	if err := a.checkExecutable(); err != nil {
		return "", err
	}
	switch a.GOOS {
	case "windows":
		return "@echo off\r\nstart \"\" /min \"" + a.Executable + "\" notifier run\r\n", nil
	case "darwin":
		var escaped strings.Builder
		if err := xml.EscapeText(&escaped, []byte(a.Executable)); err != nil {
			return "", err
		}
		return `<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>` + a.label() + `</string>
  <key>ProgramArguments</key>
  <array>
    <string>` + escaped.String() + `</string>
    <string>notifier</string>
    <string>run</string>
  </array>
  <key>RunAtLoad</key>
  <true/>
  <key>ProcessType</key>
  <string>Background</string>
</dict>
</plist>
`, nil
	default:
		quoted := strings.ReplaceAll(a.Executable, `\`, `\\`)
		return "[Desktop Entry]\nType=Application\nName=" + meta.AppName + " notifier\nComment=Shows " + meta.AppName + " notifications\nExec=\"" + quoted + "\" notifier run\nTerminal=false\nNoDisplay=true\nX-GNOME-Autostart-enabled=true\n", nil
	}
}

func (a Autostart) Install(dryRun bool) (string, error) {
	path, err := a.Path()
	if err != nil {
		return "", err
	}
	content, err := a.Content()
	if err != nil {
		return path, err
	}
	if dryRun {
		return path, nil
	}
	if err := os.MkdirAll(filepath.Dir(path), 0o700); err != nil {
		return path, fmt.Errorf("cannot create %s: %w", filepath.Dir(path), err)
	}
	if err := os.WriteFile(path, []byte(content), 0o600); err != nil {
		return path, fmt.Errorf("cannot write %s: %w", path, err)
	}
	return path, nil
}

func (a Autostart) Uninstall(dryRun bool) (bool, string, error) {
	path, err := a.Path()
	if err != nil {
		return false, "", err
	}
	if _, err := os.Stat(path); errors.Is(err, fs.ErrNotExist) {
		return false, path, nil
	} else if err != nil {
		return false, path, err
	}
	if dryRun {
		return true, path, nil
	}
	if err := os.Remove(path); err != nil {
		return false, path, fmt.Errorf("cannot remove %s: %w", path, err)
	}
	return true, path, nil
}

func (a Autostart) Status() (bool, string, error) {
	path, err := a.Path()
	if err != nil {
		return false, "", err
	}
	info, err := os.Stat(path)
	if errors.Is(err, fs.ErrNotExist) {
		return false, path, nil
	}
	if err != nil {
		return false, path, err
	}
	return info.Mode().IsRegular(), path, nil
}
