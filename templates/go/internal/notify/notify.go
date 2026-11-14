package notify

import (
	"context"
	"database/sql"
	"errors"
	"fmt"
	"regexp"
	"strings"
	"time"
	"unicode"

	"github.com/NexzaDev/templateapp/internal/theme"
)

const (
	EventRecordUrgent    = "record.urgent"
	EventRecordCompleted = "record.completed"

	maxTitleLength = 120
	timeLayout     = time.RFC3339

	sqlSelectRule     = "SELECT enabled, min_severity, quiet_start, quiet_end, dedup_minutes FROM notification_rules WHERE event_type = ?"
	sqlUpsertRule     = "INSERT INTO notification_rules (event_type, enabled, min_severity, quiet_start, quiet_end, dedup_minutes, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?) ON CONFLICT (event_type) DO UPDATE SET enabled = excluded.enabled, min_severity = excluded.min_severity, quiet_start = excluded.quiet_start, quiet_end = excluded.quiet_end, dedup_minutes = excluded.dedup_minutes, updated_at = excluded.updated_at"
	sqlDeleteRule     = "DELETE FROM notification_rules WHERE event_type = ?"
	sqlLastSimilar    = "SELECT created_at FROM notifications WHERE event_type = ? AND dedup_key = ? ORDER BY created_at DESC LIMIT 1"
	sqlInsert         = "INSERT INTO notifications (event_type, severity, title, record_kind, record_id, dedup_key, created_at) VALUES (?, ?, ?, ?, ?, ?, ?)"
	sqlList           = "SELECT id, event_type, severity, title, record_kind, record_id, created_at, read_at FROM notifications WHERE (? = 0 OR read_at IS NULL) AND (? = '' OR severity = ?) AND (? = '' OR event_type = ?) ORDER BY created_at DESC, id DESC LIMIT ?"
	sqlUnreadCount    = "SELECT unread_notifications FROM overview_attention"
	sqlMarkRead       = "UPDATE notifications SET read_at = ? WHERE id = ? AND read_at IS NULL"
	sqlMarkAllRead    = "UPDATE notifications SET read_at = ? WHERE read_at IS NULL"
	sqlAfter          = "SELECT id, event_type, severity, title, record_kind, record_id, created_at, read_at FROM notifications WHERE id > ? ORDER BY id LIMIT ?"
	sqlHighestID      = "SELECT coalesce(max(id), 0) FROM notifications"
	sqlNotificationID = "SELECT id FROM notifications WHERE id = ?"
)

var HotQueries = map[string]string{
	"notification de-duplication lookup": sqlLastSimilar,
	"notifications after cursor":         sqlAfter,
	"mark notification read":             sqlMarkRead,
}

type EventType struct {
	ID          string
	Label       string
	Description string
}

var eventTypes = []EventType{
	{ID: EventRecordUrgent, Label: "Record needs attention", Description: "An open record has high or critical severity."},
	{ID: EventRecordCompleted, Label: "Record completed", Description: "A record was marked as done."},
}

func EventTypes() []EventType {
	return append([]EventType(nil), eventTypes...)
}

func knownEvent(id string) bool {
	for _, event := range eventTypes {
		if event.ID == id {
			return true
		}
	}
	return false
}

type Rule struct {
	EventType    string
	Enabled      bool
	MinSeverity  theme.Severity
	QuietStart   string
	QuietEnd     string
	DedupMinutes int
	Customized   bool
}

func DefaultRule(eventType string) Rule {
	return Rule{EventType: eventType, Enabled: true, MinSeverity: theme.SeverityInfo, DedupMinutes: 60}
}

type RuleError struct {
	Problem string
	Fix     string
}

func (e *RuleError) Error() string { return e.Problem + ". " + e.Fix }

var clockPattern = regexp.MustCompile(`^([01][0-9]|2[0-3]):[0-5][0-9]$`)

func (r Rule) Validate() error {
	if !knownEvent(r.EventType) {
		return &RuleError{Problem: fmt.Sprintf("unknown event type %q", r.EventType), Fix: "List the notification rules to see the event types"}
	}
	if r.MinSeverity.Rank() < 0 {
		return &RuleError{Problem: fmt.Sprintf("unknown severity %q", r.MinSeverity), Fix: "Use info, low, medium, high or critical"}
	}
	if (r.QuietStart == "") != (r.QuietEnd == "") {
		return &RuleError{Problem: "quiet hours need both a start and an end", Fix: "Give both times as HH:MM or leave both empty"}
	}
	for _, clock := range []string{r.QuietStart, r.QuietEnd} {
		if clock != "" && !clockPattern.MatchString(clock) {
			return &RuleError{Problem: fmt.Sprintf("%q is not a time of day", clock), Fix: "Use 24-hour HH:MM, for example 22:30"}
		}
	}
	if r.DedupMinutes < 0 || r.DedupMinutes > 10080 {
		return &RuleError{Problem: fmt.Sprintf("de-duplication window %d is out of range", r.DedupMinutes), Fix: "Use 0 to 10080 minutes (0 turns it off)"}
	}
	return nil
}

type Outcome int

const (
	Delivered Outcome = iota
	SuppressedDisabled
	SuppressedSeverity
	SuppressedQuietHours
	SuppressedDuplicate
)

func (o Outcome) String() string {
	return [...]string{"delivered", "suppressed: rule disabled", "suppressed: below severity threshold", "suppressed: quiet hours", "suppressed: duplicate"}[o]
}

type Event struct {
	Type       string
	Severity   theme.Severity
	Title      string
	RecordKind string
	RecordID   int64
	DedupKey   string
}

func minutesOfDay(clock string) int {
	return int(clock[0]-'0')*600 + int(clock[1]-'0')*60 + int(clock[3]-'0')*10 + int(clock[4]-'0')
}

func InQuietHours(rule Rule, now time.Time) bool {
	if rule.QuietStart == "" || rule.QuietEnd == "" || rule.QuietStart == rule.QuietEnd {
		return false
	}
	start, end, current := minutesOfDay(rule.QuietStart), minutesOfDay(rule.QuietEnd), now.Hour()*60+now.Minute()
	if start < end {
		return current >= start && current < end
	}
	return current >= start || current < end
}

func Evaluate(rule Rule, event Event, now time.Time, lastSimilar *time.Time) Outcome {
	if !rule.Enabled {
		return SuppressedDisabled
	}
	if event.Severity.Rank() < rule.MinSeverity.Rank() {
		return SuppressedSeverity
	}
	if InQuietHours(rule, now) {
		return SuppressedQuietHours
	}
	if lastSimilar != nil && rule.DedupMinutes > 0 && now.Sub(*lastSimilar) < time.Duration(rule.DedupMinutes)*time.Minute && !now.Before(*lastSimilar) {
		return SuppressedDuplicate
	}
	return Delivered
}

var secretLike = regexp.MustCompile(`[A-Za-z0-9+/_=-]{24,}`)

func SafeTitle(title string) string {
	cleaned := strings.Map(func(r rune) rune {
		if unicode.IsControl(r) {
			return ' '
		}
		return r
	}, title)
	cleaned = strings.Join(strings.Fields(cleaned), " ")
	cleaned = secretLike.ReplaceAllString(cleaned, "[redacted]")
	runes := []rune(cleaned)
	if len(runes) > maxTitleLength {
		cleaned = string(runes[:maxTitleLength-1]) + "…"
	}
	if cleaned == "" {
		cleaned = "Notification"
	}
	return cleaned
}

type Notification struct {
	ID         int64
	EventType  string
	Severity   theme.Severity
	Title      string
	RecordKind string
	RecordID   int64
	CreatedAt  time.Time
	ReadAt     *time.Time
}

func (n Notification) Unread() bool { return n.ReadAt == nil }

type Filter struct {
	UnreadOnly bool
	Severity   theme.Severity
	EventType  string
	Limit      int
}

type Service struct {
	db  *sql.DB
	now func() time.Time
}

func New(db *sql.DB, now func() time.Time) *Service {
	return &Service{db: db, now: now}
}

type querier interface {
	QueryRowContext(ctx context.Context, query string, args ...any) *sql.Row
	ExecContext(ctx context.Context, query string, args ...any) (sql.Result, error)
}

func loadRule(ctx context.Context, db querier, eventType string) (Rule, error) {
	rule := DefaultRule(eventType)
	var enabled int
	var severity string
	err := db.QueryRowContext(ctx, sqlSelectRule, eventType).Scan(&enabled, &severity, &rule.QuietStart, &rule.QuietEnd, &rule.DedupMinutes)
	if errors.Is(err, sql.ErrNoRows) {
		return rule, nil
	}
	if err != nil {
		return Rule{}, err
	}
	rule.Enabled, rule.MinSeverity, rule.Customized = enabled == 1, theme.Severity(severity), true
	return rule, nil
}

func (s *Service) Rule(ctx context.Context, eventType string) (Rule, error) {
	if !knownEvent(eventType) {
		return Rule{}, &RuleError{Problem: fmt.Sprintf("unknown event type %q", eventType), Fix: "List the notification rules to see the event types"}
	}
	return loadRule(ctx, s.db, eventType)
}

func (s *Service) Rules(ctx context.Context) ([]Rule, error) {
	rules := make([]Rule, 0, len(eventTypes))
	for _, event := range eventTypes {
		rule, err := loadRule(ctx, s.db, event.ID)
		if err != nil {
			return nil, err
		}
		rules = append(rules, rule)
	}
	return rules, nil
}

func (s *Service) SetRule(ctx context.Context, rule Rule) error {
	if err := rule.Validate(); err != nil {
		return err
	}
	enabled := 0
	if rule.Enabled {
		enabled = 1
	}
	_, err := s.db.ExecContext(ctx, sqlUpsertRule, rule.EventType, enabled, string(rule.MinSeverity), rule.QuietStart, rule.QuietEnd, rule.DedupMinutes, s.now().UTC().Format(timeLayout))
	return err
}

func (s *Service) ResetRule(ctx context.Context, eventType string) error {
	if !knownEvent(eventType) {
		return &RuleError{Problem: fmt.Sprintf("unknown event type %q", eventType), Fix: "List the notification rules to see the event types"}
	}
	_, err := s.db.ExecContext(ctx, sqlDeleteRule, eventType)
	return err
}

func (s *Service) Publish(ctx context.Context, event Event) (Outcome, int64, error) {
	return PublishWith(ctx, s.db, s.now(), event)
}

func PublishWith(ctx context.Context, db querier, now time.Time, event Event) (Outcome, int64, error) {
	if !knownEvent(event.Type) {
		return 0, 0, fmt.Errorf("unknown event type %q", event.Type)
	}
	if event.Severity.Rank() < 0 {
		return 0, 0, fmt.Errorf("unknown severity %q", event.Severity)
	}
	if event.DedupKey == "" {
		event.DedupKey = fmt.Sprintf("%s:%d", event.RecordKind, event.RecordID)
	}
	rule, err := loadRule(ctx, db, event.Type)
	if err != nil {
		return 0, 0, err
	}
	var lastSimilar *time.Time
	var stamp string
	err = db.QueryRowContext(ctx, sqlLastSimilar, event.Type, event.DedupKey).Scan(&stamp)
	if err == nil {
		if parsed, parseErr := time.Parse(timeLayout, stamp); parseErr == nil {
			lastSimilar = &parsed
		}
	} else if !errors.Is(err, sql.ErrNoRows) {
		return 0, 0, err
	}
	outcome := Evaluate(rule, event, now, lastSimilar)
	if outcome != Delivered {
		return outcome, 0, nil
	}
	result, err := db.ExecContext(ctx, sqlInsert, event.Type, string(event.Severity), SafeTitle(event.Title), event.RecordKind, event.RecordID, event.DedupKey, now.UTC().Format(timeLayout))
	if err != nil {
		return 0, 0, err
	}
	id, err := result.LastInsertId()
	return Delivered, id, err
}

func scanNotifications(rows *sql.Rows) ([]Notification, error) {
	defer rows.Close()
	var list []Notification
	for rows.Next() {
		var item Notification
		var severity, created string
		var read sql.NullString
		if err := rows.Scan(&item.ID, &item.EventType, &severity, &item.Title, &item.RecordKind, &item.RecordID, &created, &read); err != nil {
			return nil, err
		}
		item.Severity = theme.Severity(severity)
		parsed, err := time.Parse(timeLayout, created)
		if err != nil {
			return nil, fmt.Errorf("notification %d has an unreadable timestamp: %w", item.ID, err)
		}
		item.CreatedAt = parsed
		if read.Valid {
			if readAt, err := time.Parse(timeLayout, read.String); err == nil {
				item.ReadAt = &readAt
			}
		}
		list = append(list, item)
	}
	return list, rows.Err()
}

func (s *Service) List(ctx context.Context, filter Filter) ([]Notification, error) {
	limit := filter.Limit
	if limit <= 0 || limit > 1000 {
		limit = 200
	}
	unread := 0
	if filter.UnreadOnly {
		unread = 1
	}
	if filter.Severity != "" && filter.Severity.Rank() < 0 {
		return nil, fmt.Errorf("unknown severity %q: use info, low, medium, high or critical", filter.Severity)
	}
	rows, err := s.db.QueryContext(ctx, sqlList, unread, string(filter.Severity), string(filter.Severity), filter.EventType, filter.EventType, limit)
	if err != nil {
		return nil, err
	}
	return scanNotifications(rows)
}

func (s *Service) After(ctx context.Context, afterID int64, limit int) ([]Notification, error) {
	if limit <= 0 || limit > 500 {
		limit = 50
	}
	rows, err := s.db.QueryContext(ctx, sqlAfter, afterID, limit)
	if err != nil {
		return nil, err
	}
	return scanNotifications(rows)
}

func (s *Service) HighestID(ctx context.Context) (int64, error) {
	var id int64
	err := s.db.QueryRowContext(ctx, sqlHighestID).Scan(&id)
	return id, err
}

func (s *Service) UnreadCount(ctx context.Context) (int, error) {
	var count int
	err := s.db.QueryRowContext(ctx, sqlUnreadCount).Scan(&count)
	return count, err
}

func (s *Service) MarkRead(ctx context.Context, id int64) error {
	var found int64
	if err := s.db.QueryRowContext(ctx, sqlNotificationID, id).Scan(&found); errors.Is(err, sql.ErrNoRows) {
		return fmt.Errorf("notification %d does not exist: list the notifications to see valid identifiers", id)
	} else if err != nil {
		return err
	}
	_, err := s.db.ExecContext(ctx, sqlMarkRead, s.now().UTC().Format(timeLayout), id)
	return err
}

func (s *Service) MarkAllRead(ctx context.Context) (int64, error) {
	result, err := s.db.ExecContext(ctx, sqlMarkAllRead, s.now().UTC().Format(timeLayout))
	if err != nil {
		return 0, err
	}
	return result.RowsAffected()
}
