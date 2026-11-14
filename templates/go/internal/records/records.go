package records

import (
	"context"
	"database/sql"
	"errors"
	"fmt"
	"strings"
	"time"
	"unicode"
	"unicode/utf8"

	"github.com/NexzaDev/templateapp/internal/notify"
	"github.com/NexzaDev/templateapp/internal/store"
	"github.com/NexzaDev/templateapp/internal/theme"
)

const (
	StatusOpen       = "open"
	StatusInProgress = "in_progress"
	StatusDone       = "done"

	MaxNameLength = 200
	MaxNoteLength = 4000
	Kind          = "record"

	timeLayout = time.RFC3339

	sqlInsert     = "INSERT INTO records (name, status, severity, note, enabled, is_demo, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?)"
	sqlGet        = "SELECT id, name, status, severity, note, enabled, is_demo, revision, created_at, updated_at FROM records WHERE id = ?"
	sqlGetByName  = "SELECT id, name, status, severity, note, enabled, is_demo, revision, created_at, updated_at FROM records WHERE name = ?"
	sqlUpdate     = "UPDATE records SET name = ?, status = ?, severity = ?, note = ?, enabled = ?, revision = revision + 1, updated_at = ? WHERE id = ? AND revision = ?"
	sqlSetEnabled = "UPDATE records SET enabled = ?, revision = revision + 1, updated_at = ? WHERE id = ? AND enabled <> ?"
	sqlDelete     = "DELETE FROM records WHERE id = ?"
	sqlDeleteDemo = "DELETE FROM records WHERE is_demo = 1"
	sqlCountDemo  = "SELECT count(*) FROM records WHERE is_demo = 1"
	sqlCount      = "SELECT count(*) FROM records"
	sqlList       = `SELECT id, name, status, severity, note, enabled, is_demo, revision, created_at, updated_at
FROM records
WHERE (?1 = '' OR status = ?1)
  AND (?2 = '' OR severity = ?2)
  AND (?3 = 1 OR enabled = 1)
  AND (?4 = '' OR id IN (SELECT rowid FROM records_fts WHERE records_fts MATCH ?4))
ORDER BY
  CASE WHEN ?5 = 'name' AND ?6 = 0 THEN name END COLLATE NOCASE ASC,
  CASE WHEN ?5 = 'name' AND ?6 = 1 THEN name END COLLATE NOCASE DESC,
  CASE WHEN ?5 = 'status' AND ?6 = 0 THEN status END ASC,
  CASE WHEN ?5 = 'status' AND ?6 = 1 THEN status END DESC,
  CASE WHEN ?5 = 'severity' AND ?6 = 0 THEN CASE severity WHEN 'critical' THEN 4 WHEN 'high' THEN 3 WHEN 'medium' THEN 2 WHEN 'low' THEN 1 ELSE 0 END END ASC,
  CASE WHEN ?5 = 'severity' AND ?6 = 1 THEN CASE severity WHEN 'critical' THEN 4 WHEN 'high' THEN 3 WHEN 'medium' THEN 2 WHEN 'low' THEN 1 ELSE 0 END END DESC,
  CASE WHEN ?5 = 'updated' AND ?6 = 0 THEN updated_at END ASC,
  CASE WHEN ?5 = 'updated' AND ?6 = 1 THEN updated_at END DESC,
  id ASC
LIMIT ?7 OFFSET ?8`
	sqlByStatus     = "SELECT id FROM records WHERE status = ? AND severity = ?"
	sqlRecent       = "SELECT id, name, status, severity, note, enabled, is_demo, revision, created_at, updated_at FROM records ORDER BY updated_at DESC LIMIT ?"
	sqlOverview     = "SELECT open_records, urgent_records, disabled_records, demo_records, unread_notifications FROM overview_attention"
	sqlOverviewRows = "SELECT status, severity, total, demo_total FROM overview_record_counts"
)

var HotQueries = map[string]string{
	"records by status and severity": sqlByStatus,
	"recently updated records":       sqlRecent,
	"record by name":                 sqlGetByName,
	"record by identifier":           sqlGet,
}

var Statuses = []string{StatusOpen, StatusInProgress, StatusDone}

var SortKeys = []string{"name", "status", "severity", "updated"}

func ColumnLabel(column string) string {
	switch column {
	case "id":
		return "ID"
	case "name":
		return "Name"
	case "status":
		return "Status"
	case "severity":
		return "Severity"
	case "updated":
		return "Updated"
	case "state":
		return "State"
	}
	return column
}

func StatusLabel(status string) string {
	switch status {
	case StatusOpen:
		return "Open"
	case StatusInProgress:
		return "In progress"
	case StatusDone:
		return "Done"
	}
	return status
}

func StatusGlyph(status string) string {
	switch status {
	case StatusOpen:
		return "○"
	case StatusInProgress:
		return "◐"
	case StatusDone:
		return "●"
	}
	return "?"
}

type Record struct {
	ID        int64
	Name      string
	Status    string
	Severity  theme.Severity
	Note      string
	Enabled   bool
	Demo      bool
	Revision  int64
	CreatedAt time.Time
	UpdatedAt time.Time
}

type Input struct {
	Name     string
	Status   string
	Severity theme.Severity
	Note     string
	Enabled  bool
}

func (r Record) Input() Input {
	return Input{Name: r.Name, Status: r.Status, Severity: r.Severity, Note: r.Note, Enabled: r.Enabled}
}

type ValidationError struct {
	Field   string
	Problem string
	Fix     string
}

func (e *ValidationError) Error() string {
	return e.Field + ": " + e.Problem + ". " + e.Fix
}

type ConflictError struct {
	ID      int64
	Current Record
	Deleted bool
}

func (e *ConflictError) Error() string {
	if e.Deleted {
		return fmt.Sprintf("record %d was removed by another window; reload the list", e.ID)
	}
	return fmt.Sprintf("record %d was changed by another window (now revision %d); reload it and apply your change again", e.ID, e.Current.Revision)
}

var ErrNotFound = errors.New("the record does not exist; list the records to see valid identifiers")

func validStatus(status string) bool {
	return status == StatusOpen || status == StatusInProgress || status == StatusDone
}

func Normalize(in Input) (Input, error) {
	if !utf8.ValidString(in.Name) || !utf8.ValidString(in.Note) {
		return in, &ValidationError{Field: "name", Problem: "the text is not valid UTF-8", Fix: "Re-enter the text"}
	}
	in.Name = strings.Join(strings.Fields(in.Name), " ")
	for _, r := range in.Name {
		if unicode.IsControl(r) {
			return in, &ValidationError{Field: "name", Problem: "the name contains a control character", Fix: "Use letters, digits, spaces and punctuation"}
		}
	}
	if in.Name == "" {
		return in, &ValidationError{Field: "name", Problem: "the name is empty", Fix: "Enter a name of 1 to 200 characters"}
	}
	if utf8.RuneCountInString(in.Name) > MaxNameLength {
		return in, &ValidationError{Field: "name", Problem: fmt.Sprintf("the name has more than %d characters", MaxNameLength), Fix: "Shorten the name"}
	}
	in.Note = strings.TrimSpace(strings.ReplaceAll(in.Note, "\r\n", "\n"))
	for _, r := range in.Note {
		if unicode.IsControl(r) && r != '\n' && r != '\t' {
			return in, &ValidationError{Field: "note", Problem: "the note contains a control character", Fix: "Remove the control character"}
		}
	}
	if utf8.RuneCountInString(in.Note) > MaxNoteLength {
		return in, &ValidationError{Field: "note", Problem: fmt.Sprintf("the note has more than %d characters", MaxNoteLength), Fix: "Shorten the note"}
	}
	if in.Status == "" {
		in.Status = StatusOpen
	}
	in.Status = strings.ToLower(strings.TrimSpace(in.Status))
	if !validStatus(in.Status) {
		return in, &ValidationError{Field: "status", Problem: fmt.Sprintf("unknown status %q", in.Status), Fix: "Use open, in_progress or done"}
	}
	if in.Severity == "" {
		in.Severity = theme.SeverityInfo
	}
	in.Severity = theme.Severity(strings.ToLower(strings.TrimSpace(string(in.Severity))))
	if in.Severity.Rank() < 0 {
		return in, &ValidationError{Field: "severity", Problem: fmt.Sprintf("unknown severity %q", in.Severity), Fix: "Use info, low, medium, high or critical"}
	}
	return in, nil
}

func SearchExpression(text string) string {
	var terms []string
	for _, field := range strings.FieldsFunc(text, func(r rune) bool {
		return !unicode.IsLetter(r) && !unicode.IsDigit(r)
	}) {
		if len(terms) == 16 {
			break
		}
		if utf8.RuneCountInString(field) > 64 {
			field = string([]rune(field)[:64])
		}
		terms = append(terms, `"`+field+`"*`)
	}
	return strings.Join(terms, " ")
}

type Query struct {
	Search          string
	Status          string
	Severity        theme.Severity
	IncludeDisabled bool
	Sort            string
	Descending      bool
	Limit           int
	Offset          int
}

type Service struct {
	db  *sql.DB
	now func() time.Time
}

func New(db *sql.DB, now func() time.Time) *Service {
	return &Service{db: db, now: now}
}

type scanner interface {
	Scan(dest ...any) error
}

func scan(row scanner) (Record, error) {
	var record Record
	var severity, created, updated string
	var enabled, demo int
	if err := row.Scan(&record.ID, &record.Name, &record.Status, &severity, &record.Note, &enabled, &demo, &record.Revision, &created, &updated); err != nil {
		return Record{}, err
	}
	record.Severity, record.Enabled, record.Demo = theme.Severity(severity), enabled == 1, demo == 1
	var err error
	if record.CreatedAt, err = time.Parse(timeLayout, created); err != nil {
		return Record{}, fmt.Errorf("record %d has an unreadable creation time: %w", record.ID, err)
	}
	if record.UpdatedAt, err = time.Parse(timeLayout, updated); err != nil {
		return Record{}, fmt.Errorf("record %d has an unreadable update time: %w", record.ID, err)
	}
	return record, nil
}

func flag(value bool) int {
	if value {
		return 1
	}
	return 0
}

func urgent(status string, severity theme.Severity) bool {
	return status != StatusDone && severity.Rank() >= theme.SeverityHigh.Rank()
}

func duplicateName(err error, name string) error {
	if store.IsConstraint(err) && strings.Contains(err.Error(), "records.name") {
		return &ValidationError{Field: "name", Problem: fmt.Sprintf("a record named %q already exists", name), Fix: "Choose another name or edit the existing record"}
	}
	return err
}

func (s *Service) inTx(ctx context.Context, work func(tx *sql.Tx) error) error {
	return store.Retry(ctx, 5, 40*time.Millisecond, func() error {
		tx, err := s.db.BeginTx(ctx, nil)
		if err != nil {
			return err
		}
		if err := work(tx); err != nil {
			_ = tx.Rollback()
			return err
		}
		return tx.Commit()
	})
}

func announce(ctx context.Context, tx *sql.Tx, now time.Time, before *Record, after Record) error {
	if !after.Enabled {
		return nil
	}
	wasUrgent := before != nil && before.Enabled && urgent(before.Status, before.Severity)
	if urgent(after.Status, after.Severity) && !wasUrgent {
		if _, _, err := notify.PublishWith(ctx, tx, now, notify.Event{Type: notify.EventRecordUrgent, Severity: after.Severity, Title: "A record needs attention", RecordKind: Kind, RecordID: after.ID}); err != nil {
			return err
		}
	}
	if after.Status == StatusDone && before != nil && before.Status != StatusDone {
		if _, _, err := notify.PublishWith(ctx, tx, now, notify.Event{Type: notify.EventRecordCompleted, Severity: theme.SeverityInfo, Title: "A record was completed", RecordKind: Kind, RecordID: after.ID}); err != nil {
			return err
		}
	}
	return nil
}

func create(ctx context.Context, tx *sql.Tx, now time.Time, in Input, demo bool) (Record, error) {
	stamp := now.UTC().Format(timeLayout)
	result, err := tx.ExecContext(ctx, sqlInsert, in.Name, in.Status, string(in.Severity), in.Note, flag(in.Enabled), flag(demo), stamp, stamp)
	if err != nil {
		return Record{}, duplicateName(err, in.Name)
	}
	id, err := result.LastInsertId()
	if err != nil {
		return Record{}, err
	}
	created, err := scan(tx.QueryRowContext(ctx, sqlGet, id))
	if err != nil {
		return Record{}, err
	}
	return created, announce(ctx, tx, now, nil, created)
}

func (s *Service) Create(ctx context.Context, in Input) (Record, error) {
	normalized, err := Normalize(in)
	if err != nil {
		return Record{}, err
	}
	var created Record
	err = s.inTx(ctx, func(tx *sql.Tx) error {
		var inner error
		created, inner = create(ctx, tx, s.now(), normalized, false)
		return inner
	})
	return created, err
}

func (s *Service) Get(ctx context.Context, id int64) (Record, error) {
	record, err := scan(s.db.QueryRowContext(ctx, sqlGet, id))
	if errors.Is(err, sql.ErrNoRows) {
		return Record{}, ErrNotFound
	}
	return record, err
}

func (s *Service) Update(ctx context.Context, id, revision int64, in Input) (Record, error) {
	normalized, err := Normalize(in)
	if err != nil {
		return Record{}, err
	}
	var updated Record
	err = s.inTx(ctx, func(tx *sql.Tx) error {
		before, err := scan(tx.QueryRowContext(ctx, sqlGet, id))
		if errors.Is(err, sql.ErrNoRows) {
			return &ConflictError{ID: id, Deleted: true}
		}
		if err != nil {
			return err
		}
		if before.Revision != revision {
			return &ConflictError{ID: id, Current: before}
		}
		now := s.now()
		result, err := tx.ExecContext(ctx, sqlUpdate, normalized.Name, normalized.Status, string(normalized.Severity), normalized.Note, flag(normalized.Enabled), now.UTC().Format(timeLayout), id, revision)
		if err != nil {
			return duplicateName(err, normalized.Name)
		}
		if changed, err := result.RowsAffected(); err != nil || changed != 1 {
			return &ConflictError{ID: id, Current: before}
		}
		if updated, err = scan(tx.QueryRowContext(ctx, sqlGet, id)); err != nil {
			return err
		}
		return announce(ctx, tx, now, &before, updated)
	})
	return updated, err
}

func (s *Service) SetEnabled(ctx context.Context, id int64, enabled bool) (Record, error) {
	var updated Record
	err := s.inTx(ctx, func(tx *sql.Tx) error {
		before, err := scan(tx.QueryRowContext(ctx, sqlGet, id))
		if errors.Is(err, sql.ErrNoRows) {
			return ErrNotFound
		}
		if err != nil {
			return err
		}
		now := s.now()
		if _, err := tx.ExecContext(ctx, sqlSetEnabled, flag(enabled), now.UTC().Format(timeLayout), id, flag(enabled)); err != nil {
			return err
		}
		if updated, err = scan(tx.QueryRowContext(ctx, sqlGet, id)); err != nil {
			return err
		}
		return announce(ctx, tx, now, &before, updated)
	})
	return updated, err
}

func (s *Service) Delete(ctx context.Context, id int64) error {
	return s.inTx(ctx, func(tx *sql.Tx) error {
		result, err := tx.ExecContext(ctx, sqlDelete, id)
		if err != nil {
			return err
		}
		if removed, err := result.RowsAffected(); err != nil {
			return err
		} else if removed == 0 {
			return ErrNotFound
		}
		return nil
	})
}

func (s *Service) List(ctx context.Context, query Query) ([]Record, error) {
	if query.Status != "" && !validStatus(query.Status) {
		return nil, &ValidationError{Field: "status", Problem: fmt.Sprintf("unknown status %q", query.Status), Fix: "Use open, in_progress or done"}
	}
	if query.Severity != "" && query.Severity.Rank() < 0 {
		return nil, &ValidationError{Field: "severity", Problem: fmt.Sprintf("unknown severity %q", query.Severity), Fix: "Use info, low, medium, high or critical"}
	}
	sortKey := query.Sort
	if sortKey == "" {
		sortKey = "name"
	}
	known := false
	for _, key := range SortKeys {
		known = known || key == sortKey
	}
	if !known {
		return nil, &ValidationError{Field: "sort", Problem: fmt.Sprintf("unknown sort column %q", query.Sort), Fix: "Use name, status, severity or updated"}
	}
	limit := query.Limit
	if limit <= 0 || limit > 10000 {
		limit = 500
	}
	offset := query.Offset
	if offset < 0 {
		offset = 0
	}
	search := SearchExpression(query.Search)
	if strings.TrimSpace(query.Search) != "" && search == "" {
		return nil, nil
	}
	rows, err := s.db.QueryContext(ctx, sqlList, query.Status, string(query.Severity), flag(query.IncludeDisabled), search, sortKey, flag(query.Descending), limit, offset)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	var list []Record
	for rows.Next() {
		record, err := scan(rows)
		if err != nil {
			return nil, err
		}
		list = append(list, record)
	}
	return list, rows.Err()
}

func (s *Service) Count(ctx context.Context) (int, error) {
	var count int
	err := s.db.QueryRowContext(ctx, sqlCount).Scan(&count)
	return count, err
}

type Overview struct {
	Open                int
	Urgent              int
	Disabled            int
	Demo                int
	UnreadNotifications int
	Breakdown           []Breakdown
}

type Breakdown struct {
	Status   string
	Severity theme.Severity
	Total    int
	Demo     int
}

func (o Overview) Total() int {
	total := 0
	for _, row := range o.Breakdown {
		total += row.Total
	}
	return total
}

func (o Overview) ByStatus(status string) int {
	total := 0
	for _, row := range o.Breakdown {
		if row.Status == status {
			total += row.Total
		}
	}
	return total
}

func (o Overview) BySeverity(severity theme.Severity) int {
	total := 0
	for _, row := range o.Breakdown {
		if row.Severity == severity && row.Status != StatusDone {
			total += row.Total
		}
	}
	return total
}

func (s *Service) Overview(ctx context.Context) (Overview, error) {
	var overview Overview
	if err := s.db.QueryRowContext(ctx, sqlOverview).Scan(&overview.Open, &overview.Urgent, &overview.Disabled, &overview.Demo, &overview.UnreadNotifications); err != nil {
		return overview, err
	}
	rows, err := s.db.QueryContext(ctx, sqlOverviewRows)
	if err != nil {
		return overview, err
	}
	defer rows.Close()
	for rows.Next() {
		var row Breakdown
		var severity string
		if err := rows.Scan(&row.Status, &severity, &row.Total, &row.Demo); err != nil {
			return overview, err
		}
		row.Severity = theme.Severity(severity)
		overview.Breakdown = append(overview.Breakdown, row)
	}
	return overview, rows.Err()
}

var demoRecords = []Input{
	{Name: "Demo: edge router firmware review", Status: StatusOpen, Severity: theme.SeverityHigh, Note: "Sample entry. Compare the installed firmware with the vendor advisory list.", Enabled: true},
	{Name: "Demo: rotate backup credentials", Status: StatusInProgress, Severity: theme.SeverityCritical, Note: "Sample entry. Rotation is scheduled for the next maintenance window.", Enabled: true},
	{Name: "Demo: document the lab network", Status: StatusOpen, Severity: theme.SeverityLow, Note: "Sample entry. Diagram the lab VLANs and their gateways.", Enabled: true},
	{Name: "Demo: review firewall change log", Status: StatusDone, Severity: theme.SeverityMedium, Note: "Sample entry. No unexpected rules were found.", Enabled: true},
	{Name: "Demo: archive old inventory export", Status: StatusOpen, Severity: theme.SeverityInfo, Note: "Sample entry. Disabled to show how hidden entries look.", Enabled: false},
	{Name: "Demo: check time synchronisation", Status: StatusOpen, Severity: theme.SeverityMedium, Note: "Sample entry. Two hosts drifted by more than a second.", Enabled: true},
}

func (s *Service) LoadDemo(ctx context.Context) (int, error) {
	added := 0
	err := s.inTx(ctx, func(tx *sql.Tx) error {
		added = 0
		for _, in := range demoRecords {
			if _, err := scan(tx.QueryRowContext(ctx, sqlGetByName, in.Name)); err == nil {
				continue
			} else if !errors.Is(err, sql.ErrNoRows) {
				return err
			}
			if _, err := create(ctx, tx, s.now(), in, true); err != nil {
				return err
			}
			added++
		}
		return nil
	})
	return added, err
}

func (s *Service) DemoCount(ctx context.Context) (int, error) {
	var count int
	err := s.db.QueryRowContext(ctx, sqlCountDemo).Scan(&count)
	return count, err
}

func (s *Service) RemoveDemo(ctx context.Context, dryRun bool) (int, error) {
	if dryRun {
		return s.DemoCount(ctx)
	}
	removed := 0
	err := s.inTx(ctx, func(tx *sql.Tx) error {
		result, err := tx.ExecContext(ctx, sqlDeleteDemo)
		if err != nil {
			return err
		}
		count, err := result.RowsAffected()
		removed = int(count)
		return err
	})
	return removed, err
}
