package clean

import (
	"context"
	"database/sql"
	"fmt"
	"strings"
)

const (
	tableColumns = "id, name"
	sqlSelect    = "SELECT " + tableColumns + " FROM records WHERE name = ?"
	sqlInsert    = `INSERT INTO records (name) VALUES (?)`
)

func constantQueries(ctx context.Context, db *sql.DB, name string) error {
	if _, err := db.ExecContext(ctx, sqlInsert, name); err != nil {
		return err
	}
	if _, err := db.QueryContext(ctx, sqlSelect, name); err != nil {
		return err
	}
	_, err := db.ExecContext(ctx, "DELETE FROM records WHERE name = ?", name)
	return err
}

func proseIsNotSQL(name string) string {
	message := "Select a record from the list, " + name
	message += " or update the name and set a status"
	return fmt.Sprintf("Delete %s from the list? %s", name, message)
}

func joinedWords(words []string) string {
	return strings.Join(words, ", ")
}

type runner struct{}

func (runner) Exec(command string) string { return command }

func unrelatedExec(r runner) string {
	return r.Exec("select" + "ed")
}
