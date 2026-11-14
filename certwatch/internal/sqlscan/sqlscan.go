package sqlscan

import (
	"fmt"
	"go/ast"
	"go/parser"
	"go/token"
	"io/fs"
	"os"
	"path/filepath"
	"regexp"
	"sort"
	"strconv"
	"strings"
)

type Finding struct {
	File   string
	Line   int
	Reason string
	Source string
}

func (f Finding) String() string {
	return fmt.Sprintf("%s:%d: %s: %s", f.File, f.Line, f.Reason, f.Source)
}

type Options struct {
	Allowed      map[string][]string
	IncludeTests bool
}

const statementTemplate = `^\s*(SELECT\s.+\sFROM\s|INSERT\s+(OR\s+\w+\s+)?INTO\s|UPDATE\s+[^\s]+\s+SET\s|DELETE\s+FROM\s|CREATE\s+(VIRTUAL\s+)?(TABLE|INDEX|UNIQUE|VIEW|TRIGGER)\s|DROP\s+(TABLE|INDEX|VIEW|TRIGGER)\s|ALTER\s+TABLE\s|PRAGMA\s+\w|WITH\s+\w+\s+AS\s|EXPLAIN\s)`

const fragmentTemplate = `(\sWHERE\s|\sORDER\s+BY\s|\sGROUP\s+BY\s|\sLIMIT\s|\sVALUES\s*\(|\sSET\s+\w+\s*=|\sFROM\s+\w)`

func eitherCase(template string) *regexp.Regexp {
	return regexp.MustCompile("(?s)(?:" + template + "|" + strings.ToLower(template) + ")")
}

var statementPattern = eitherCase(statementTemplate)

var fragmentPattern = eitherCase(fragmentTemplate)

var queryMethods = map[string]int{"Exec": 0, "Query": 0, "QueryRow": 0, "Prepare": 0, "ExecContext": 1, "QueryContext": 1, "QueryRowContext": 1, "PrepareContext": 1}

var builders = map[string]bool{"fmt.Sprintf": true, "fmt.Sprint": true, "fmt.Sprintln": true, "fmt.Fprintf": true, "strings.Join": true, "strings.Replace": true, "strings.ReplaceAll": true}

func LooksLikeSQL(text string) bool {
	return statementPattern.MatchString(text)
}

type scanFile struct {
	path    string
	rel     string
	file    *ast.File
	pkgKey  string
	imports map[string]string
}

type constant struct {
	value ast.Expr
	file  *scanFile
}

type scanner struct {
	fset     *token.FileSet
	root     string
	module   string
	files    []*scanFile
	consts   map[string]map[string]constant
	pure     map[string]map[string]bool
	options  Options
	used     map[string]bool
	findings []Finding
}

func moduleName(root string) (string, error) {
	raw, err := os.ReadFile(filepath.Join(root, "go.mod"))
	if err != nil {
		return "", err
	}
	for _, line := range strings.Split(string(raw), "\n") {
		if fields := strings.Fields(line); len(fields) == 2 && fields[0] == "module" {
			return fields[1], nil
		}
	}
	return "", fmt.Errorf("no module line in %s", filepath.Join(root, "go.mod"))
}

func Scan(root string, options Options) ([]Finding, error) {
	module, err := moduleName(root)
	if err != nil {
		return nil, err
	}
	s := &scanner{fset: token.NewFileSet(), root: root, module: module, consts: map[string]map[string]constant{}, pure: map[string]map[string]bool{}, options: options, used: map[string]bool{}}
	err = filepath.WalkDir(root, func(path string, entry fs.DirEntry, err error) error {
		if err != nil {
			return err
		}
		name := entry.Name()
		if entry.IsDir() {
			if path != root && (strings.HasPrefix(name, ".") || name == "testdata" || name == "vendor" || name == "dist" || name == "build" || name == "node_modules") {
				return filepath.SkipDir
			}
			return nil
		}
		if !strings.HasSuffix(name, ".go") || (!options.IncludeTests && strings.HasSuffix(name, "_test.go")) {
			return nil
		}
		return s.load(path)
	})
	if err != nil {
		return nil, err
	}
	for _, file := range s.files {
		s.inspect(file)
	}
	for file, expressions := range options.Allowed {
		for _, expression := range expressions {
			if !s.used[file+"\x00"+expression] {
				s.findings = append(s.findings, Finding{File: file, Reason: "the allowlist entry matches nothing and must be removed", Source: expression})
			}
		}
	}
	sort.Slice(s.findings, func(i, j int) bool {
		if s.findings[i].File != s.findings[j].File {
			return s.findings[i].File < s.findings[j].File
		}
		return s.findings[i].Line < s.findings[j].Line
	})
	return s.findings, nil
}

func ScanFile(path string) ([]Finding, error) {
	s := &scanner{fset: token.NewFileSet(), root: filepath.Dir(path), consts: map[string]map[string]constant{}, pure: map[string]map[string]bool{}, used: map[string]bool{}}
	if err := s.load(path); err != nil {
		return nil, err
	}
	s.inspect(s.files[0])
	return s.findings, nil
}

func (s *scanner) load(path string) error {
	source, err := os.ReadFile(path)
	if err != nil {
		return err
	}
	parsed, err := parser.ParseFile(s.fset, path, source, parser.SkipObjectResolution)
	if err != nil {
		return err
	}
	rel, err := filepath.Rel(s.root, path)
	if err != nil {
		return err
	}
	rel = filepath.ToSlash(rel)
	dir := filepath.ToSlash(filepath.Dir(rel))
	file := &scanFile{path: path, rel: rel, file: parsed, pkgKey: dir + ":" + strings.TrimSuffix(parsed.Name.Name, "_test"), imports: map[string]string{}}
	for _, spec := range parsed.Imports {
		importPath, _ := strconv.Unquote(spec.Path.Value)
		name := importPath[strings.LastIndex(importPath, "/")+1:]
		if spec.Name != nil {
			name = spec.Name.Name
		}
		file.imports[name] = importPath
	}
	if s.consts[file.pkgKey] == nil {
		s.consts[file.pkgKey] = map[string]constant{}
	}
	for _, declaration := range parsed.Decls {
		general, ok := declaration.(*ast.GenDecl)
		if !ok || general.Tok != token.CONST {
			continue
		}
		for _, spec := range general.Specs {
			value := spec.(*ast.ValueSpec)
			for index, name := range value.Names {
				if index < len(value.Values) {
					s.consts[file.pkgKey][name.Name] = constant{value: value.Values[index], file: file}
				}
			}
		}
	}
	s.files = append(s.files, file)
	return nil
}

func (s *scanner) packageKey(file *scanFile, importName string) (string, bool) {
	importPath, ok := file.imports[importName]
	if !ok || s.module == "" || !strings.HasPrefix(importPath, s.module+"/") {
		return "", false
	}
	dir := strings.TrimPrefix(importPath, s.module+"/")
	return dir + ":" + dir[strings.LastIndex(dir, "/")+1:], true
}

func (s *scanner) pureConst(pkgKey, name string, depth int) bool {
	if known, ok := s.pure[pkgKey][name]; ok {
		return known
	}
	declared, ok := s.consts[pkgKey][name]
	if !ok || depth > 20 {
		return false
	}
	result := s.pureExpr(declared.value, declared.file, depth+1)
	if s.pure[pkgKey] == nil {
		s.pure[pkgKey] = map[string]bool{}
	}
	s.pure[pkgKey][name] = result
	return result
}

func (s *scanner) pureExpr(expression ast.Expr, file *scanFile, depth int) bool {
	switch node := expression.(type) {
	case *ast.BasicLit:
		return node.Kind == token.STRING
	case *ast.ParenExpr:
		return s.pureExpr(node.X, file, depth)
	case *ast.BinaryExpr:
		return node.Op == token.ADD && s.pureExpr(node.X, file, depth) && s.pureExpr(node.Y, file, depth)
	case *ast.Ident:
		return s.pureConst(file.pkgKey, node.Name, depth)
	case *ast.SelectorExpr:
		if base, ok := node.X.(*ast.Ident); ok {
			if key, ok := s.packageKey(file, base.Name); ok {
				return s.pureConst(key, node.Sel.Name, depth)
			}
		}
	}
	return false
}

func literalText(expression ast.Expr) (string, bool) {
	literal, ok := expression.(*ast.BasicLit)
	if !ok || literal.Kind != token.STRING {
		return "", false
	}
	text, err := strconv.Unquote(literal.Value)
	return text, err == nil
}

func (s *scanner) text(file *scanFile, node ast.Node) string {
	source, err := os.ReadFile(file.path)
	if err != nil {
		return ""
	}
	start, end := s.fset.Position(node.Pos()).Offset, s.fset.Position(node.End()).Offset
	if start < 0 || end > len(source) || start > end {
		return ""
	}
	return strings.Join(strings.Fields(string(source[start:end])), " ")
}

func (s *scanner) report(file *scanFile, node ast.Node, reason string) {
	source := s.text(file, node)
	for _, allowed := range s.options.Allowed[file.rel] {
		if allowed == source {
			s.used[file.rel+"\x00"+allowed] = true
			return
		}
	}
	if len(source) > 160 {
		source = source[:160] + "..."
	}
	s.findings = append(s.findings, Finding{File: file.rel, Line: s.fset.Position(node.Pos()).Line, Reason: reason, Source: source})
}

func flatten(expression ast.Expr) string {
	switch node := expression.(type) {
	case *ast.ParenExpr:
		return flatten(node.X)
	case *ast.BinaryExpr:
		if node.Op == token.ADD {
			return flatten(node.X) + flatten(node.Y)
		}
	}
	if text, ok := literalText(expression); ok {
		return text
	}
	return "x"
}

func hasSQLLiteral(expression ast.Expr, fragments bool) bool {
	found := false
	ast.Inspect(expression, func(node ast.Node) bool {
		current, ok := node.(ast.Expr)
		if !ok {
			return !found
		}
		text, literal := literalText(current)
		if binary, joined := current.(*ast.BinaryExpr); joined && binary.Op == token.ADD {
			text, literal = flatten(binary), true
		}
		if literal && (LooksLikeSQL(text) || fragments && fragmentPattern.MatchString(" "+text)) {
			found = true
		}
		return !found
	})
	return found
}

func callName(call *ast.CallExpr) string {
	selector, ok := call.Fun.(*ast.SelectorExpr)
	if !ok {
		return ""
	}
	if base, ok := selector.X.(*ast.Ident); ok {
		return base.Name + "." + selector.Sel.Name
	}
	return "." + selector.Sel.Name
}

func (s *scanner) inspect(file *scanFile) {
	ast.Inspect(file.file, func(node ast.Node) bool {
		switch current := node.(type) {
		case *ast.CallExpr:
			name := callName(current)
			method := name[strings.LastIndex(name, ".")+1:]
			if index, ok := queryMethods[method]; ok && name != "" && len(current.Args) > index {
				argument := current.Args[index]
				if !s.pureExpr(argument, file, 0) {
					s.report(file, argument, "the query passed to "+method+" is not a constant")
					return false
				}
			}
			if builders[name] || method == "WriteString" {
				for _, argument := range current.Args {
					if hasSQLLiteral(argument, false) {
						label := name
						if !builders[name] {
							label = method
						}
						s.report(file, current, "SQL text is assembled with "+label)
						return false
					}
				}
			}
		case *ast.BinaryExpr:
			if current.Op == token.ADD && !s.pureExpr(current, file, 0) && hasSQLLiteral(current, false) {
				s.report(file, current, "SQL text is concatenated with a value that is not a constant")
				return false
			}
		case *ast.AssignStmt:
			if current.Tok == token.ADD_ASSIGN && len(current.Rhs) == 1 && hasSQLLiteral(current.Rhs[0], true) {
				s.report(file, current, "SQL text is appended to a variable")
				return false
			}
		}
		return true
	})
}
