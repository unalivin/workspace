package app

import (
	"context"
	"errors"
	"fmt"
	"io"
	"io/fs"
	"os"
	"path/filepath"
	"sync"
	"time"

	"github.com/unalivin/templateapp/internal/meta"
	"github.com/unalivin/templateapp/internal/notify"
	"github.com/unalivin/templateapp/internal/records"
	"github.com/unalivin/templateapp/internal/settings"
	"github.com/unalivin/templateapp/internal/store"
	"github.com/unalivin/templateapp/internal/theme"
	"github.com/unalivin/templateapp/internal/userconfig"
	"github.com/unalivin/templateapp/migrations"
)

const sqlCopyInto = "VACUUM INTO ?"

type Options struct {
	Environment  userconfig.Environment
	ConfigDir    string
	DatabasePath string
	Overrides    map[string]string
	Now          func() time.Time
	ReadOnly     bool
}

type Location struct {
	Path         string
	Source       string
	OverriddenBy string
	Exists       bool
	ConfigDir    string
	Config       userconfig.Config
}

type App struct {
	Store    *store.Store
	Settings *settings.Service
	Records  *records.Service
	Notices  *notify.Service

	location Location
	options  Options
	created  bool
	lock     sync.Mutex
	seen     int64
}

func (o Options) withDefaults() Options {
	if o.Environment.Getenv == nil {
		o.Environment = userconfig.SystemEnvironment()
	}
	if o.Now == nil {
		o.Now = time.Now
	}
	return o
}

func Locate(options Options) (Location, error) {
	options = options.withDefaults()
	location := Location{ConfigDir: options.ConfigDir}
	if location.ConfigDir == "" {
		dir, err := userconfig.Dir(options.Environment)
		if err != nil {
			return location, err
		}
		location.ConfigDir = dir
	}
	config, err := userconfig.Load(location.ConfigDir)
	if err != nil {
		return location, err
	}
	location.Config = config
	switch {
	case options.DatabasePath != "":
		location.Path, location.Source, location.OverriddenBy = options.DatabasePath, "flag", "command-line flag --db"
	case options.Environment.Getenv(meta.EnvName("db")) != "":
		location.Path, location.Source, location.OverriddenBy = options.Environment.Getenv(meta.EnvName("db")), "environment", "environment variable "+meta.EnvName("db")
	case config.DatabasePath != "":
		location.Path, location.Source = config.DatabasePath, "saved"
	default:
		path, err := userconfig.DefaultDatabasePath(options.Environment)
		if err != nil {
			return location, err
		}
		location.Path, location.Source = path, "default"
	}
	absolute, err := filepath.Abs(location.Path)
	if err != nil {
		return location, fmt.Errorf("the database path %q cannot be resolved: %w", location.Path, err)
	}
	location.Path = absolute
	info, err := os.Stat(location.Path)
	location.Exists = err == nil && info.Mode().IsRegular() && info.Size() > 0
	return location, nil
}

func Open(ctx context.Context, options Options) (*App, error) {
	options = options.withDefaults()
	location, err := Locate(options)
	if err != nil {
		return nil, err
	}
	application := &App{options: options}
	if err := application.open(ctx, location); err != nil {
		return nil, err
	}
	return application, nil
}

func (a *App) open(ctx context.Context, location Location) error {
	if a.options.ReadOnly && !location.Exists {
		return fmt.Errorf("the database %q does not exist yet: start the application once to create it", location.Path)
	}
	opened, err := store.Open(ctx, location.Path, store.Options{ReadOnly: a.options.ReadOnly})
	if err != nil {
		return err
	}
	if !a.options.ReadOnly {
		loaded, err := store.LoadMigrations(migrations.Files)
		if err != nil {
			_ = opened.Close()
			return err
		}
		if _, err := opened.Migrate(ctx, loaded); err != nil {
			_ = opened.Close()
			return err
		}
	}
	a.Store = opened
	a.Settings = settings.New(opened.DB(), a.options.Now, a.options.Environment.Getenv, a.options.Overrides)
	a.Records = records.New(opened.DB(), a.options.Now)
	a.Notices = notify.New(opened.DB(), a.options.Now)
	a.created = !location.Exists
	location.Exists = true
	a.location = location
	a.seen, err = opened.DataVersion(ctx)
	return err
}

func (a *App) Close() error {
	if a.Store == nil {
		return nil
	}
	return a.Store.Close()
}

func (a *App) Location() Location { return a.location }

func (a *App) CreatedDatabase() bool { return a.created }

func (a *App) Now() time.Time { return a.options.Now() }

func (a *App) FrontEnd() string { return a.location.Config.FrontEnd }

func (a *App) RememberFrontEnd(mode string) error {
	if mode != "" && mode != userconfig.FrontEndGUI && mode != userconfig.FrontEndTUI {
		return fmt.Errorf("unknown front end %q: use gui, tui or an empty value to ask every time", mode)
	}
	config := a.location.Config
	config.FrontEnd = mode
	if err := userconfig.Save(a.location.ConfigDir, config); err != nil {
		return err
	}
	a.location.Config = config
	return nil
}

type MoveMode int

const (
	MoveCopy MoveMode = iota
	MoveSwitch
)

type MovePlan struct {
	From        string
	To          string
	Mode        MoveMode
	TargetState string
	Statement   string
}

func (a *App) PlanMove(target string, mode MoveMode) (MovePlan, error) {
	plan := MovePlan{From: a.location.Path, Mode: mode}
	if a.location.OverriddenBy != "" {
		return plan, fmt.Errorf("the database location is set by the %s, so the saved location would not take effect: remove the override first", a.location.OverriddenBy)
	}
	if target == "" {
		return plan, errors.New("the new database path is empty: enter a file path")
	}
	absolute, err := filepath.Abs(target)
	if err != nil {
		return plan, fmt.Errorf("the path %q cannot be resolved: %w", target, err)
	}
	plan.To = absolute
	if absolute == a.location.Path {
		return plan, errors.New("the database is already at this location")
	}
	info, err := os.Stat(absolute)
	switch {
	case errors.Is(err, fs.ErrNotExist):
		plan.TargetState = "missing"
	case err != nil:
		return plan, fmt.Errorf("the path %q cannot be checked: %w", absolute, err)
	case info.IsDir():
		return plan, fmt.Errorf("%q is a directory: add a file name such as %s.db", absolute, meta.Slug)
	default:
		plan.TargetState = "existing"
	}
	switch {
	case mode == MoveCopy && plan.TargetState == "existing":
		return plan, fmt.Errorf("%q already exists and would be overwritten: choose a new file name, or switch to the existing database instead", absolute)
	case mode == MoveCopy:
		plan.Statement = fmt.Sprintf("Copy the current database to %s and use the copy from now on. The file at %s stays untouched.", plan.To, plan.From)
	case plan.TargetState == "existing":
		plan.Statement = fmt.Sprintf("Use the existing database at %s from now on. The file at %s stays untouched.", plan.To, plan.From)
	default:
		plan.Statement = fmt.Sprintf("Create a new empty database at %s and use it from now on. The file at %s stays untouched.", plan.To, plan.From)
	}
	return plan, nil
}

func (a *App) Move(ctx context.Context, plan MovePlan) error {
	a.lock.Lock()
	defer a.lock.Unlock()
	if plan.From != a.location.Path || plan.To == "" {
		return errors.New("the plan does not match the open database: plan the change again")
	}
	if plan.Mode == MoveCopy {
		if err := os.MkdirAll(filepath.Dir(plan.To), 0o700); err != nil {
			return fmt.Errorf("cannot create %s: %w", filepath.Dir(plan.To), err)
		}
		if _, err := a.Store.DB().ExecContext(ctx, sqlCopyInto, plan.To); err != nil {
			return fmt.Errorf("cannot copy the database to %s: %w", plan.To, err)
		}
		if err := os.Chmod(plan.To, 0o600); err != nil {
			return err
		}
	}
	candidate := &App{options: a.options}
	target := a.location
	target.Path, target.Source = plan.To, "saved"
	info, err := os.Stat(plan.To)
	target.Exists = err == nil && info.Size() > 0
	if err := candidate.open(ctx, target); err != nil {
		return err
	}
	config := a.location.Config
	config.DatabasePath = plan.To
	if err := userconfig.Save(a.location.ConfigDir, config); err != nil {
		_ = candidate.Close()
		return err
	}
	previous := a.Store
	a.Store, a.Settings, a.Records, a.Notices = candidate.Store, candidate.Settings, candidate.Records, candidate.Notices
	a.location, a.created, a.seen = candidate.location, candidate.created, candidate.seen
	a.location.Config = config
	return previous.Close()
}

type Change struct {
	Changed bool
	Version int64
}

func (a *App) Poll(ctx context.Context) (Change, error) {
	a.lock.Lock()
	defer a.lock.Unlock()
	version, err := a.Store.DataVersion(ctx)
	if err != nil {
		return Change{}, err
	}
	change := Change{Changed: version != a.seen, Version: version}
	a.seen = version
	return change, nil
}

func (a *App) Acknowledge(ctx context.Context) error {
	_, err := a.Poll(ctx)
	return err
}

func (a *App) Appearance(ctx context.Context, systemDark bool) (theme.Tokens, error) {
	values, err := a.Settings.Effective(ctx)
	if err != nil {
		return theme.Tokens{}, err
	}
	return theme.Resolve(theme.Options{
		Name:          values[settings.KeyTheme].Raw,
		Mode:          theme.Mode(values[settings.KeyMode].Raw),
		SystemDark:    systemDark,
		Accent:        values[settings.KeyAccent].Raw,
		Density:       values[settings.KeyDensity].Raw,
		TextScale:     values[settings.KeyTextScale].Int(),
		ReducedMotion: values[settings.KeyReducedMotion].Bool(),
	}), nil
}

func (a *App) FormatTime(ctx context.Context, moment time.Time) string {
	zone := time.Local
	label := ""
	if value, err := a.Settings.Get(ctx, settings.KeyTimeZone); err == nil && value.Raw == "utc" {
		zone, label = time.UTC, " UTC"
	}
	return moment.In(zone).Format("2006-01-02 15:04") + label
}

func (a *App) Notifier(deliverer notify.Deliverer, stateDir string) (*notify.Runner, error) {
	ctx := context.Background()
	values, err := a.Settings.Effective(ctx)
	if err != nil {
		return nil, err
	}
	path := a.location.Path
	now := a.options.Now
	if stateDir == "" {
		stateDir = a.location.ConfigDir
	}
	return &notify.Runner{
		Interval:   time.Duration(values[settings.KeyNotifierInterval].Int()) * time.Second,
		ShowTitles: values[settings.KeyNotifierTitles].Bool(),
		StateDir:   stateDir,
		Deliverer:  deliverer,
		Open: func(ctx context.Context) (notify.Source, io.Closer, error) {
			readOnly, err := store.Open(ctx, path, store.Options{ReadOnly: true})
			if err != nil {
				return nil, nil, err
			}
			return notify.New(readOnly.DB(), now), readOnly, nil
		},
	}, nil
}
