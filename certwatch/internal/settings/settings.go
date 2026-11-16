package settings

import (
	"context"
	"database/sql"
	"errors"
	"fmt"
	"regexp"
	"sort"
	"strconv"
	"strings"
	"time"

	"github.com/unalivin/certwatch/internal/meta"
)

type Kind string

const (
	KindEnum  Kind = "enum"
	KindBool  Kind = "bool"
	KindInt   Kind = "int"
	KindColor Kind = "color"
	KindText  Kind = "text"

	KindColumns Kind = "columns"
	KindSort    Kind = "sort"
	KindPath    Kind = "path"
	KindDays    Kind = "days"
)

type Source string

const (
	SourceDefault     Source = "default"
	SourceStored      Source = "stored"
	SourceEnvironment Source = "environment"
	SourceFlag        Source = "flag"
)

const (
	KeyMode             = "appearance.mode"
	KeyTheme            = "appearance.theme"
	KeyAccent           = "appearance.accent"
	KeyDensity          = "appearance.density"
	KeyTextScale        = "appearance.text_scale"
	KeyReducedMotion    = "appearance.reduced_motion"
	KeyMonochrome       = "appearance.monochrome"
	KeyTimeZone         = "display.time_zone"
	KeySidebarCollapsed = "window.sidebar_collapsed"
	KeyWindowWidth      = "window.width"
	KeyWindowHeight     = "window.height"
	KeyRefreshSeconds   = "data.refresh_seconds"
	KeyConfirm          = "safety.confirm_destructive"
	KeyNotifierInterval = "notifier.interval_seconds"
	KeyNotifierTitles   = "notifier.show_titles"
	KeyEndpointColumns  = "table.endpoints.columns"
	KeyEndpointSort     = "table.endpoints.sort"
	KeyCheckInterval    = "checks.interval_minutes"
	KeyCheckTimeout     = "checks.timeout_seconds"
	KeyProbeProtocols   = "checks.probe_protocols"
	KeyTrustFile        = "checks.trust_file"
	KeyHistoryKeep      = "checks.history_keep"
	KeyExpiryDays       = "alerts.expiry_days"

	sqlSelectOne = "SELECT value FROM settings WHERE key = ?"
	sqlSelectAll = "SELECT key, value FROM settings ORDER BY key"
	sqlUpsert    = "INSERT INTO settings (key, value, updated_at) VALUES (?, ?, ?) ON CONFLICT (key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at"
	sqlDelete    = "DELETE FROM settings WHERE key = ?"
)

type Definition struct {
	Key     string
	Group   string
	Label   string
	Help    string
	Kind    Kind
	Default string
	Options []string
	Pinned  []string
	Min     int
	Max     int
}

type Value struct {
	Definition
	Raw           string
	Stored        string
	HasStored     bool
	Source        Source
	OverriddenBy  string
	OverrideError string
	Unknown       bool
}

func (v Value) Bool() bool { return v.Raw == "true" }

func (v Value) Int() int {
	number, _ := strconv.Atoi(v.Raw)
	return number
}

type ValidationError struct {
	Key     string
	Problem string
	Fix     string
}

func (e *ValidationError) Error() string {
	return fmt.Sprintf("%s: %s. %s", e.Key, e.Problem, e.Fix)
}

var colorPattern = regexp.MustCompile(`^#[0-9a-fA-F]{6}$`)

var (
	EndpointColumns  = []string{"endpoint", "status", "certificate", "expires", "days", "checked", "tls", "state"}
	EndpointSortKeys = []string{"endpoint", "expires", "checked"}
)

var definitions = []Definition{
	{Key: KeyMode, Group: "Appearance", Label: "Mode", Help: "Light, dark or follow the operating system.", Kind: KindEnum, Default: "system", Options: []string{"system", "light", "dark"}},
	{Key: KeyTheme, Group: "Appearance", Label: "Color theme", Help: "Named palette for accents and surfaces.", Kind: KindEnum, Default: "slate", Options: []string{"slate", "blue", "green", "crimson", "amber", "purple", "contrast"}},
	{Key: KeyAccent, Group: "Appearance", Label: "Custom accent", Help: "Optional accent color as #RRGGBB. It is adjusted when needed to keep WCAG AA contrast. Empty uses the theme accent.", Kind: KindColor, Default: ""},
	{Key: KeyDensity, Group: "Appearance", Label: "Density", Help: "Row height and spacing of tables and lists.", Kind: KindEnum, Default: "comfortable", Options: []string{"comfortable", "compact"}},
	{Key: KeyTextScale, Group: "Appearance", Label: "Text size", Help: "Text size in percent of the default.", Kind: KindInt, Default: "100", Min: 80, Max: 200},
	{Key: KeyReducedMotion, Group: "Appearance", Label: "Reduced motion", Help: "Turn off animations and transitions.", Kind: KindBool, Default: "false"},
	{Key: KeyMonochrome, Group: "Appearance", Label: "Monochrome terminal", Help: "Draw the terminal UI without colors.", Kind: KindBool, Default: "false"},
	{Key: KeyTimeZone, Group: "Units", Label: "Time zone", Help: "Show timestamps in local time or in UTC.", Kind: KindEnum, Default: "local", Options: []string{"local", "utc"}},
	{Key: KeySidebarCollapsed, Group: "Window", Label: "Sidebar collapsed", Help: "Show the sidebar as icons only.", Kind: KindBool, Default: "false"},
	{Key: KeyWindowWidth, Group: "Window", Label: "Window width", Help: "Dashboard window width in pixels.", Kind: KindInt, Default: "1180", Min: 640, Max: 10000},
	{Key: KeyWindowHeight, Group: "Window", Label: "Window height", Help: "Dashboard window height in pixels.", Kind: KindInt, Default: "760", Min: 480, Max: 10000},
	{Key: KeyRefreshSeconds, Group: "Data", Label: "Terminal refresh interval", Help: "Seconds between automatic refreshes of the terminal UI.", Kind: KindInt, Default: "2", Min: 1, Max: 60},
	{Key: KeyConfirm, Group: "Safe defaults", Label: "Confirm destructive actions", Help: "Ask before deleting or overwriting data.", Kind: KindBool, Default: "true"},
	{Key: KeyNotifierInterval, Group: "Notifications", Label: "Notifier interval", Help: "Seconds between checks of the background notifier.", Kind: KindInt, Default: "60", Min: 15, Max: 3600},
	{Key: KeyNotifierTitles, Group: "Notifications", Label: "Show titles in system notifications", Help: "Off shows only a count. On adds the alert title, which names the host or the certificate.", Kind: KindBool, Default: "false"},
	{Key: KeyCheckInterval, Group: "Checks", Label: "Automatic check interval", Help: "Minutes between automatic checks of every enabled endpoint while the dashboard, the terminal UI or the watch command runs. Each check opens a TLS connection to the endpoint. 0 turns automatic checks off.", Kind: KindInt, Default: "0", Min: 0, Max: 10080},
	{Key: KeyCheckTimeout, Group: "Checks", Label: "Check timeout", Help: "Seconds to wait for one endpoint to answer.", Kind: KindInt, Default: "10", Min: 1, Max: 60},
	{Key: KeyProbeProtocols, Group: "Checks", Label: "Probe TLS versions", Help: "Also test which of TLS 1.0, 1.1, 1.2 and 1.3 the endpoint accepts. This opens up to three more connections per check.", Kind: KindBool, Default: "false"},
	{Key: KeyTrustFile, Group: "Checks", Label: "Additional trusted roots", Help: "Path of a PEM file with root certificates to trust in addition to the roots of the operating system, for example a private authority. Empty uses the system roots only.", Kind: KindPath, Default: ""},
	{Key: KeyHistoryKeep, Group: "Checks", Label: "Observations kept per endpoint", Help: "Older observations of an endpoint are removed after a check.", Kind: KindInt, Default: "200", Min: 2, Max: 5000},
	{Key: KeyExpiryDays, Group: "Alerts", Label: "Expiry thresholds", Help: "Days before expiry at which an alert is raised, separated by commas. Every certificate raises one alert per threshold and one when it expires.", Kind: KindDays, Default: "30,14,7,1"},
	{Key: KeyEndpointColumns, Group: "Tables", Label: "Endpoint table columns", Help: "Columns of the endpoint table, separated by commas. The endpoint column is always shown.", Kind: KindColumns, Default: "endpoint,status,certificate,expires,days,checked,tls,state", Options: EndpointColumns, Pinned: []string{"endpoint"}},
	{Key: KeyEndpointSort, Group: "Tables", Label: "Endpoint table sort", Help: "Sort column and direction of the endpoint table, for example expires:asc.", Kind: KindSort, Default: "endpoint:asc", Options: EndpointSortKeys},
}

func Definitions() []Definition {
	return append([]Definition(nil), definitions...)
}

func Lookup(key string) (Definition, bool) {
	for _, definition := range definitions {
		if definition.Key == key {
			return definition, true
		}
	}
	return Definition{}, false
}

func SplitColumns(raw string) []string {
	var columns []string
	for _, part := range strings.Split(raw, ",") {
		if part = strings.TrimSpace(part); part != "" {
			columns = append(columns, part)
		}
	}
	return columns
}

func SplitSort(raw string) (string, bool) {
	key, direction, _ := strings.Cut(raw, ":")
	return key, direction == "desc"
}

func JoinSort(key string, descending bool) string {
	if descending {
		return key + ":desc"
	}
	return key + ":asc"
}

func (v Value) Columns() []string { return SplitColumns(v.Raw) }

func (v Value) Shows(column string) bool {
	for _, shown := range v.Columns() {
		if shown == column {
			return true
		}
	}
	return false
}

func (v Value) Sort() (string, bool) { return SplitSort(v.Raw) }

func ParseDays(text string) ([]int, error) {
	seen := map[int]bool{}
	list := []int{}
	for _, part := range strings.Split(text, ",") {
		part = strings.TrimSpace(part)
		if part == "" {
			continue
		}
		days, err := strconv.Atoi(part)
		if err != nil || days < 1 || days > 3650 {
			return nil, fmt.Errorf("%q is not a number of days", part)
		}
		if !seen[days] {
			seen[days] = true
			list = append(list, days)
		}
	}
	if len(list) > 12 {
		return nil, errors.New("more than 12 thresholds")
	}
	sort.Sort(sort.Reverse(sort.IntSlice(list)))
	return list, nil
}

func FormatDays(list []int) string {
	parts := make([]string, len(list))
	for i, days := range list {
		parts[i] = strconv.Itoa(days)
	}
	return strings.Join(parts, ",")
}

func (v Value) Days() []int {
	days, _ := ParseDays(v.Raw)
	return days
}

func EnvName(key string) string {
	return meta.EnvName(key)
}

func Normalize(definition Definition, raw string) (string, error) {
	value := strings.TrimSpace(raw)
	fail := func(problem, fix string) (string, error) {
		return "", &ValidationError{Key: definition.Key, Problem: problem, Fix: fix}
	}
	switch definition.Kind {
	case KindEnum:
		for _, option := range definition.Options {
			if strings.EqualFold(option, value) {
				return option, nil
			}
		}
		return fail(fmt.Sprintf("%q is not one of the allowed values", raw), "Use one of: "+strings.Join(definition.Options, ", "))
	case KindBool:
		switch strings.ToLower(value) {
		case "true", "on", "yes", "1":
			return "true", nil
		case "false", "off", "no", "0":
			return "false", nil
		}
		return fail(fmt.Sprintf("%q is not a yes or no value", raw), "Use true or false")
	case KindInt:
		number, err := strconv.Atoi(value)
		if err != nil {
			return fail(fmt.Sprintf("%q is not a whole number", raw), fmt.Sprintf("Use a number from %d to %d", definition.Min, definition.Max))
		}
		if number < definition.Min || number > definition.Max {
			return fail(fmt.Sprintf("%d is outside the allowed range", number), fmt.Sprintf("Use a number from %d to %d", definition.Min, definition.Max))
		}
		return strconv.Itoa(number), nil
	case KindColor:
		if value == "" {
			return "", nil
		}
		if !colorPattern.MatchString(value) {
			return fail(fmt.Sprintf("%q is not a color", raw), "Use the form #RRGGBB, for example #1257C4, or leave it empty")
		}
		return strings.ToUpper(value), nil
	case KindPath:
		if len(value) > 1024 || strings.ContainsAny(value, "\x00\n\r") {
			return fail("the path is too long or has control characters", "Enter the path of one file or leave it empty")
		}
		return value, nil
	case KindDays:
		days, err := ParseDays(value)
		if err != nil {
			return fail(err.Error(), "Use whole days from 1 to 3650 separated by commas, for example 30,14,7,1")
		}
		return FormatDays(days), nil
	case KindColumns:
		if len(value) > 400 {
			return fail("the list is too long", "Use some of: "+strings.Join(definition.Options, ", "))
		}
		chosen := map[string]bool{}
		for _, pinned := range definition.Pinned {
			chosen[pinned] = true
		}
		for _, column := range SplitColumns(strings.ToLower(value)) {
			known := false
			for _, option := range definition.Options {
				known = known || option == column
			}
			if !known {
				return fail(fmt.Sprintf("%q is not a column of this table", column), "Use some of: "+strings.Join(definition.Options, ", "))
			}
			chosen[column] = true
		}
		var ordered []string
		for _, option := range definition.Options {
			if chosen[option] {
				ordered = append(ordered, option)
			}
		}
		return strings.Join(ordered, ","), nil
	case KindSort:
		key, direction, found := strings.Cut(strings.ToLower(value), ":")
		key, direction = strings.TrimSpace(key), strings.TrimSpace(direction)
		if !found {
			direction = "asc"
		}
		fix := "Use a column (" + strings.Join(definition.Options, ", ") + ") followed by :asc or :desc, for example " + definition.Default
		known := false
		for _, option := range definition.Options {
			known = known || option == key
		}
		if !known {
			return fail(fmt.Sprintf("%q is not a sortable column", key), fix)
		}
		if direction != "asc" && direction != "desc" {
			return fail(fmt.Sprintf("%q is not a direction", direction), fix)
		}
		return key + ":" + direction, nil
	case KindText:
		return value, nil
	}
	return fail("the setting has an unknown type", "Report this as a bug")
}

type Service struct {
	db     *sql.DB
	now    func() time.Time
	getenv func(string) string
	flags  map[string]string
}

func New(db *sql.DB, now func() time.Time, getenv func(string) string, flags map[string]string) *Service {
	if getenv == nil {
		getenv = func(string) string { return "" }
	}
	copied := make(map[string]string, len(flags))
	for key, value := range flags {
		copied[key] = value
	}
	return &Service{db: db, now: now, getenv: getenv, flags: copied}
}

func (s *Service) resolve(definition Definition, stored string, hasStored bool) Value {
	value := Value{Definition: definition, Raw: definition.Default, Source: SourceDefault, Stored: stored, HasStored: hasStored}
	if hasStored {
		if normalized, err := Normalize(definition, stored); err == nil {
			value.Raw, value.Source = normalized, SourceStored
		} else {
			value.OverrideError = "stored value is invalid and the default is used: " + err.Error()
		}
	}
	if raw := s.getenv(EnvName(definition.Key)); raw != "" {
		if normalized, err := Normalize(definition, raw); err == nil {
			value.Raw, value.Source, value.OverriddenBy = normalized, SourceEnvironment, "environment variable "+EnvName(definition.Key)
		} else {
			value.OverrideError = "environment variable " + EnvName(definition.Key) + " is ignored: " + err.Error()
		}
	}
	if raw, ok := s.flags[definition.Key]; ok {
		if normalized, err := Normalize(definition, raw); err == nil {
			value.Raw, value.Source, value.OverriddenBy = normalized, SourceFlag, "command-line flag --set "+definition.Key
		} else {
			value.OverrideError = "command-line flag --set " + definition.Key + " is ignored: " + err.Error()
		}
	}
	return value
}

func (s *Service) Get(ctx context.Context, key string) (Value, error) {
	definition, ok := Lookup(key)
	if !ok {
		return Value{}, &ValidationError{Key: key, Problem: "unknown setting", Fix: "List the settings to see the available keys"}
	}
	var stored string
	err := s.db.QueryRowContext(ctx, sqlSelectOne, key).Scan(&stored)
	if errors.Is(err, sql.ErrNoRows) {
		return s.resolve(definition, "", false), nil
	}
	if err != nil {
		return Value{}, err
	}
	return s.resolve(definition, stored, true), nil
}

func (s *Service) List(ctx context.Context) ([]Value, error) {
	rows, err := s.db.QueryContext(ctx, sqlSelectAll)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	stored := map[string]string{}
	for rows.Next() {
		var key, value string
		if err := rows.Scan(&key, &value); err != nil {
			return nil, err
		}
		stored[key] = value
	}
	if err := rows.Err(); err != nil {
		return nil, err
	}
	values := make([]Value, 0, len(definitions)+len(stored))
	for _, definition := range definitions {
		raw, ok := stored[definition.Key]
		values = append(values, s.resolve(definition, raw, ok))
		delete(stored, definition.Key)
	}
	extra := make([]string, 0, len(stored))
	for key := range stored {
		extra = append(extra, key)
	}
	sort.Strings(extra)
	for _, key := range extra {
		if definition, ok := Lookup(key); ok {
			values = append(values, s.resolve(definition, stored[key], true))
			continue
		}
		values = append(values, Value{Definition: Definition{Key: key, Group: "Unknown", Label: key, Help: "Stored by another version of the application. It is kept and not used.", Kind: KindText}, Raw: stored[key], Stored: stored[key], HasStored: true, Source: SourceStored, Unknown: true})
	}
	return values, nil
}

func (s *Service) Set(ctx context.Context, key, raw string) (Value, error) {
	definition, ok := Lookup(key)
	if !ok {
		return Value{}, &ValidationError{Key: key, Problem: "unknown setting", Fix: "List the settings to see the available keys"}
	}
	normalized, err := Normalize(definition, raw)
	if err != nil {
		return Value{}, err
	}
	if _, err := s.db.ExecContext(ctx, sqlUpsert, key, normalized, s.now().UTC().Format(time.RFC3339)); err != nil {
		return Value{}, err
	}
	return s.resolve(definition, normalized, true), nil
}

func (s *Service) Reset(ctx context.Context, key string) (Value, error) {
	definition, ok := Lookup(key)
	if !ok {
		return Value{}, &ValidationError{Key: key, Problem: "unknown setting", Fix: "List the settings to see the available keys"}
	}
	if _, err := s.db.ExecContext(ctx, sqlDelete, key); err != nil {
		return Value{}, err
	}
	return s.resolve(definition, "", false), nil
}

func (s *Service) Effective(ctx context.Context) (map[string]Value, error) {
	values, err := s.List(ctx)
	if err != nil {
		return nil, err
	}
	effective := make(map[string]Value, len(values))
	for _, value := range values {
		effective[value.Key] = value
	}
	return effective, nil
}
