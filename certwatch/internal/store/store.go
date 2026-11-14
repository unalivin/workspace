package store

import (
	"context"
	"database/sql"
	"errors"
	"fmt"
	"io"
	"io/fs"
	"os"
	"path/filepath"
	"regexp"
	"sort"
	"strconv"
	"strings"
	"sync"
	"time"

	sqlite3 "github.com/mattn/go-sqlite3"
)

const (
	defaultBusyTimeout = 5 * time.Second
	sqlUserVersion     = "PRAGMA user_version"
	sqlDataVersion     = "PRAGMA data_version"
	sqlIntegrityCheck  = "PRAGMA integrity_check"
	sqlForeignKeyCheck = "PRAGMA foreign_key_check"
	sqlJournalMode     = "PRAGMA journal_mode"
	sqlForeignKeys     = "PRAGMA foreign_keys"
	sqlBusyTimeout     = "PRAGMA busy_timeout"
	sqlSchemaSnapshot  = "SELECT type, name, coalesce(sql, '') FROM sqlite_master WHERE name NOT LIKE 'sqlite_%' AND name NOT LIKE 'records_fts_%' ORDER BY type, name"
	sqlCompileOptions  = "PRAGMA compile_options"
)

var migrationName = regexp.MustCompile(`^(\d{4})_([a-z0-9_]+)\.sql$`)

type Options struct {
	ReadOnly    bool
	BusyTimeout time.Duration
}

type Store struct {
	db        *sql.DB
	path      string
	readOnly  bool
	watcher   *sql.Conn
	watchLock sync.Mutex
}

type Migration struct {
	Number int
	Name   string
	SQL    string
}

func LoadMigrations(files fs.FS) ([]Migration, error) {
	entries, err := fs.ReadDir(files, ".")
	if err != nil {
		return nil, fmt.Errorf("cannot list migrations: %w", err)
	}
	var migrations []Migration
	for _, entry := range entries {
		if entry.IsDir() || !strings.HasSuffix(entry.Name(), ".sql") {
			continue
		}
		match := migrationName.FindStringSubmatch(entry.Name())
		if match == nil {
			return nil, fmt.Errorf("migration %q must be named NNNN_name.sql", entry.Name())
		}
		number, err := strconv.Atoi(match[1])
		if err != nil {
			return nil, err
		}
		content, err := fs.ReadFile(files, entry.Name())
		if err != nil {
			return nil, fmt.Errorf("cannot read migration %s: %w", entry.Name(), err)
		}
		text := strings.TrimSpace(string(content))
		marker := "user_version = " + strconv.Itoa(number) + ";"
		if !strings.HasSuffix(text, "PRAGMA "+marker) {
			return nil, fmt.Errorf("migration %s must end with %q", entry.Name(), marker)
		}
		migrations = append(migrations, Migration{Number: number, Name: entry.Name(), SQL: text})
	}
	sort.Slice(migrations, func(i, j int) bool { return migrations[i].Number < migrations[j].Number })
	for index, migration := range migrations {
		if migration.Number != index+1 {
			return nil, fmt.Errorf("migrations must be numbered without gaps from 0001: found %s at position %d", migration.Name, index+1)
		}
	}
	if len(migrations) == 0 {
		return nil, errors.New("no migrations found")
	}
	return migrations, nil
}

func dataSourceName(path string, opts Options) string {
	escaped := strings.NewReplacer("%", "%25", "?", "%3f", "#", "%23").Replace(filepath.ToSlash(path))
	if !strings.HasPrefix(escaped, "/") {
		escaped = "/" + escaped
	}
	timeout := opts.BusyTimeout
	if timeout <= 0 {
		timeout = defaultBusyTimeout
	}
	parameters := []string{
		"_busy_timeout=" + strconv.FormatInt(timeout.Milliseconds(), 10),
		"_foreign_keys=on",
	}
	if opts.ReadOnly {
		parameters = append(parameters, "mode=ro", "_query_only=on")
	} else {
		parameters = append(parameters, "mode=rwc", "_journal_mode=WAL", "_synchronous=NORMAL", "_txlock=immediate")
	}
	return "file://" + escaped + "?" + strings.Join(parameters, "&")
}

const fileHeader = "SQLite format 3\x00"

func checkHeader(path string) error {
	file, err := os.Open(path)
	if errors.Is(err, fs.ErrNotExist) {
		return nil
	}
	if err != nil {
		return fmt.Errorf("the database file %q cannot be read: %w", path, err)
	}
	defer file.Close()
	header := make([]byte, len(fileHeader))
	count, err := io.ReadFull(file, header)
	if count == 0 && errors.Is(err, io.EOF) {
		return nil
	}
	if err != nil || string(header) != fileHeader {
		return fmt.Errorf("%q is not a usable SQLite database and was left untouched; choose another file", path)
	}
	return nil
}

func Open(ctx context.Context, path string, opts Options) (*Store, error) {
	if !filepath.IsAbs(path) {
		return nil, fmt.Errorf("database path %q must be absolute", path)
	}
	path = filepath.Clean(path)
	if info, err := os.Stat(path); err == nil && info.IsDir() {
		return nil, fmt.Errorf("database path %q is a directory: choose a file path", path)
	}
	if err := checkHeader(path); err != nil {
		return nil, err
	}
	if opts.ReadOnly {
		if _, err := os.Stat(path); err != nil {
			return nil, fmt.Errorf("database %q cannot be opened read-only: %w", path, err)
		}
	} else {
		if err := os.MkdirAll(filepath.Dir(path), 0o700); err != nil {
			return nil, fmt.Errorf("cannot create the database directory: %w", err)
		}
		file, err := os.OpenFile(path, os.O_RDWR|os.O_CREATE, 0o600)
		if err != nil {
			return nil, fmt.Errorf("cannot create or open the database file: %w", err)
		}
		if err := file.Close(); err != nil {
			return nil, err
		}
	}
	db, err := sql.Open("sqlite3", dataSourceName(path, opts))
	if err != nil {
		return nil, err
	}
	db.SetMaxOpenConns(4)
	store := &Store{db: db, path: path, readOnly: opts.ReadOnly}
	if err := Retry(ctx, 40, 25*time.Millisecond, func() error { return db.PingContext(ctx) }); err != nil {
		_ = db.Close()
		return nil, fmt.Errorf("%q is not a usable SQLite database or cannot be opened; choose another file: %w", path, err)
	}
	if _, err := store.SchemaVersion(ctx); err != nil {
		_ = db.Close()
		return nil, fmt.Errorf("%q is not a usable SQLite database; choose another file: %w", path, err)
	}
	return store, nil
}

func (s *Store) DB() *sql.DB    { return s.db }
func (s *Store) Path() string   { return s.path }
func (s *Store) ReadOnly() bool { return s.readOnly }

func (s *Store) Close() error {
	var watcherErr error
	if s.watcher != nil {
		watcherErr = s.watcher.Close()
		s.watcher = nil
	}
	return errors.Join(watcherErr, s.db.Close())
}

func (s *Store) queryInt(ctx context.Context, query string) (int64, error) {
	var value int64
	err := s.db.QueryRowContext(ctx, query).Scan(&value)
	return value, err
}

func (s *Store) queryText(ctx context.Context, query string) (string, error) {
	var value string
	err := s.db.QueryRowContext(ctx, query).Scan(&value)
	return value, err
}

func (s *Store) SchemaVersion(ctx context.Context) (int, error) {
	version, err := s.queryInt(ctx, sqlUserVersion)
	return int(version), err
}

func (s *Store) JournalMode(ctx context.Context) (string, error) {
	return s.queryText(ctx, sqlJournalMode)
}

func (s *Store) ForeignKeysEnabled(ctx context.Context) (bool, error) {
	value, err := s.queryInt(ctx, sqlForeignKeys)
	return value == 1, err
}

func (s *Store) BusyTimeout(ctx context.Context) (time.Duration, error) {
	value, err := s.queryInt(ctx, sqlBusyTimeout)
	return time.Duration(value) * time.Millisecond, err
}

func (s *Store) DataVersion(ctx context.Context) (int64, error) {
	s.watchLock.Lock()
	defer s.watchLock.Unlock()
	if s.watcher == nil {
		conn, err := s.db.Conn(ctx)
		if err != nil {
			return 0, err
		}
		s.watcher = conn
	}
	var version int64
	err := s.watcher.QueryRowContext(ctx, sqlDataVersion).Scan(&version)
	return version, err
}

func (s *Store) Migrate(ctx context.Context, migrations []Migration) (int, error) {
	if s.readOnly {
		return 0, errors.New("a read-only database cannot be migrated")
	}
	current, err := s.SchemaVersion(ctx)
	if err != nil {
		return 0, err
	}
	if current > len(migrations) {
		return 0, fmt.Errorf("the database has schema version %d but this build knows only %d: use a newer build", current, len(migrations))
	}
	applied := 0
	for _, migration := range migrations {
		if migration.Number <= current {
			continue
		}
		done, err := s.applyMigration(ctx, migration)
		if err != nil {
			return applied, fmt.Errorf("migration %s failed and was rolled back: %w", migration.Name, err)
		}
		if done {
			applied++
		}
	}
	return applied, nil
}

func (s *Store) applyMigration(ctx context.Context, migration Migration) (bool, error) {
	tx, err := s.db.BeginTx(ctx, nil)
	if err != nil {
		return false, err
	}
	defer func() { _ = tx.Rollback() }()
	var current int
	if err := tx.QueryRowContext(ctx, sqlUserVersion).Scan(&current); err != nil {
		return false, err
	}
	if current >= migration.Number {
		return false, nil
	}
	if current != migration.Number-1 {
		return false, fmt.Errorf("expected schema version %d, found %d", migration.Number-1, current)
	}
	if _, err := tx.ExecContext(ctx, migration.SQL); err != nil {
		return false, err
	}
	return true, tx.Commit()
}

func (s *Store) IntegrityCheck(ctx context.Context) error {
	result, err := s.queryText(ctx, sqlIntegrityCheck)
	if err != nil {
		return err
	}
	if result != "ok" {
		return fmt.Errorf("integrity check reported: %s", result)
	}
	return nil
}

func (s *Store) ForeignKeyCheck(ctx context.Context) error {
	rows, err := s.db.QueryContext(ctx, sqlForeignKeyCheck)
	if err != nil {
		return err
	}
	defer rows.Close()
	if rows.Next() {
		return errors.New("foreign key check reported at least one violation")
	}
	return rows.Err()
}

func (s *Store) SchemaSnapshot(ctx context.Context) (string, error) {
	rows, err := s.db.QueryContext(ctx, sqlSchemaSnapshot)
	if err != nil {
		return "", err
	}
	defer rows.Close()
	var builder strings.Builder
	for rows.Next() {
		var kind, name, statement string
		if err := rows.Scan(&kind, &name, &statement); err != nil {
			return "", err
		}
		fmt.Fprintf(&builder, "[%s %s]\n%s\n\n", kind, name, strings.TrimSpace(statement))
	}
	return builder.String(), rows.Err()
}

func (s *Store) HasFullTextSearch(ctx context.Context) (bool, error) {
	rows, err := s.db.QueryContext(ctx, sqlCompileOptions)
	if err != nil {
		return false, err
	}
	defer rows.Close()
	for rows.Next() {
		var option string
		if err := rows.Scan(&option); err != nil {
			return false, err
		}
		if option == "ENABLE_FTS5" {
			return true, nil
		}
	}
	return false, rows.Err()
}

func IsBusy(err error) bool {
	var sqliteErr sqlite3.Error
	if errors.As(err, &sqliteErr) {
		return sqliteErr.Code == sqlite3.ErrBusy || sqliteErr.Code == sqlite3.ErrLocked
	}
	return false
}

func IsConstraint(err error) bool {
	var sqliteErr sqlite3.Error
	return errors.As(err, &sqliteErr) && sqliteErr.Code == sqlite3.ErrConstraint
}

func Retry(ctx context.Context, attempts int, wait time.Duration, operation func() error) error {
	var err error
	for attempt := 0; attempt < attempts; attempt++ {
		err = operation()
		if err == nil || !IsBusy(err) {
			return err
		}
		select {
		case <-ctx.Done():
			return errors.Join(err, ctx.Err())
		case <-time.After(wait):
		}
	}
	return fmt.Errorf("the database stayed busy after %d attempts: %w", attempts, err)
}
