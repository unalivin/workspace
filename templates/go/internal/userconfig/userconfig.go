package userconfig

import (
	"encoding/json"
	"errors"
	"fmt"
	"io/fs"
	"os"
	"path/filepath"
	"runtime"

	"github.com/unalivin/templateapp/internal/meta"
)

const (
	FrontEndGUI = "gui"
	FrontEndTUI = "tui"
	fileName    = "config.json"
	maxFileSize = 64 * 1024
)

type Config struct {
	DatabasePath string `json:"database_path"`
	FrontEnd     string `json:"front_end"`
}

type Environment struct {
	Getenv  func(string) string
	HomeDir func() (string, error)
	GOOS    string
}

func SystemEnvironment() Environment {
	return Environment{Getenv: os.Getenv, HomeDir: os.UserHomeDir, GOOS: runtime.GOOS}
}

func Dir(env Environment) (string, error) {
	if override := env.Getenv(meta.EnvName("config_dir")); override != "" {
		return filepath.Clean(override), nil
	}
	base, err := baseDir(env, "config")
	if err != nil {
		return "", err
	}
	return filepath.Join(base, meta.Slug), nil
}

func DefaultDatabasePath(env Environment) (string, error) {
	base, err := baseDir(env, "data")
	if err != nil {
		return "", err
	}
	return filepath.Join(base, meta.Slug, meta.Slug+".db"), nil
}

func baseDir(env Environment, kind string) (string, error) {
	home, err := env.HomeDir()
	if err != nil || home == "" {
		return "", fmt.Errorf("cannot find the home directory: set %s to a writable directory", meta.EnvName("config_dir"))
	}
	switch env.GOOS {
	case "windows":
		variable := "AppData"
		if kind == "data" {
			variable = "LocalAppData"
		}
		if value := env.Getenv(variable); value != "" {
			return value, nil
		}
		return filepath.Join(home, "AppData", map[string]string{"config": "Roaming", "data": "Local"}[kind]), nil
	case "darwin":
		return filepath.Join(home, "Library", "Application Support"), nil
	default:
		variable, fallback := "XDG_CONFIG_HOME", filepath.Join(home, ".config")
		if kind == "data" {
			variable, fallback = "XDG_DATA_HOME", filepath.Join(home, ".local", "share")
		}
		if value := env.Getenv(variable); filepath.IsAbs(value) {
			return value, nil
		}
		return fallback, nil
	}
}

func Load(dir string) (Config, error) {
	var cfg Config
	path := filepath.Join(dir, fileName)
	info, err := os.Stat(path)
	if errors.Is(err, fs.ErrNotExist) {
		return cfg, nil
	}
	if err != nil {
		return cfg, fmt.Errorf("cannot read %s: %w", path, err)
	}
	if info.Size() > maxFileSize {
		return cfg, fmt.Errorf("%s is larger than %d bytes: delete it to start with defaults", path, maxFileSize)
	}
	raw, err := os.ReadFile(path)
	if err != nil {
		return cfg, fmt.Errorf("cannot read %s: %w", path, err)
	}
	if err := json.Unmarshal(raw, &cfg); err != nil {
		return Config{}, fmt.Errorf("%s is not valid JSON: delete it to start with defaults: %w", path, err)
	}
	if err := cfg.Validate(); err != nil {
		return Config{}, fmt.Errorf("%s: %w", path, err)
	}
	return cfg, nil
}

func (c Config) Validate() error {
	switch c.FrontEnd {
	case "", FrontEndGUI, FrontEndTUI:
	default:
		return fmt.Errorf("front_end must be %q, %q or empty, found %q", FrontEndGUI, FrontEndTUI, c.FrontEnd)
	}
	if c.DatabasePath != "" && !filepath.IsAbs(c.DatabasePath) {
		return fmt.Errorf("database_path must be an absolute path, found %q", c.DatabasePath)
	}
	return nil
}

func Save(dir string, cfg Config) error {
	if err := cfg.Validate(); err != nil {
		return err
	}
	if err := os.MkdirAll(dir, 0o700); err != nil {
		return fmt.Errorf("cannot create %s: %w", dir, err)
	}
	if err := os.Chmod(dir, 0o700); err != nil {
		return fmt.Errorf("cannot restrict %s: %w", dir, err)
	}
	raw, err := json.MarshalIndent(cfg, "", "  ")
	if err != nil {
		return err
	}
	temporary, err := os.CreateTemp(dir, fileName+".*")
	if err != nil {
		return fmt.Errorf("cannot write in %s: %w", dir, err)
	}
	name := temporary.Name()
	_, writeErr := temporary.Write(append(raw, '\n'))
	closeErr := temporary.Close()
	if err := errors.Join(writeErr, closeErr); err != nil {
		_ = os.Remove(name)
		return fmt.Errorf("cannot write %s: %w", name, err)
	}
	if err := os.Chmod(name, 0o600); err != nil {
		_ = os.Remove(name)
		return fmt.Errorf("cannot restrict %s: %w", name, err)
	}
	if err := os.Rename(name, filepath.Join(dir, fileName)); err != nil {
		_ = os.Remove(name)
		return fmt.Errorf("cannot replace %s: %w", filepath.Join(dir, fileName), err)
	}
	return nil
}
