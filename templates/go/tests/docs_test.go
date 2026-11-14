package tests

import (
	"flag"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/NexzaDev/templateapp/internal/app"
	"github.com/NexzaDev/templateapp/internal/cli"
)

var updateDocs = flag.Bool("update-docs", false, "rewrite the generated tables in docs/COMMANDS.md")

func replaceBlock(t *testing.T, document, name, content string) string {
	t.Helper()
	start, end := "<!-- "+name+":start -->\n", "<!-- "+name+":end -->"
	from := strings.Index(document, start)
	to := strings.Index(document, end)
	if from < 0 || to < from {
		t.Fatalf("docs/COMMANDS.md lacks the %s markers", name)
	}
	return document[:from+len(start)] + content + document[to:]
}

func TestGeneratedCommandTablesAreCurrent(t *testing.T) {
	path := filepath.Join("..", "docs", "COMMANDS.md")
	raw, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	current := strings.ReplaceAll(string(raw), "\r\n", "\n")
	wanted := replaceBlock(t, current, "capabilities", app.CapabilityTable())
	wanted = replaceBlock(t, wanted, "reference", cli.Reference())
	if wanted == current {
		return
	}
	if !*updateDocs {
		t.Fatal("docs/COMMANDS.md is out of date: run make docs")
	}
	if err := os.WriteFile(path, []byte(wanted), 0o644); err != nil {
		t.Fatal(err)
	}
}
