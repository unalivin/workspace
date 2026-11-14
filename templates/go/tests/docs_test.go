package tests

import (
	"flag"
	"os"
	"path/filepath"
	"regexp"
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

func TestTestsNamedInDocumentsExist(t *testing.T) {
	declared := map[string]bool{}
	declaration := regexp.MustCompile(`(?m)^func ((?:Test|Fuzz)[A-Za-z0-9_]+)\(`)
	for _, path := range projectFiles(t) {
		if strings.HasSuffix(path, "_test.go") {
			for _, match := range declaration.FindAllStringSubmatch(read(t, path), -1) {
				declared[match[1]] = true
			}
		}
	}
	if len(declared) < 50 {
		t.Fatalf("only %d tests were found", len(declared))
	}
	mention := regexp.MustCompile("`((?:Test|Fuzz)[A-Z][A-Za-z0-9_]+)`")
	mentioned := 0
	for _, path := range projectFiles(t) {
		if filepath.Ext(path) != ".md" {
			continue
		}
		for _, match := range mention.FindAllStringSubmatch(read(t, path), -1) {
			mentioned++
			if !declared[match[1]] {
				t.Errorf("%s names the test %s, which does not exist", path, match[1])
			}
		}
	}
	if mentioned == 0 {
		t.Fatal("no document names a test")
	}
}
