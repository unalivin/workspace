package db

import (
	"database/sql"
	_ "embed"
	"fmt"

	_ "modernc.org/sqlite"
)

//go:embed ../../migrations/0001_initial.sql
var initialMigration string

type Database struct {
	Conn *sql.DB
}

func Open(path string) (*Database, error) {
	conn, err := sql.Open("sqlite", path)
	if err != nil {
		return nil, fmt.Errorf("open sqlite %s: %w", path, err)
	}
	if _, err := conn.Exec("PRAGMA busy_timeout = 5000; PRAGMA journal_mode = WAL; PRAGMA foreign_keys = ON;"); err != nil {
		conn.Close()
		return nil, fmt.Errorf("configure pragmas: %w", err)
	}
	var version int
	if err := conn.QueryRow("PRAGMA user_version").Scan(&version); err != nil {
		conn.Close()
		return nil, fmt.Errorf("read user_version: %w", err)
	}
	if version < 1 {
		tx, err := conn.Begin()
		if err != nil {
			conn.Close()
			return nil, fmt.Errorf("begin migration: %w", err)
		}
		if _, err := tx.Exec(initialMigration); err != nil {
			tx.Rollback()
			conn.Close()
			return nil, fmt.Errorf("apply migration 1: %w", err)
		}
		if _, err := tx.Exec("PRAGMA user_version = 1"); err != nil {
			tx.Rollback()
			conn.Close()
			return nil, fmt.Errorf("set user_version: %w", err)
		}
		if err := tx.Commit(); err != nil {
			conn.Close()
			return nil, fmt.Errorf("commit migration: %w", err)
		}
	}
	return &Database{Conn: conn}, nil
}

func (d *Database) Close() error {
	return d.Conn.Close()
}
