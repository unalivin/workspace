package tests

import (
	"go/ast"
	"go/parser"
	"go/token"
	"io/fs"
	"os"
	"path/filepath"
	"regexp"
	"strings"
	"testing"

	"github.com/unalivin/certwatch/internal/settings"
)

var requiredFiles = []string{
	"README.md", "CHANGELOG.md", "CONTRIBUTING.md", "CODE_OF_CONDUCT.md", "SECURITY.md", "LICENSE", "Makefile",
	".gitignore", ".gitattributes", ".env.example", "THIRD_PARTY_NOTICES.md",
	"docs/README.md", "docs/CONTRIBUTING.md", "docs/INSTALLATION.md", "docs/USAGE.md", "docs/COMMANDS.md",
	"docs/CONFIGURATION.md", "docs/TROUBLESHOOTING.md", "docs/SECURITY.md", "docs/THREAT_MODEL.md",
	"docs/PLATFORM_SUPPORT.md", "docs/TESTING.md", "docs/ARCHITECTURE.md", "docs/ROADMAP.md", "docs/RELEASING.md",
	"docs/RELEASE_NOTES.md", "docs/LICENSES.md", "docs/adr/README.md",
}

var requiredDirectories = []string{"migrations", "scripts", "assets", "testdata", "docs", "docs/adr"}

var skippedDirectories = map[string]bool{".git": true, "dist": true, "node_modules": true}

func projectFiles(t *testing.T) []string {
	t.Helper()
	var files []string
	err := filepath.WalkDir("..", func(path string, entry fs.DirEntry, err error) error {
		if err != nil {
			return err
		}
		if entry.IsDir() {
			if skippedDirectories[entry.Name()] {
				return filepath.SkipDir
			}
			return nil
		}
		files = append(files, path)
		return nil
	})
	if err != nil {
		t.Fatal(err)
	}
	return files
}

func read(t *testing.T, path string) string {
	t.Helper()
	raw, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	return strings.ReplaceAll(string(raw), "\r\n", "\n")
}

func TestRequiredFilesAndReadmeSectionsExist(t *testing.T) {
	for _, name := range requiredFiles {
		info, err := os.Stat(filepath.Join("..", filepath.FromSlash(name)))
		if err != nil || info.IsDir() || info.Size() == 0 {
			t.Errorf("required file %s is missing or empty", name)
		}
	}
	for _, name := range requiredDirectories {
		info, err := os.Stat(filepath.Join("..", filepath.FromSlash(name)))
		if err != nil || !info.IsDir() {
			t.Errorf("required directory %s is missing", name)
		}
	}
	readme := read(t, filepath.Join("..", "README.md"))
	for _, section := range []string{"Download", "Install", "Usage", "Documentation", "Security", "Contributing", "License"} {
		if !strings.Contains(readme, "\n## "+section+"\n") {
			t.Errorf("README.md has no section %q", section)
		}
	}
	index := read(t, filepath.Join("..", "docs", "README.md"))
	for _, name := range requiredFiles {
		if page, found := strings.CutPrefix(name, "docs/"); found && page != "README.md" && !strings.Contains(index, "("+page+")") {
			t.Errorf("docs/README.md does not link %s", page)
		}
	}
}

func TestRelativeLinksInDocumentsResolve(t *testing.T) {
	link := regexp.MustCompile(`\]\(([^)#\s]+)(#[^)]*)?\)`)
	for _, path := range projectFiles(t) {
		if filepath.Ext(path) != ".md" || filepath.Base(path) == "THIRD_PARTY_NOTICES.md" {
			continue
		}
		for _, match := range link.FindAllStringSubmatch(read(t, path), -1) {
			target := match[1]
			if strings.Contains(target, "://") {
				continue
			}
			if _, err := os.Stat(filepath.Join(filepath.Dir(path), filepath.FromSlash(target))); err != nil {
				t.Errorf("%s links to %s, which does not exist", path, target)
			}
		}
	}
}

func TestNoEmailAddressAnywhere(t *testing.T) {
	address := regexp.MustCompile(`[A-Za-z0-9._%+-]+@[A-Za-z0-9-]+(\.[A-Za-z0-9-]+)*\.[A-Za-z]{2,}`)
	for _, path := range projectFiles(t) {
		raw, err := os.ReadFile(path)
		if err != nil {
			t.Fatal(err)
		}
		if strings.ContainsRune(string(raw[:min(len(raw), 8000)]), 0) {
			continue
		}
		for number, line := range strings.Split(string(raw), "\n") {
			if address.MatchString(line) {
				t.Errorf("%s:%d contains an email address", path, number+1)
			}
		}
	}
}

func TestSourceFilesHaveNoComments(t *testing.T) {
	for _, path := range projectFiles(t) {
		switch filepath.Ext(path) {
		case ".go":
			if strings.Contains(filepath.ToSlash(path), "/testdata/") {
				continue
			}
			file, err := parser.ParseFile(token.NewFileSet(), path, nil, parser.ParseComments)
			if err != nil {
				t.Errorf("%s: %v", path, err)
				continue
			}
			for _, group := range file.Comments {
				for _, comment := range group.List {
					if !strings.HasPrefix(comment.Text, "//go:") {
						t.Errorf("%s contains the comment %q", path, comment.Text)
					}
				}
			}
		case ".sql":
			for number, line := range strings.Split(read(t, path), "\n") {
				if strings.HasPrefix(strings.TrimSpace(line), "--") || strings.Contains(line, "/*") {
					t.Errorf("%s:%d contains a comment", path, number+1)
				}
			}
		case ".sh", ".bash", ".bats":
			for number, line := range strings.Split(read(t, path), "\n") {
				if strings.HasPrefix(strings.TrimSpace(line), "#") && !strings.HasPrefix(line, "#!") {
					t.Errorf("%s:%d contains a comment", path, number+1)
				}
			}
		}
	}
}

func TestConfigurationDocumentListsEverySetting(t *testing.T) {
	document := read(t, filepath.Join("..", "docs", "CONFIGURATION.md"))
	documented := map[string]string{}
	for _, line := range strings.Split(document, "\n") {
		cells := strings.Split(line, "|")
		if len(cells) < 4 || !strings.HasPrefix(strings.TrimSpace(cells[1]), "`") {
			continue
		}
		key := strings.Trim(strings.TrimSpace(cells[1]), "`")
		if _, known := settings.Lookup(key); known {
			documented[key] = strings.TrimSpace(cells[2])
		} else if strings.Count(key, ".") >= 1 && !strings.ContainsAny(key, " -<") {
			t.Errorf("docs/CONFIGURATION.md documents %q, which is not a setting", key)
		}
	}
	for _, definition := range settings.Definitions() {
		value, found := documented[definition.Key]
		if !found {
			t.Errorf("docs/CONFIGURATION.md does not document %s", definition.Key)
			continue
		}
		wanted := "`" + definition.Default + "`"
		if definition.Default == "" {
			wanted = "empty"
		}
		if value != wanted {
			t.Errorf("docs/CONFIGURATION.md gives the default of %s as %s, the application uses %s", definition.Key, value, wanted)
		}
		for _, option := range definition.Options {
			if !strings.Contains(document, "`"+option+"`") {
				t.Errorf("docs/CONFIGURATION.md does not mention the value %q of %s", option, definition.Key)
			}
		}
	}
}

func TestNetworkAccessStaysInTheCheckPackage(t *testing.T) {
	disabling := "Insecure" + "SkipVerify"
	forbiddenImports := map[string]bool{"net/http": true, "net/rpc": true, "net/smtp": true, "net/mail": true, "crypto/tls": true}
	checked := 0
	for _, path := range projectFiles(t) {
		slashed := filepath.ToSlash(path)
		if filepath.Ext(path) != ".go" || strings.Contains(slashed, "/testdata/") {
			continue
		}
		file, err := parser.ParseFile(token.NewFileSet(), path, nil, 0)
		if err != nil {
			t.Errorf("%s: %v", path, err)
			continue
		}
		checked++
		allowed := strings.HasSuffix(path, "_test.go") || strings.Contains(slashed, "/internal/tlscheck/") || strings.Contains(slashed, "/internal/testkit/")
		for _, spec := range file.Imports {
			if name := strings.Trim(spec.Path.Value, `"`); !allowed && forbiddenImports[name] {
				t.Errorf("%s imports %s; only internal/tlscheck may open connections", path, name)
			}
		}
		ast.Inspect(file, func(node ast.Node) bool {
			switch value := node.(type) {
			case *ast.Ident:
				if value.Name == disabling {
					t.Errorf("%s turns certificate verification off", path)
				}
			case *ast.SelectorExpr:
				owner, isName := value.X.(*ast.Ident)
				if !isName || owner.Name != "net" || allowed {
					return true
				}
				name := value.Sel.Name
				if strings.HasPrefix(name, "Dial") || strings.HasPrefix(name, "Listen") || strings.HasPrefix(name, "Lookup") || name == "Resolver" || name == "DefaultResolver" {
					t.Errorf("%s uses net.%s; only internal/tlscheck may open connections", path, name)
				}
			}
			return true
		})
	}
	if checked < 50 {
		t.Fatalf("only %d source files were checked", checked)
	}
}
