package main

import (
	"bytes"
	"flag"
	"fmt"
	"os"
	"path/filepath"
	"sort"

	"github.com/NexzaDev/templateapp/internal/brand"
	"github.com/NexzaDev/templateapp/internal/meta"
)

func run(directory string, check bool) error {
	files := brand.Files(meta.AppName, meta.Tagline, meta.Slug)
	names := make([]string, 0, len(files))
	for name := range files {
		names = append(names, name)
	}
	sort.Strings(names)
	for _, name := range names {
		path := filepath.Join(directory, filepath.FromSlash(name))
		if check {
			current, err := os.ReadFile(path)
			if err != nil {
				return fmt.Errorf("%s is missing: run make assets", path)
			}
			if !bytes.Equal(current, files[name]) {
				return fmt.Errorf("%s is out of date: run make assets", path)
			}
			continue
		}
		if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
			return err
		}
		if err := os.WriteFile(path, files[name], 0o644); err != nil {
			return err
		}
		fmt.Println("wrote", path)
	}
	return nil
}

func main() {
	directory := flag.String("out", "assets", "directory that receives the icon files")
	check := flag.Bool("check", false, "fail when a file differs instead of writing it")
	flag.Parse()
	if err := run(*directory, *check); err != nil {
		fmt.Fprintln(os.Stderr, "error:", err)
		os.Exit(1)
	}
}
