package platform

import (
	"context"
	"errors"
	"strings"
	"testing"

	"github.com/NexzaDev/templateapp/internal/meta"
)

type call struct {
	name      string
	arguments []string
}

func recorder(calls *[]call, failFirst int) Runner {
	return func(_ context.Context, name string, arguments ...string) error {
		*calls = append(*calls, call{name, arguments})
		if len(*calls) <= failFirst {
			return errors.New(name + " missing")
		}
		return nil
	}
}

func TestOpenRepositoryURLOnlyOpensRepositoryLinksAsOneArgument(t *testing.T) {
	url := meta.RepositoryURL + "/issues/new?title=a%20b&body=%22%3B+rm+-rf"
	for goos, program := range map[string]string{"linux": "xdg-open", "darwin": "open", "windows": "rundll32"} {
		var calls []call
		if err := OpenRepositoryURL(goos, url, recorder(&calls, 0)); err != nil {
			t.Fatal(err)
		}
		if len(calls) != 1 || calls[0].name != program || calls[0].arguments[len(calls[0].arguments)-1] != url {
			t.Fatalf("%s: %+v", goos, calls)
		}
	}
	for _, blocked := range []string{"https://example.org/x", "file:///etc/passwd", meta.RepositoryURL + "/../x", "", "--help"} {
		var calls []call
		if err := OpenRepositoryURL("linux", blocked, recorder(&calls, 0)); err == nil || len(calls) != 0 {
			t.Fatalf("%q was opened", blocked)
		}
	}
	var calls []call
	if err := OpenRepositoryURL("linux", meta.RepositoryURL, recorder(&calls, 1)); err == nil || !strings.Contains(err.Error(), "copy the link instead") {
		t.Fatalf("got %v", err)
	}
}

func TestOpenTerminalWindowTriesCandidatesInOrder(t *testing.T) {
	arguments := []string{"--ui", "tui", "--db", "/tmp/my data.db"}
	var calls []call
	if err := OpenTerminalWindow("linux", "/opt/app/bin", arguments, recorder(&calls, 2)); err != nil {
		t.Fatal(err)
	}
	if len(calls) != 3 || calls[0].name != "x-terminal-emulator" || calls[2].name != "konsole" {
		t.Fatalf("calls: %+v", calls)
	}
	joined := strings.Join(calls[2].arguments, "|")
	if joined != "-e|/opt/app/bin|--ui|tui|--db|/tmp/my data.db" {
		t.Fatalf("arguments: %s", joined)
	}
	calls = nil
	if err := OpenTerminalWindow("linux", "/opt/app/bin", nil, recorder(&calls, 100)); err == nil || !strings.Contains(err.Error(), "--ui tui") {
		t.Fatalf("got %v", err)
	}
	calls = nil
	if err := OpenTerminalWindow("windows", `C:\Apps\app.exe`, arguments, recorder(&calls, 0)); err != nil || calls[0].name != "cmd" || calls[0].arguments[3] != `C:\Apps\app.exe` {
		t.Fatalf("windows: %+v, %v", calls, err)
	}
	calls = nil
	if err := OpenTerminalWindow("darwin", `/Applications/My "App's".app/Contents/MacOS/app`, []string{"--ui", "tui"}, recorder(&calls, 0)); err != nil || len(calls) != 1 || calls[0].name != "osascript" {
		t.Fatalf("darwin: %+v, %v", calls, err)
	}
	want := []string{"-e", `tell application "Terminal" to do script "'/Applications/My \"App'\\''s\".app/Contents/MacOS/app' '--ui' 'tui'"`, "-e", `tell application "Terminal" to activate`}
	if strings.Join(calls[0].arguments, "|") != strings.Join(want, "|") {
		t.Fatalf("darwin arguments:\n%q\nwant\n%q", calls[0].arguments, want)
	}
	if quoted := shellQuote("a b'c"); quoted != `'a b'\''c'` {
		t.Fatalf("quoting: %s", quoted)
	}
}
