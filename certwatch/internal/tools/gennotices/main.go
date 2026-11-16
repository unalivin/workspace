package main

import (
	"bytes"
	"flag"
	"fmt"
	"os"
	"os/exec"
	"sort"
	"strings"

	"github.com/unalivin/certwatch/internal/licenses"
	"github.com/unalivin/certwatch/internal/meta"
)

const bundled = `## Components bundled inside dependencies

Some dependencies carry further components that end up in the binary:

- Fonts shipped by the dashboard toolkit Fyne: Inter and Noto Sans (SIL Open Font License 1.1), DejaVu Sans Mono (Bitstream Vera terms, with DejaVu changes in the public domain) and EmojiOne Color (MIT). Their license files are reproduced under ` + "`fyne.io/fyne/v2`" + ` below.
- Widget glyphs shipped by Fyne (check boxes, drop-down arrows, scroll bars). The Fyne module ships them without a separate license file, so they are listed here under the license of the Fyne module itself; the module does not state where they came from.
- The GLFW C library (zlib license), compiled in through ` + "`github.com/go-gl/glfw`" + `. Its license file is reproduced below.
- The SQLite library (public domain), compiled in through ` + "`github.com/mattn/go-sqlite3`" + `.

The application's own icons (navigation, status, severity and action icons, and the application icon) are original drawings released under the application's license.

`

var nested = map[string][]string{
	"fyne.io/fyne/v2":                 {"theme/font"},
	"github.com/go-gl/glfw/v3.4/glfw": {"glfw"},
}

func modules(tags string) ([]licenses.Module, error) {
	found := map[string]licenses.Module{}
	for _, target := range []string{"linux", "darwin", "windows"} {
		command := exec.Command("go", "list", "-deps", "-tags", tags, "-f", "{{with .Module}}{{if not .Main}}{{.Path}}\t{{.Version}}\t{{.Dir}}{{end}}{{end}}", "./cmd/"+meta.Slug)
		command.Env = append(os.Environ(), "GOOS="+target, "CGO_ENABLED=1")
		var failure bytes.Buffer
		command.Stderr = &failure
		output, err := command.Output()
		if err != nil {
			return nil, fmt.Errorf("go list for %s failed: %v: %s", target, err, strings.TrimSpace(failure.String()))
		}
		for _, line := range strings.Split(strings.TrimSpace(string(output)), "\n") {
			fields := strings.Split(line, "\t")
			if len(fields) != 3 || fields[0] == "" {
				continue
			}
			if fields[2] == "" {
				return nil, fmt.Errorf("module %s is not downloaded: run go mod download", fields[0])
			}
			found[fields[0]] = licenses.Module{Path: fields[0], Version: fields[1], Dir: fields[2]}
		}
	}
	list := make([]licenses.Module, 0, len(found))
	for _, module := range found {
		list = append(list, module)
	}
	sort.Slice(list, func(i, j int) bool { return list[i].Path < list[j].Path })
	return list, nil
}

func run(output, tags string, check bool) error {
	list, err := modules(tags)
	if err != nil {
		return err
	}
	var notices []licenses.Notice
	var problems []string
	for _, module := range list {
		notice, err := licenses.Collect(module, nested[module.Path]...)
		if err != nil {
			return err
		}
		problems = append(problems, notice.Problems()...)
		notices = append(notices, notice)
	}
	if len(problems) > 0 {
		return fmt.Errorf("license review failed:\n  %s", strings.Join(problems, "\n  "))
	}
	rendered := licenses.Render(meta.AppName, meta.License, notices, bundled)
	if check {
		current, err := os.ReadFile(output)
		if err != nil {
			return fmt.Errorf("%s is missing: run make notices", output)
		}
		if strings.ReplaceAll(string(current), "\r\n", "\n") != rendered {
			return fmt.Errorf("%s is out of date: run make notices", output)
		}
		fmt.Printf("%s is current: %d modules, every license is on the allowed list\n", output, len(notices))
		return nil
	}
	if err := os.WriteFile(output, []byte(rendered), 0o644); err != nil {
		return err
	}
	fmt.Printf("wrote %s: %d modules\n", output, len(notices))
	return nil
}

func main() {
	output := flag.String("out", "THIRD_PARTY_NOTICES.md", "file to write")
	tags := flag.String("tags", "sqlite_fts5 no_animations", "build tags of the release build")
	check := flag.Bool("check", false, "fail when the file differs instead of writing it")
	flag.Parse()
	if err := run(*output, *tags, *check); err != nil {
		fmt.Fprintln(os.Stderr, "error:", err)
		os.Exit(1)
	}
}
