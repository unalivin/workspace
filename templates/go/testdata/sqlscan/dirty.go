package dirty

import (
	"context"
	"database/sql"
	"fmt"
	"strings"
)

const tableName = "records"

func concatenatedArgument(ctx context.Context, db *sql.DB, name string) {
	db.QueryRowContext(ctx, "SELECT id FROM records WHERE name = '"+name+"'")
}

func formattedArgument(ctx context.Context, db *sql.DB, column string) {
	db.QueryContext(ctx, fmt.Sprintf("SELECT id FROM records ORDER BY %s", column))
}

func variableArgument(ctx context.Context, db *sql.DB, query string) {
	db.ExecContext(ctx, query)
}

func concatenatedVariable(name string) string {
	query := "DELETE FROM records WHERE name = '" + name + "'"
	return query
}

func formattedVariable(limit int) string {
	return fmt.Sprintf("select id from records limit %d", limit)
}

func appendedClause(filter string) string {
	query := "SELECT id FROM records"
	query += " WHERE status = '" + filter + "'"
	return query
}

func builderText(column string) string {
	var builder strings.Builder
	builder.WriteString("UPDATE records SET ")
	builder.WriteString(column)
	return builder.String()
}

func joinedText(columns []string) string {
	return strings.Join([]string{"SELECT " + columns[0] + " FROM records"}, " ")
}

func legacyCall(db *sql.DB, id string) {
	db.Exec("DELETE FROM " + tableName + " WHERE id = " + id)
}

func preparedFromVariable(db *sql.DB, statements []string) {
	db.Prepare(statements[0])
}
