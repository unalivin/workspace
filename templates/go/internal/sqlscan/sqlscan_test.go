package sqlscan

import (
	"go/ast"
	"go/parser"
	"go/token"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func fixture(name string) string {
	return filepath.Join("..", "..", "testdata", "sqlscan", name)
}

func TestDirtyFixtureIsReportedFunctionByFunction(t *testing.T) {
	findings, err := ScanFile(fixture("dirty.go"))
	if err != nil {
		t.Fatal(err)
	}
	fset := token.NewFileSet()
	parsed, err := parser.ParseFile(fset, fixture("dirty.go"), nil, 0)
	if err != nil {
		t.Fatal(err)
	}
	expected := map[string]string{
		"concatenatedArgument": "not a constant",
		"formattedArgument":    "not a constant",
		"variableArgument":     "not a constant",
		"concatenatedVariable": "concatenated",
		"formattedVariable":    "assembled with fmt.Sprintf",
		"appendedClause":       "appended",
		"builderText":          "assembled with WriteString",
		"joinedText":           "assembled with strings.Join",
		"legacyCall":           "not a constant",
		"preparedFromVariable": "not a constant",
	}
	functions := 0
	for _, declaration := range parsed.Decls {
		function, ok := declaration.(*ast.FuncDecl)
		if !ok {
			continue
		}
		functions++
		first, last := fset.Position(function.Pos()).Line, fset.Position(function.End()).Line
		var inside []Finding
		for _, finding := range findings {
			if finding.Line >= first && finding.Line <= last {
				inside = append(inside, finding)
			}
		}
		want, known := expected[function.Name.Name]
		if !known {
			t.Errorf("fixture function %s has no expectation", function.Name.Name)
			continue
		}
		if len(inside) != 1 || !strings.Contains(inside[0].Reason, want) {
			t.Errorf("%s: got %v, want one finding with %q", function.Name.Name, inside, want)
		}
	}
	if functions != len(expected) || len(findings) != len(expected) {
		t.Fatalf("%d functions, %d findings, %d expectations", functions, len(findings), len(expected))
	}
	if text := findings[0].String(); !strings.Contains(text, "dirty.go:") {
		t.Fatalf("finding text lacks the position: %q", text)
	}
}

func TestCleanFixtureHasNoFindings(t *testing.T) {
	findings, err := ScanFile(fixture("clean.go"))
	if err != nil {
		t.Fatal(err)
	}
	for _, finding := range findings {
		t.Errorf("unexpected finding: %s", finding)
	}
}

func TestLooksLikeSQL(t *testing.T) {
	for text, want := range map[string]bool{
		"SELECT id FROM records":                  true,
		"  select id from records where x = 1":    true,
		"INSERT INTO t (a) VALUES (?)":            true,
		"INSERT OR REPLACE INTO t (a) VALUES (?)": true,
		"UPDATE t SET a = ?":                      true,
		"DELETE FROM t":                           true,
		"CREATE VIRTUAL TABLE f USING fts5 (a)":   true,
		"PRAGMA user_version":                     true,
		"WITH x AS (SELECT 1) SELECT * FROM x":    true,
		"Select a record from the list":           false,
		"Update the name and set a status":        false,
		"selected from the menu":                  false,
		"":                                        false,
		"record deleted from disk":                false,
	} {
		if got := LooksLikeSQL(text); got != want {
			t.Errorf("LooksLikeSQL(%q) = %v, want %v", text, got, want)
		}
	}
}

func writeModule(t *testing.T, files map[string]string) string {
	t.Helper()
	root := t.TempDir()
	files["go.mod"] = "module example.test/app\n\ngo 1.27\n"
	for name, content := range files {
		path := filepath.Join(root, filepath.FromSlash(name))
		if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
			t.Fatal(err)
		}
		if err := os.WriteFile(path, []byte(content), 0o644); err != nil {
			t.Fatal(err)
		}
	}
	return root
}

func TestScanResolvesConstantsAcrossPackagesAndHonoursTheAllowlist(t *testing.T) {
	root := writeModule(t, map[string]string{
		"internal/queries/queries.go": "package queries\n\nconst Columns = \"id\"\n\nconst Select = \"SELECT \" + Columns + \" FROM t WHERE id = ?\"\n\nvar Mutable = \"SELECT 1\"\n",
		"internal/app/app.go":         "package app\n\nimport (\n\t\"database/sql\"\n\n\t\"example.test/app/internal/queries\"\n)\n\nfunc Good(db *sql.DB) { db.Query(queries.Select, 1) }\n\nfunc Variable(db *sql.DB) { db.Query(queries.Mutable) }\n\nfunc Passed(db *sql.DB, query string) { db.Query(query) }\n",
		"internal/app/app_test.go":    "package app\n\nimport \"database/sql\"\n\nfunc helper(db *sql.DB, name string) { db.Exec(\"DELETE FROM t WHERE n = '\" + name + \"'\") }\n",
		"testdata/ignored.go":         "package ignored\n\nimport \"database/sql\"\n\nfunc Bad(db *sql.DB, q string) { db.Exec(q) }\n",
		"vendor/v/v.go":               "package v\n\nimport \"database/sql\"\n\nfunc Bad(db *sql.DB, q string) { db.Exec(q) }\n",
	})
	findings, err := Scan(root, Options{})
	if err != nil {
		t.Fatal(err)
	}
	if len(findings) != 2 || findings[0].File != "internal/app/app.go" || !strings.Contains(findings[0].Source, "queries.Mutable") || findings[1].Source != "query" {
		t.Fatalf("unexpected findings: %v", findings)
	}
	allowed, err := Scan(root, Options{Allowed: map[string][]string{"internal/app/app.go": {"query", "queries.Mutable"}}})
	if err != nil || len(allowed) != 0 {
		t.Fatalf("the allowlist was not honoured: %v, %v", allowed, err)
	}
	stale, err := Scan(root, Options{Allowed: map[string][]string{"internal/app/app.go": {"query", "queries.Mutable", "gone"}}})
	if err != nil || len(stale) != 1 || !strings.Contains(stale[0].Reason, "matches nothing") {
		t.Fatalf("a stale allowlist entry must be reported: %v, %v", stale, err)
	}
	withTests, err := Scan(root, Options{IncludeTests: true})
	if err != nil || len(withTests) != 3 {
		t.Fatalf("test files were not scanned on request: %v, %v", withTests, err)
	}
}

func TestScanReportsUnreadableInput(t *testing.T) {
	if _, err := Scan(t.TempDir(), Options{}); err == nil {
		t.Fatal("a directory without go.mod was accepted")
	}
	root := writeModule(t, map[string]string{"broken.go": "package broken\n\nfunc {"})
	if _, err := Scan(root, Options{}); err == nil {
		t.Fatal("a file that does not parse was accepted")
	}
	if _, err := ScanFile(filepath.Join(root, "missing.go")); err == nil {
		t.Fatal("a missing file was accepted")
	}
}
