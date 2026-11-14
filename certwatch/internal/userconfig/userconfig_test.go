package userconfig

import (
	"errors"
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"testing"

	"github.com/NexzaDev/certwatch/internal/meta"
)

func rooted(parts ...string) string {
	directory, err := os.Getwd()
	if err != nil {
		panic(err)
	}
	return filepath.Join(append([]string{filepath.VolumeName(directory) + string(filepath.Separator)}, parts...)...)
}

func fakeEnv(goos string, values map[string]string) Environment {
	return Environment{
		Getenv:  func(key string) string { return values[key] },
		HomeDir: func() (string, error) { return rooted("home", "tester"), nil },
		GOOS:    goos,
	}
}

func TestDirPerOperatingSystem(t *testing.T) {
	home := rooted("home", "tester")
	absolute := rooted("custom")
	cases := []struct {
		name   string
		env    Environment
		config string
		data   string
	}{
		{"linux default", fakeEnv("linux", nil), filepath.Join(home, ".config", meta.Slug), filepath.Join(home, ".local", "share", meta.Slug, meta.Slug+".db")},
		{"linux xdg", fakeEnv("linux", map[string]string{"XDG_CONFIG_HOME": absolute, "XDG_DATA_HOME": absolute}), filepath.Join(absolute, meta.Slug), filepath.Join(absolute, meta.Slug, meta.Slug+".db")},
		{"linux relative xdg ignored", fakeEnv("linux", map[string]string{"XDG_CONFIG_HOME": "relative"}), filepath.Join(home, ".config", meta.Slug), filepath.Join(home, ".local", "share", meta.Slug, meta.Slug+".db")},
		{"macos", fakeEnv("darwin", nil), filepath.Join(home, "Library", "Application Support", meta.Slug), filepath.Join(home, "Library", "Application Support", meta.Slug, meta.Slug+".db")},
		{"windows variables", fakeEnv("windows", map[string]string{"AppData": absolute, "LocalAppData": absolute}), filepath.Join(absolute, meta.Slug), filepath.Join(absolute, meta.Slug, meta.Slug+".db")},
		{"windows fallback", fakeEnv("windows", nil), filepath.Join(home, "AppData", "Roaming", meta.Slug), filepath.Join(home, "AppData", "Local", meta.Slug, meta.Slug+".db")},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			dir, err := Dir(tc.env)
			if err != nil || dir != tc.config {
				t.Fatalf("Dir = %q, %v; want %q", dir, err, tc.config)
			}
			data, err := DefaultDatabasePath(tc.env)
			if err != nil || data != tc.data {
				t.Fatalf("DefaultDatabasePath = %q, %v; want %q", data, err, tc.data)
			}
		})
	}
}

func TestDirOverrideAndMissingHome(t *testing.T) {
	override := filepath.Join(string(filepath.Separator), "portable", "cfg")
	dir, err := Dir(fakeEnv("linux", map[string]string{meta.EnvName("config_dir"): override}))
	if err != nil || dir != override {
		t.Fatalf("override ignored: %q %v", dir, err)
	}
	env := Environment{Getenv: func(string) string { return "" }, HomeDir: func() (string, error) { return "", errors.New("no home") }, GOOS: "linux"}
	if _, err := Dir(env); err == nil || !strings.Contains(err.Error(), meta.EnvName("config_dir")) {
		t.Fatalf("expected an error naming the override variable, got %v", err)
	}
}

func TestSaveLoadRoundTripWithRestrictivePermissions(t *testing.T) {
	dir := filepath.Join(t.TempDir(), "nested", "cfg")
	want := Config{DatabasePath: filepath.Join(t.TempDir(), "data.db"), FrontEnd: FrontEndTUI}
	if err := Save(dir, want); err != nil {
		t.Fatal(err)
	}
	got, err := Load(dir)
	if err != nil || got != want {
		t.Fatalf("Load = %+v, %v; want %+v", got, err, want)
	}
	if runtime.GOOS != "windows" {
		for path, mode := range map[string]os.FileMode{dir: 0o700, filepath.Join(dir, fileName): 0o600} {
			info, err := os.Stat(path)
			if err != nil {
				t.Fatal(err)
			}
			if info.Mode().Perm() != mode {
				t.Fatalf("%s has mode %v, want %v", path, info.Mode().Perm(), mode)
			}
		}
	}
	entries, err := os.ReadDir(dir)
	if err != nil || len(entries) != 1 {
		t.Fatalf("expected only the config file, found %d entries (%v)", len(entries), err)
	}
}

func TestLoadMissingFileReturnsDefaults(t *testing.T) {
	got, err := Load(t.TempDir())
	if err != nil || got != (Config{}) {
		t.Fatalf("Load = %+v, %v", got, err)
	}
}

func TestLoadRejectsBrokenFiles(t *testing.T) {
	cases := map[string]string{
		"malformed json":    "{not json",
		"unknown front end": `{"front_end":"web"}`,
		"relative database": `{"database_path":"relative.db"}`,
		"oversized":         strings.Repeat(" ", maxFileSize+1),
	}
	for name, content := range cases {
		t.Run(name, func(t *testing.T) {
			dir := t.TempDir()
			if err := os.WriteFile(filepath.Join(dir, fileName), []byte(content), 0o600); err != nil {
				t.Fatal(err)
			}
			if got, err := Load(dir); err == nil || got != (Config{}) {
				t.Fatalf("expected an error and an empty config, got %+v, %v", got, err)
			}
		})
	}
}

func TestSaveRejectsInvalidConfigAndUnwritableDirectory(t *testing.T) {
	if err := Save(t.TempDir(), Config{FrontEnd: "web"}); err == nil {
		t.Fatal("expected a validation error")
	}
	blocker := filepath.Join(t.TempDir(), "file")
	if err := os.WriteFile(blocker, []byte("x"), 0o600); err != nil {
		t.Fatal(err)
	}
	if err := Save(filepath.Join(blocker, "cfg"), Config{}); err == nil {
		t.Fatal("expected an error when the directory cannot be created")
	}
}
