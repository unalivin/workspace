package notify

import (
	"context"
	"errors"
	"strings"
	"testing"
	"time"

	"github.com/NexzaDev/templateapp/internal/testkit"
	"github.com/NexzaDev/templateapp/internal/theme"
)

func at(clock string) time.Time {
	parsed, err := time.Parse("15:04", clock)
	if err != nil {
		panic(err)
	}
	return time.Date(2026, time.March, 2, parsed.Hour(), parsed.Minute(), 0, 0, time.UTC)
}

func TestInQuietHours(t *testing.T) {
	cases := []struct {
		name, start, end, now string
		want                  bool
	}{
		{"no quiet hours", "", "", "03:00", false},
		{"same day inside", "09:00", "17:00", "12:00", true},
		{"same day at start", "09:00", "17:00", "09:00", true},
		{"same day at end is outside", "09:00", "17:00", "17:00", false},
		{"same day before", "09:00", "17:00", "08:59", false},
		{"over midnight late evening", "22:00", "07:00", "23:30", true},
		{"over midnight early morning", "22:00", "07:00", "06:59", true},
		{"over midnight at end is outside", "22:00", "07:00", "07:00", false},
		{"over midnight midday", "22:00", "07:00", "12:00", false},
		{"over midnight at midnight", "22:00", "07:00", "00:00", true},
		{"equal start and end means none", "08:00", "08:00", "08:00", false},
		{"last minute of the day", "23:59", "00:01", "23:59", true},
		{"first minute after the window", "23:59", "00:01", "00:01", false},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			rule := Rule{QuietStart: tc.start, QuietEnd: tc.end}
			if got := InQuietHours(rule, at(tc.now)); got != tc.want {
				t.Fatalf("got %v, want %v", got, tc.want)
			}
		})
	}
}

func TestEvaluate(t *testing.T) {
	now := at("12:00")
	ago := func(d time.Duration) *time.Time {
		stamp := now.Add(-d)
		return &stamp
	}
	base := Rule{EventType: EventRecordUrgent, Enabled: true, MinSeverity: theme.SeverityMedium, DedupMinutes: 60}
	with := func(change func(*Rule)) Rule {
		rule := base
		change(&rule)
		return rule
	}
	cases := []struct {
		name     string
		rule     Rule
		severity theme.Severity
		last     *time.Time
		want     Outcome
	}{
		{"delivered at threshold", base, theme.SeverityMedium, nil, Delivered},
		{"delivered above threshold", base, theme.SeverityCritical, nil, Delivered},
		{"below threshold", base, theme.SeverityLow, nil, SuppressedSeverity},
		{"disabled wins over everything", with(func(r *Rule) { r.Enabled = false }), theme.SeverityCritical, nil, SuppressedDisabled},
		{"quiet hours", with(func(r *Rule) { r.QuietStart, r.QuietEnd = "11:00", "13:00" }), theme.SeverityHigh, nil, SuppressedQuietHours},
		{"severity checked before quiet hours", with(func(r *Rule) { r.QuietStart, r.QuietEnd = "11:00", "13:00" }), theme.SeverityInfo, nil, SuppressedSeverity},
		{"duplicate inside window", base, theme.SeverityHigh, ago(59 * time.Minute), SuppressedDuplicate},
		{"duplicate exactly at window edge is delivered", base, theme.SeverityHigh, ago(60 * time.Minute), Delivered},
		{"duplicate outside window", base, theme.SeverityHigh, ago(2 * time.Hour), Delivered},
		{"window of zero disables de-duplication", with(func(r *Rule) { r.DedupMinutes = 0 }), theme.SeverityHigh, ago(time.Second), Delivered},
		{"clock moved backwards does not suppress forever", base, theme.SeverityHigh, ago(-3 * time.Hour), Delivered},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			got := Evaluate(tc.rule, Event{Type: EventRecordUrgent, Severity: tc.severity}, now, tc.last)
			if got != tc.want {
				t.Fatalf("got %v, want %v", got, tc.want)
			}
			if got.String() == "" {
				t.Fatal("outcome has no text")
			}
		})
	}
}

func TestRuleValidate(t *testing.T) {
	valid := DefaultRule(EventRecordUrgent)
	if err := valid.Validate(); err != nil {
		t.Fatalf("default rule is invalid: %v", err)
	}
	cases := []struct {
		name   string
		change func(*Rule)
		text   string
	}{
		{"unknown event", func(r *Rule) { r.EventType = "nope" }, "unknown event type"},
		{"unknown severity", func(r *Rule) { r.MinSeverity = "urgent" }, "unknown severity"},
		{"only start", func(r *Rule) { r.QuietStart = "22:00" }, "both a start and an end"},
		{"only end", func(r *Rule) { r.QuietEnd = "07:00" }, "both a start and an end"},
		{"bad hour", func(r *Rule) { r.QuietStart, r.QuietEnd = "24:00", "07:00" }, "not a time of day"},
		{"bad minute", func(r *Rule) { r.QuietStart, r.QuietEnd = "22:00", "07:60" }, "not a time of day"},
		{"not padded", func(r *Rule) { r.QuietStart, r.QuietEnd = "9:00", "17:00" }, "not a time of day"},
		{"injection attempt", func(r *Rule) { r.QuietStart, r.QuietEnd = "22:00'; DROP TABLE x;--", "07:00" }, "not a time of day"},
		{"negative window", func(r *Rule) { r.DedupMinutes = -1 }, "out of range"},
		{"huge window", func(r *Rule) { r.DedupMinutes = 10081 }, "out of range"},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			rule := valid
			tc.change(&rule)
			err := rule.Validate()
			var ruleErr *RuleError
			if !errors.As(err, &ruleErr) || !strings.Contains(err.Error(), tc.text) || ruleErr.Fix == "" {
				t.Fatalf("got %v, want a rule error with %q and a fix", err, tc.text)
			}
		})
	}
}

func TestSafeTitle(t *testing.T) {
	cases := []struct{ name, in, want string }{
		{"plain", "Record needs attention: edge router", "Record needs attention: edge router"},
		{"control characters", "line one\nline two\x1b[31m\ttab", "line one line two [31m tab"},
		{"empty", "  \n ", "Notification"},
		{"token is redacted", "leaked ghp_abcdefghijklmnopqrstuvwxyz0123456789 here", "leaked [redacted] here"},
		{"base64 blob is redacted", "key QUJDREVGR0hJSktMTU5PUFFSU1RVVldYWVo= end", "key [redacted] end"},
		{"short identifiers stay", "host web-01.internal is down", "host web-01.internal is down"},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			if got := SafeTitle(tc.in); got != tc.want {
				t.Fatalf("got %q, want %q", got, tc.want)
			}
		})
	}
	long := SafeTitle(strings.Repeat("ab cd ", 100))
	if length := len([]rune(long)); length != maxTitleLength || !strings.HasSuffix(long, "…") {
		t.Fatalf("long title has %d runes: %q", length, long)
	}
	unicodeTitle := SafeTitle(strings.Repeat("é ", 200))
	if length := len([]rune(unicodeTitle)); length > maxTitleLength {
		t.Fatalf("unicode title has %d runes", length)
	}
}

func FuzzSafeTitle(f *testing.F) {
	for _, seed := range []string{"", "plain", "a\x00b", strings.Repeat("x", 500), "ünï cödé \u202e reversed", "\xff\xfe"} {
		f.Add(seed)
	}
	f.Fuzz(func(t *testing.T, input string) {
		got := SafeTitle(input)
		if got == "" || len([]rune(got)) > maxTitleLength {
			t.Fatalf("bad length for %q: %q", input, got)
		}
		for _, r := range got {
			if r < 0x20 || r == 0x7f {
				t.Fatalf("control character survived in %q", got)
			}
		}
		if secretLike.MatchString(strings.ReplaceAll(got, "[redacted]", " ")) {
			t.Fatalf("secret-like run survived in %q", got)
		}
	})
}

func newService(t *testing.T) (*Service, *testkit.Clock) {
	t.Helper()
	clock := testkit.NewClock()
	return New(testkit.Open(t).DB(), clock.Now), clock
}

func TestRulesDefaultsAndPersistence(t *testing.T) {
	ctx := context.Background()
	service, _ := newService(t)
	rules, err := service.Rules(ctx)
	if err != nil || len(rules) != len(EventTypes()) {
		t.Fatalf("rules: %v, %v", rules, err)
	}
	for _, rule := range rules {
		if !rule.Enabled || rule.Customized || rule.DedupMinutes != 60 || rule.MinSeverity != theme.SeverityInfo {
			t.Fatalf("unexpected default rule %+v", rule)
		}
	}
	changed := Rule{EventType: EventRecordUrgent, Enabled: false, MinSeverity: theme.SeverityHigh, QuietStart: "22:00", QuietEnd: "07:00", DedupMinutes: 5}
	if err := service.SetRule(ctx, changed); err != nil {
		t.Fatal(err)
	}
	changed.Enabled = true
	if err := service.SetRule(ctx, changed); err != nil {
		t.Fatal(err)
	}
	got, err := service.Rule(ctx, EventRecordUrgent)
	changed.Customized = true
	if err != nil || got != changed {
		t.Fatalf("got %+v, %v; want %+v", got, err, changed)
	}
	if err := service.SetRule(ctx, Rule{EventType: "nope", MinSeverity: theme.SeverityInfo}); err == nil {
		t.Fatal("an unknown event type was stored")
	}
	if _, err := service.Rule(ctx, "nope"); err == nil {
		t.Fatal("an unknown event type was read")
	}
	if err := service.ResetRule(ctx, "nope"); err == nil {
		t.Fatal("an unknown event type was reset")
	}
	if err := service.ResetRule(ctx, EventRecordUrgent); err != nil {
		t.Fatal(err)
	}
	if got, _ := service.Rule(ctx, EventRecordUrgent); got != DefaultRule(EventRecordUrgent) {
		t.Fatalf("reset left %+v", got)
	}
}

func TestPublishAppliesRulesAndDeduplicates(t *testing.T) {
	ctx := context.Background()
	service, clock := newService(t)
	event := Event{Type: EventRecordUrgent, Severity: theme.SeverityHigh, Title: "Record needs attention: core switch", RecordKind: "record", RecordID: 7}
	outcome, id, err := service.Publish(ctx, event)
	if err != nil || outcome != Delivered || id == 0 {
		t.Fatalf("first publish: %v, %d, %v", outcome, id, err)
	}
	clock.Advance(10 * time.Minute)
	if outcome, id, err := service.Publish(ctx, event); err != nil || outcome != SuppressedDuplicate || id != 0 {
		t.Fatalf("duplicate publish: %v, %d, %v", outcome, id, err)
	}
	other := event
	other.RecordID = 8
	if outcome, _, err := service.Publish(ctx, other); err != nil || outcome != Delivered {
		t.Fatalf("another record was treated as a duplicate: %v, %v", outcome, err)
	}
	clock.Advance(51 * time.Minute)
	if outcome, _, err := service.Publish(ctx, event); err != nil || outcome != Delivered {
		t.Fatalf("publish after the window: %v, %v", outcome, err)
	}
	if err := service.SetRule(ctx, Rule{EventType: EventRecordUrgent, Enabled: true, MinSeverity: theme.SeverityCritical, DedupMinutes: 0}); err != nil {
		t.Fatal(err)
	}
	if outcome, _, _ := service.Publish(ctx, event); outcome != SuppressedSeverity {
		t.Fatalf("threshold not applied: %v", outcome)
	}
	quiet := Rule{EventType: EventRecordUrgent, Enabled: true, MinSeverity: theme.SeverityInfo, QuietStart: "09:00", QuietEnd: "12:00", DedupMinutes: 0}
	if err := service.SetRule(ctx, quiet); err != nil {
		t.Fatal(err)
	}
	if outcome, _, _ := service.Publish(ctx, event); outcome != SuppressedQuietHours {
		t.Fatalf("quiet hours not applied at %s: %v", clock.Now().Format("15:04"), outcome)
	}
	quiet.Enabled = false
	if err := service.SetRule(ctx, quiet); err != nil {
		t.Fatal(err)
	}
	if outcome, _, _ := service.Publish(ctx, event); outcome != SuppressedDisabled {
		t.Fatalf("disabled rule not applied: %v", outcome)
	}
	if _, _, err := service.Publish(ctx, Event{Type: "nope", Severity: theme.SeverityInfo}); err == nil {
		t.Fatal("an unknown event type was published")
	}
	if _, _, err := service.Publish(ctx, Event{Type: EventRecordUrgent, Severity: "urgent"}); err == nil {
		t.Fatal("an unknown severity was published")
	}
	count, err := service.UnreadCount(ctx)
	if err != nil || count != 3 {
		t.Fatalf("unread count = %d, %v; want 3", count, err)
	}
}

func TestPublishStoresOnlySafeTitles(t *testing.T) {
	ctx := context.Background()
	service, _ := newService(t)
	_, _, err := service.Publish(ctx, Event{Type: EventRecordUrgent, Severity: theme.SeverityHigh, Title: "token ghp_abcdefghijklmnopqrstuvwxyz0123456789\nsecond line", RecordKind: "record", RecordID: 1})
	if err != nil {
		t.Fatal(err)
	}
	list, err := service.List(ctx, Filter{})
	if err != nil || len(list) != 1 {
		t.Fatalf("list: %v, %v", list, err)
	}
	if strings.Contains(list[0].Title, "ghp_") || strings.ContainsAny(list[0].Title, "\n\r") {
		t.Fatalf("unsafe title stored: %q", list[0].Title)
	}
}

func TestListFiltersAndMarkRead(t *testing.T) {
	ctx := context.Background()
	service, clock := newService(t)
	publish := func(eventType string, severity theme.Severity, record int64) int64 {
		t.Helper()
		clock.Advance(time.Minute)
		outcome, id, err := service.Publish(ctx, Event{Type: eventType, Severity: severity, Title: "item", RecordKind: "record", RecordID: record})
		if err != nil || outcome != Delivered {
			t.Fatalf("publish: %v, %v", outcome, err)
		}
		return id
	}
	first := publish(EventRecordUrgent, theme.SeverityHigh, 1)
	second := publish(EventRecordUrgent, theme.SeverityCritical, 2)
	third := publish(EventRecordCompleted, theme.SeverityInfo, 3)
	all, err := service.List(ctx, Filter{})
	if err != nil || len(all) != 3 || all[0].ID != third || all[2].ID != first {
		t.Fatalf("newest first expected: %+v, %v", all, err)
	}
	if all[0].RecordKind != "record" || all[0].RecordID != 3 || !all[0].Unread() || !all[0].CreatedAt.Equal(clock.Now()) {
		t.Fatalf("unexpected notification %+v", all[0])
	}
	critical, _ := service.List(ctx, Filter{Severity: theme.SeverityCritical})
	if len(critical) != 1 || critical[0].ID != second {
		t.Fatalf("severity filter: %+v", critical)
	}
	completed, _ := service.List(ctx, Filter{EventType: EventRecordCompleted})
	if len(completed) != 1 || completed[0].ID != third {
		t.Fatalf("event filter: %+v", completed)
	}
	if _, err := service.List(ctx, Filter{Severity: "urgent"}); err == nil {
		t.Fatal("an unknown severity filter was accepted")
	}
	if limited, _ := service.List(ctx, Filter{Limit: 2}); len(limited) != 2 {
		t.Fatalf("limit ignored: %d", len(limited))
	}
	clock.Advance(time.Minute)
	if err := service.MarkRead(ctx, second); err != nil {
		t.Fatal(err)
	}
	readAt := clock.Now()
	clock.Advance(time.Hour)
	if err := service.MarkRead(ctx, second); err != nil {
		t.Fatal(err)
	}
	if err := service.MarkRead(ctx, 9999); err == nil || !strings.Contains(err.Error(), "does not exist") {
		t.Fatalf("missing notification: %v", err)
	}
	unread, _ := service.List(ctx, Filter{UnreadOnly: true})
	if len(unread) != 2 {
		t.Fatalf("unread filter: %+v", unread)
	}
	again, _ := service.List(ctx, Filter{Severity: theme.SeverityCritical})
	if again[0].ReadAt == nil || !again[0].ReadAt.Equal(readAt) {
		t.Fatalf("read time changed on the second mark: %+v", again[0].ReadAt)
	}
	if count, _ := service.UnreadCount(ctx); count != 2 {
		t.Fatalf("unread count = %d", count)
	}
	changed, err := service.MarkAllRead(ctx)
	if err != nil || changed != 2 {
		t.Fatalf("mark all: %d, %v", changed, err)
	}
	if count, _ := service.UnreadCount(ctx); count != 0 {
		t.Fatalf("unread count after mark all = %d", count)
	}
	after, err := service.After(ctx, first, 0)
	if err != nil || len(after) != 2 || after[0].ID != second {
		t.Fatalf("after cursor: %+v, %v", after, err)
	}
	if highest, err := service.HighestID(ctx); err != nil || highest != third {
		t.Fatalf("highest = %d, %v", highest, err)
	}
}
