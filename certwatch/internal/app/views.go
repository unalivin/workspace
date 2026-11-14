package app

import (
	"bytes"
	"context"
	"errors"
	"fmt"
	"sort"
	"strconv"
	"strings"
	"time"

	"github.com/NexzaDev/certwatch/internal/inventory"
	"github.com/NexzaDev/certwatch/internal/notify"
	"github.com/NexzaDev/certwatch/internal/settings"
	"github.com/NexzaDev/certwatch/internal/theme"
	"github.com/NexzaDev/certwatch/internal/tlscheck"
	"github.com/NexzaDev/certwatch/internal/words"
)

const (
	ViewCertificates  = "certificates"
	ViewEndpoints     = "endpoints"
	ViewTimeline      = "timeline"
	ViewChains        = "chains"
	ViewConfiguration = "configuration"
	ViewReports       = "reports"

	FieldText   = "text"
	FieldNumber = "number"
	FieldBool   = "bool"
	FieldChoice = "choice"
	FieldPath   = "path"

	barWidth     = 24
	historyShown = 20
)

type Choice struct {
	ID    string
	Label string
}

type Field struct {
	ID       string
	Label    string
	Help     string
	Kind     string
	Options  []Choice
	Default  string
	Required bool
}

type Action struct {
	ID      string
	Label   string
	Help    string
	Key     string
	Row     bool
	Danger  bool
	Network bool
	Preview bool
	Fields  []Field
}

type View struct {
	ID          string
	Title       string
	Help        string
	FilterLabel string
	Filters     []Choice
	Actions     []Action
}

type Column struct {
	ID    string
	Title string
	Width int
	Right bool
}

type Row struct {
	ID       int64
	Cells    []string
	Severity theme.Severity
	Demo     bool
	Muted    bool
}

type Mark struct {
	Label    string
	Days     int
	Severity theme.Severity
}

type Table struct {
	Columns []Column
	Rows    []Row
	Empty   string
	Footer  string
	Window  int
	Marks   []Mark
}

type Section struct {
	Title string
	Facts [][2]string
	Table *Table
	Text  string
}

type Detail struct {
	Title    string
	Subtitle string
	Severity theme.Severity
	Sections []Section
}

type ViewQuery struct {
	Search string
	Filter string
}

type Request struct {
	View   string
	Action string
	ID     int64
	Values map[string]string
}

func (r Request) value(id string) string {
	return strings.TrimSpace(r.Values[id])
}

func (r Request) flag(id string) bool {
	switch strings.ToLower(r.value(id)) {
	case "true", "yes", "on", "1":
		return true
	}
	return false
}

type Plan struct {
	Statement string
	Network   []string
	Lines     []string
	Confirm   bool
	Danger    bool
}

type Outcome struct {
	Message string
	Lines   []string
	Select  int64
}

type UsageError struct{ Message string }

func (e *UsageError) Error() string { return e.Message }

var authorizedField = Field{ID: "authorized", Label: "I am authorized to test this host", Help: "Confirm that you may test this host. CertWatch connects only to confirmed hosts; testing a host without permission can break rules or laws.", Kind: FieldBool, Default: "false", Required: true}

var endpointFields = []Field{
	{ID: "host", Label: "Host", Help: "Host name or IP address, optionally with a port, for example www.example.org or mail.example.org:993.", Kind: FieldText, Required: true},
	{ID: "port", Label: "Port", Help: "TCP port of the TLS service.", Kind: FieldNumber, Default: "443"},
	{ID: "server-name", Label: "Server name", Help: "Name to ask for and to verify when it differs from the host, for example when the host is an IP address. Empty uses the host.", Kind: FieldText},
	{ID: "note", Label: "Note", Help: "Free text for your own reference.", Kind: FieldText},
}

func choices(ids []string, labels map[string]string) []Choice {
	list := make([]Choice, len(ids))
	for i, id := range ids {
		list[i] = Choice{ID: id, Label: labels[id]}
	}
	return list
}

var views = []View{
	{ID: ViewCertificates, Title: "Certificates", Help: "Every certificate that an enabled endpoint presents or that was imported from a file.", FilterLabel: "Show",
		Filters: choices(inventory.CertificateFilters, map[string]string{"tracked": "Tracked", "expiring-7": "Expiring in 7 days", "expiring-30": "Expiring in 30 days", "expiring-90": "Expiring in 90 days", "expired": "Expired", "authorities": "Authorities", "all": "All stored"}),
		Actions: []Action{
			{ID: "import", Label: "Import PEM file", Help: "Track the certificates of a PEM file. Private keys in the file are ignored and never stored.", Key: "i", Preview: true, Fields: []Field{{ID: "file", Label: "PEM file", Help: "Path of a file with one or more PEM certificates.", Kind: FieldPath, Required: true}}},
			{ID: "export", Label: "Save as PEM", Help: "Write the selected certificate to a PEM file.", Key: "o", Row: true, Fields: []Field{{ID: "file", Label: "File", Help: "Path of the file to write.", Kind: FieldPath, Required: true}, {ID: "overwrite", Label: "Overwrite an existing file", Kind: FieldBool, Default: "false"}}},
			{ID: "stop-tracking", Label: "Stop tracking", Help: "Remove an imported certificate from the inventory.", Key: "d", Row: true, Danger: true},
		}},
	{ID: ViewEndpoints, Title: "Endpoints", Help: "The hosts you monitor and what the last observation found.", FilterLabel: "Show",
		Filters: choices(inventory.EndpointFilters, map[string]string{"all": "All", "enabled": "Enabled", "disabled": "Disabled", "never": "Not checked yet", "unreachable": "Failed checks", "chain": "Chain failures", "hostname": "Name mismatches", "changed": "Changed certificate"}),
		Actions: []Action{
			{ID: "add", Label: "Add endpoint", Help: "Monitor a host you are authorized to test.", Key: "n", Fields: append(append([]Field{}, endpointFields...), authorizedField)},
			{ID: "check", Label: "Check now", Help: "Connect to the selected endpoint and record what it presents.", Key: "c", Row: true, Network: true},
			{ID: "check-all", Label: "Check all", Help: "Connect to every enabled endpoint and record what it presents.", Key: "C", Network: true},
			{ID: "edit", Label: "Edit", Help: "Change the selected endpoint.", Key: "e", Row: true, Fields: append(append([]Field{}, endpointFields...), Field{ID: "enabled", Label: "Enabled", Help: "Disabled endpoints are left out of checks, counts and alerts.", Kind: FieldBool, Default: "true"}, Field{ID: "authorized", Label: "I am authorized to test this host", Help: "Needed only when the host or the port changes.", Kind: FieldBool, Default: "false"})},
			{ID: "enable", Label: "Enable", Help: "Include the selected endpoint in checks, counts and alerts.", Key: "x", Row: true},
			{ID: "disable", Label: "Disable", Help: "Leave the selected endpoint out of checks, counts and alerts.", Key: "X", Row: true},
			{ID: "remove", Label: "Remove", Help: "Remove the selected endpoint with its observation history.", Key: "d", Row: true, Danger: true},
			{ID: "import", Label: "Import list", Help: "Add endpoints from a CSV or JSON file with the columns host, port, server_name, note and enabled.", Key: "i", Preview: true, Fields: []Field{{ID: "file", Label: "File", Help: "Path of a .csv or .json file.", Kind: FieldPath, Required: true}, authorizedField}},
			{ID: "export", Label: "Export list", Help: "Write every endpoint to a CSV or JSON file.", Key: "o", Fields: []Field{{ID: "file", Label: "File", Help: "Path ending in .csv or .json.", Kind: FieldPath, Required: true}, {ID: "overwrite", Label: "Overwrite an existing file", Kind: FieldBool, Default: "false"}}},
		}},
	{ID: ViewTimeline, Title: "Expiry Timeline", Help: "Tracked certificates in the order in which they expire. The zoom sets how far ahead the timeline looks.", FilterLabel: "Zoom",
		Filters: []Choice{{"90", "Next 90 days"}, {"30", "Next 30 days"}, {"180", "Next 180 days"}, {"365", "Next year"}, {"all", "Everything"}}},
	{ID: ViewChains, Title: "Chains", Help: "Whether the chain of every enabled endpoint leads to a trusted root, and who issued what.", FilterLabel: "Show",
		Filters: []Choice{{"all", "All"}, {"invalid", "Failed validation"}, {"valid", "Valid"}}},
	{ID: ViewConfiguration, Title: "Configuration", Help: "The TLS version and cipher suite each endpoint negotiated, the versions it accepts, and what should be improved.", FilterLabel: "Show",
		Filters: []Choice{{"all", "All"}, {"findings", "With findings"}},
		Actions: []Action{
			{ID: "probe", Label: "Probe TLS versions", Help: "Connect to the selected endpoint once per TLS version to see which versions it accepts.", Key: "c", Row: true, Network: true},
		}},
	{ID: ViewReports, Title: "Reports", Help: "Reports generated from the inventory. They are stored in the database and can be saved as files.", FilterLabel: "Show",
		Filters: choices(inventory.ReportFilters, map[string]string{"all": "All", inventory.ReportInventory: "Inventory", inventory.ReportExpiry: "Expiry", inventory.ReportFindings: "Findings"}),
		Actions: []Action{
			{ID: "generate", Label: "Generate report", Help: "Create a report from the current data.", Key: "n", Fields: []Field{
				{ID: "kind", Label: "Content", Kind: FieldChoice, Default: inventory.ReportInventory, Options: []Choice{{inventory.ReportInventory, "Certificate inventory"}, {inventory.ReportExpiry, "Certificates expiring within 90 days"}, {inventory.ReportFindings, "Validation and configuration findings"}}},
				{ID: "format", Label: "Format", Kind: FieldChoice, Default: inventory.FormatMarkdown, Options: []Choice{{inventory.FormatMarkdown, "Markdown"}, {inventory.FormatCSV, "CSV"}, {inventory.FormatJSON, "JSON"}}}}},
			{ID: "export", Label: "Save as file", Help: "Write the selected report to a file.", Key: "o", Row: true, Fields: []Field{{ID: "file", Label: "File", Help: "Path of the file to write.", Kind: FieldPath, Required: true}, {ID: "overwrite", Label: "Overwrite an existing file", Kind: FieldBool, Default: "false"}}},
			{ID: "remove", Label: "Remove", Help: "Remove the selected report from the database. Saved files stay.", Key: "d", Row: true, Danger: true},
		}},
}

func Views() []View {
	return append([]View(nil), views...)
}

func LookupView(id string) (View, bool) {
	for _, view := range views {
		if view.ID == id {
			return view, true
		}
	}
	return View{}, false
}

func (v View) Action(id string) (Action, bool) {
	for _, action := range v.Actions {
		if action.ID == id {
			return action, true
		}
	}
	return Action{}, false
}

func DaysText(notAfter, now time.Time) string {
	if notAfter.IsZero() {
		return ""
	}
	if !notAfter.After(now) {
		switch elapsed := int(now.Sub(notAfter) / (24 * time.Hour)); elapsed {
		case 0:
			return "expired"
		case 1:
			return "expired 1 day ago"
		default:
			return fmt.Sprintf("expired %d days ago", elapsed)
		}
	}
	days := tlscheck.DaysLeft(notAfter, now)
	switch {
	case days == 0:
		return "today"
	case days == 1:
		return "1 day"
	}
	return fmt.Sprintf("%d days", days)
}

func day(moment time.Time) string {
	if moment.IsZero() {
		return ""
	}
	return moment.UTC().Format("2006-01-02")
}

func EndpointStatus(e inventory.Endpoint) (string, theme.Severity) {
	switch {
	case e.Outcome == "never":
		return "not checked yet", ""
	case e.Outcome == tlscheck.OutcomeUnreachable:
		return "unreachable", theme.SeverityMedium
	case e.Outcome == tlscheck.OutcomeHandshakeFailed:
		return "handshake failed", theme.SeverityMedium
	case e.ChainStatus == tlscheck.StatusInvalid && e.HostnameStatus == tlscheck.HostnameMismatch:
		return "chain and name failed", theme.SeverityHigh
	case e.ChainStatus == tlscheck.StatusInvalid:
		return "chain not trusted", theme.SeverityHigh
	case e.HostnameStatus == tlscheck.HostnameMismatch:
		return "name mismatch", theme.SeverityHigh
	}
	return "ok", ""
}

func labeled(name string, demo bool) string {
	if demo {
		return name + " (demo)"
	}
	return name
}

func stateText(enabled, demo bool) string {
	text := "enabled"
	if !enabled {
		text = "disabled"
	}
	if demo {
		text += ", demo"
	}
	return text
}

func marked(severity theme.Severity, text string) string {
	if severity == "" || severity == theme.SeverityInfo {
		return text
	}
	return severity.Glyph() + " " + text
}

func SplitMark(text string) (theme.Severity, string) {
	for _, severity := range theme.Severities {
		if rest, found := strings.CutPrefix(text, severity.Glyph()+" "); found && severity != theme.SeverityInfo {
			return severity, rest
		}
	}
	return "", text
}

func (a *App) when(ctx context.Context, moment time.Time) string {
	if moment.IsZero() {
		return "never"
	}
	return a.FormatTime(ctx, moment)
}

func (a *App) endpointTable(ctx context.Context, query ViewQuery) (Table, error) {
	list, err := a.Inventory.Endpoints(ctx, inventory.Query{Search: query.Search, Filter: query.Filter})
	if err != nil {
		return Table{}, err
	}
	values, err := a.Settings.Effective(ctx)
	if err != nil {
		return Table{}, err
	}
	key, descending := values[settings.KeyEndpointSort].Sort()
	less := func(i, j int) bool {
		switch key {
		case "expires":
			if list[i].NotAfter.Equal(list[j].NotAfter) {
				return false
			}
			if list[i].NotAfter.IsZero() || list[j].NotAfter.IsZero() {
				return list[j].NotAfter.IsZero()
			}
			return list[i].NotAfter.Before(list[j].NotAfter) != descending
		case "checked":
			if list[i].ObservedAt.Equal(list[j].ObservedAt) {
				return false
			}
			return list[i].ObservedAt.Before(list[j].ObservedAt) != descending
		}
		return descending && list[i].Address() > list[j].Address()
	}
	if key != "endpoint" || descending {
		sort.SliceStable(list, less)
	}
	all := []Column{{"endpoint", "Endpoint", 26, false}, {"status", "Status", 20, false}, {"certificate", "Certificate", 24, false}, {"expires", "Expires", 11, false}, {"days", "Days left", 18, true}, {"checked", "Last check", 17, false}, {"tls", "TLS", 8, false}, {"state", "State", 14, false}}
	shown := values[settings.KeyEndpointColumns]
	table := Table{Empty: "No endpoints yet. Add a host you are authorized to test, or load the demo data from the overview."}
	if query.Search != "" || (query.Filter != "" && query.Filter != "all") {
		table.Empty = "No endpoint matches the search and the filter."
	}
	var indexes []int
	for index, column := range all {
		if shown.Shows(column.ID) {
			table.Columns = append(table.Columns, column)
			indexes = append(indexes, index)
		}
	}
	now := a.Now()
	for _, e := range list {
		status, severity := EndpointStatus(e)
		if status == "ok" {
			_, expiry := inventory.ExpiryState(e.NotAfter, now)
			if expiry != theme.SeverityInfo {
				severity = expiry
			}
		}
		cells := []string{e.Address(), marked(severity, status), e.CertificateName, day(e.NotAfter), DaysText(e.NotAfter, now), a.when(ctx, e.ObservedAt), e.Version, stateText(e.Enabled, e.Demo)}
		row := Row{ID: e.ID, Severity: severity, Demo: e.Demo, Muted: !e.Enabled}
		for _, index := range indexes {
			row.Cells = append(row.Cells, cells[index])
		}
		table.Rows = append(table.Rows, row)
	}
	table.Footer = fmt.Sprintf("%d %s", len(list), plural(len(list), "endpoint", "endpoints"))
	return table, nil
}

func sourceText(c inventory.Certificate) string {
	text := "observed"
	if c.Source == inventory.SourceImported {
		text = "imported"
	}
	if c.Demo {
		text += ", demo"
	}
	return text
}

func (a *App) certificateTable(ctx context.Context, query ViewQuery) (Table, error) {
	list, err := a.Inventory.Certificates(ctx, inventory.Query{Search: query.Search, Filter: query.Filter})
	if err != nil {
		return Table{}, err
	}
	table := Table{
		Columns: []Column{{"certificate", "Certificate", 26, false}, {"issuer", "Issuer", 26, false}, {"expires", "Expires", 11, false}, {"days", "Days left", 18, true}, {"names", "Names", 24, false}, {"used", "Used by", 24, false}, {"source", "Source", 14, false}},
		Empty:   "No certificates yet. They appear after the first check of an endpoint or after importing a PEM file.",
	}
	if query.Search != "" || (query.Filter != "" && query.Filter != "tracked") {
		table.Empty = "No certificate matches the search and the filter."
	}
	now := a.Now()
	for _, c := range list {
		_, severity := inventory.ExpiryState(c.NotAfter, now)
		if severity == theme.SeverityInfo || !c.Tracked {
			severity = ""
		}
		names := strings.Join(c.Names, ", ")
		if len(c.Names) > 3 {
			names = fmt.Sprintf("%s and %d more", strings.Join(c.Names[:3], ", "), len(c.Names)-3)
		}
		table.Rows = append(table.Rows, Row{ID: c.ID, Severity: severity, Demo: c.Demo, Muted: !c.Tracked,
			Cells: []string{labeled(c.Name, c.Demo), issuerName(c.Issuer), day(c.NotAfter), marked(severity, DaysText(c.NotAfter, now)), names, c.UsedBy, sourceText(c)}})
	}
	table.Footer = fmt.Sprintf("%d %s", len(list), plural(len(list), "certificate", "certificates"))
	return table, nil
}

func issuerName(issuer string) string {
	for _, part := range strings.Split(issuer, ",") {
		if strings.HasPrefix(part, "CN=") {
			return strings.TrimPrefix(part, "CN=")
		}
	}
	return issuer
}

func Bar(days, window int) string {
	if window <= 0 {
		window = 1
	}
	filled := 0
	if days > 0 {
		filled = (days*barWidth + window - 1) / window
	}
	tail := "▏"
	if filled > barWidth {
		filled, tail = barWidth, "›"
	}
	return "▕" + strings.Repeat("█", filled) + strings.Repeat("░", barWidth-filled) + tail
}

func (a *App) timelineTable(ctx context.Context, query ViewQuery) (Table, error) {
	view, _ := LookupView(ViewTimeline)
	zoom := query.Filter
	if zoom == "" {
		zoom = view.Filters[0].ID
	}
	window := 0
	if zoom != "all" {
		number, err := strconv.Atoi(zoom)
		if err != nil || (number != 30 && number != 90 && number != 180 && number != 365) {
			return Table{}, &UsageError{fmt.Sprintf("unknown zoom %q: use 30, 90, 180, 365 or all", zoom)}
		}
		window = number
	}
	list, err := a.Inventory.Certificates(ctx, inventory.Query{Search: query.Search, Filter: "tracked"})
	if err != nil {
		return Table{}, err
	}
	now := a.Now()
	if window == 0 {
		window = 30
		for _, c := range list {
			if days := c.DaysLeft(now) + 1; days > window {
				window = days
			}
		}
	}
	table := Table{
		Columns: []Column{{"certificate", "Certificate", 26, false}, {"expires", "Expires", 11, false}, {"days", "Days left", 18, true}, {"timeline", "Time left", barWidth + 2, false}, {"used", "Used by", 26, false}},
		Empty:   fmt.Sprintf("No tracked certificate expires within %d days.", window),
		Window:  window,
	}
	if len(list) == 0 {
		table.Empty = "No certificates yet. They appear after the first check of an endpoint or after importing a PEM file."
	}
	hidden := 0
	for _, c := range list {
		days := c.DaysLeft(now)
		if zoom != "all" && days >= window {
			hidden++
			continue
		}
		_, severity := inventory.ExpiryState(c.NotAfter, now)
		table.Marks = append(table.Marks, Mark{Label: c.Name, Days: days, Severity: severity})
		if severity == theme.SeverityInfo {
			severity = ""
		}
		table.Rows = append(table.Rows, Row{ID: c.ID, Severity: severity, Demo: c.Demo,
			Cells: []string{labeled(c.Name, c.Demo), day(c.NotAfter), marked(severity, DaysText(c.NotAfter, now)), Bar(days, window), c.UsedBy}})
	}
	table.Footer = fmt.Sprintf("%d of %s within %d days; one block is about %d %s", len(table.Rows), words.Count(len(list), "tracked certificate expires", "tracked certificates expire"), window, (window+barWidth-1)/barWidth, plural((window+barWidth-1)/barWidth, "day", "days"))
	if hidden == 0 && zoom == "all" {
		table.Footer = fmt.Sprintf("%s over %d days; one block is about %d %s", words.Count(len(table.Rows), "tracked certificate", "tracked certificates"), window, (window+barWidth-1)/barWidth, plural((window+barWidth-1)/barWidth, "day", "days"))
	}
	return table, nil
}

func (a *App) chainTable(ctx context.Context, query ViewQuery) (Table, error) {
	switch query.Filter {
	case "", "all", "valid", "invalid":
	default:
		return Table{}, &UsageError{fmt.Sprintf("unknown filter %q: use all, valid or invalid", query.Filter)}
	}
	list, err := a.Inventory.Endpoints(ctx, inventory.Query{Search: query.Search, Filter: "enabled"})
	if err != nil {
		return Table{}, err
	}
	stats, err := a.Inventory.ChainStats(ctx)
	if err != nil {
		return Table{}, err
	}
	table := Table{
		Columns: []Column{{"endpoint", "Endpoint", 26, false}, {"result", "Chain", 16, false}, {"length", "Length", 7, true}, {"leaf", "Certificate", 24, false}, {"issuer", "Issued by", 26, false}, {"anchor", "Trust anchor", 16, false}, {"problem", "Problem", 34, false}},
		Empty:   "No chains yet. They appear after the first successful check of an endpoint.",
	}
	for _, e := range list {
		if e.Outcome != tlscheck.OutcomeOK {
			continue
		}
		if query.Filter == "valid" && e.ChainStatus != tlscheck.StatusValid || query.Filter == "invalid" && e.ChainStatus != tlscheck.StatusInvalid {
			continue
		}
		result, severity, anchor := "valid", theme.Severity(""), "system or added root"
		if e.ChainStatus == tlscheck.StatusInvalid {
			result, severity, anchor = marked(theme.SeverityHigh, "not trusted"), theme.SeverityHigh, "none"
		}
		table.Rows = append(table.Rows, Row{ID: e.ID, Severity: severity, Demo: e.Demo,
			Cells: []string{labeled(e.Address(), e.Demo), result, strconv.Itoa(stats[e.ObservationID].Depth), e.CertificateName, issuerName(e.CertificateIssue), anchor, e.ChainError}})
	}
	table.Footer = fmt.Sprintf("%d %s", len(table.Rows), plural(len(table.Rows), "chain", "chains"))
	return table, nil
}

func (a *App) configurationTable(ctx context.Context, query ViewQuery) (Table, error) {
	switch query.Filter {
	case "", "all", "findings":
	default:
		return Table{}, &UsageError{fmt.Sprintf("unknown filter %q: use all or findings", query.Filter)}
	}
	list, err := a.Inventory.Endpoints(ctx, inventory.Query{Search: query.Search, Filter: "enabled"})
	if err != nil {
		return Table{}, err
	}
	stats, err := a.Inventory.FindingStats(ctx)
	if err != nil {
		return Table{}, err
	}
	table := Table{
		Columns: []Column{{"endpoint", "Endpoint", 26, false}, {"version", "Negotiated", 11, false}, {"cipher", "Cipher suite", 36, false}, {"accepted", "Accepted versions", 28, false}, {"findings", "Findings", 9, true}, {"worst", "Worst", 12, false}},
		Empty:   "No observations yet. They appear after the first successful check of an endpoint.",
	}
	total := 0
	for _, e := range list {
		if e.Outcome != tlscheck.OutcomeOK {
			continue
		}
		stat := stats[e.ID]
		if query.Filter == "findings" && stat.Count == 0 {
			continue
		}
		accepted := e.Accepted
		if !e.ProtocolsTested {
			accepted = "not probed"
		}
		worst := ""
		if stat.Count > 0 {
			worst = stat.Worst.Glyph() + " " + stat.Worst.Label()
		}
		severity := stat.Worst
		if stat.Count == 0 || severity == theme.SeverityInfo {
			severity = ""
		}
		total += stat.Count
		table.Rows = append(table.Rows, Row{ID: e.ID, Severity: severity, Demo: e.Demo, Cells: []string{labeled(e.Address(), e.Demo), e.Version, e.CipherSuite, accepted, strconv.Itoa(stat.Count), worst}})
	}
	table.Footer = fmt.Sprintf("%d %s, %d %s", len(table.Rows), plural(len(table.Rows), "endpoint", "endpoints"), total, plural(total, "finding", "findings"))
	return table, nil
}

func sizeText(bytes int) string {
	if bytes < 1024 {
		return fmt.Sprintf("%d B", bytes)
	}
	return fmt.Sprintf("%.1f kB", float64(bytes)/1024)
}

func (a *App) reportTable(ctx context.Context, query ViewQuery) (Table, error) {
	list, err := a.Inventory.Reports(ctx, inventory.Query{Search: query.Search, Filter: query.Filter})
	if err != nil {
		return Table{}, err
	}
	table := Table{
		Columns: []Column{{"report", "Report", 38, false}, {"format", "Format", 9, false}, {"rows", "Rows", 6, true}, {"size", "Size", 9, true}, {"created", "Created", 17, false}, {"demo", "Demo data", 10, false}},
		Empty:   "No reports yet. Generate one from the current data.",
	}
	for _, r := range list {
		demo := "no"
		if r.IncludesDemo {
			demo = "included"
		}
		table.Rows = append(table.Rows, Row{ID: r.ID, Demo: r.IncludesDemo, Cells: []string{r.Title, r.Format, strconv.Itoa(r.Rows), sizeText(r.Size), a.FormatTime(ctx, r.CreatedAt), demo}})
	}
	table.Footer = fmt.Sprintf("%d %s", len(list), plural(len(list), "report", "reports"))
	return table, nil
}

func (a *App) Table(ctx context.Context, view string, query ViewQuery) (Table, error) {
	switch view {
	case ViewCertificates:
		return a.certificateTable(ctx, query)
	case ViewEndpoints:
		return a.endpointTable(ctx, query)
	case ViewTimeline:
		return a.timelineTable(ctx, query)
	case ViewChains:
		return a.chainTable(ctx, query)
	case ViewConfiguration:
		return a.configurationTable(ctx, query)
	case ViewReports:
		return a.reportTable(ctx, query)
	}
	return Table{}, &UsageError{fmt.Sprintf("unknown view %q", view)}
}

func chainSection(chain []inventory.ChainLink, now time.Time) Section {
	table := &Table{Columns: []Column{{"role", "Role", 13, false}, {"certificate", "Certificate", 30, false}, {"issuer", "Issued by", 30, false}, {"expires", "Expires", 11, false}, {"key", "Key", 12, false}, {"origin", "Sent by", 16, false}}, Empty: "No chain was recorded."}
	for _, link := range chain {
		origin := "server"
		if link.Origin == inventory.OriginTrustStore {
			origin = "trust store"
		}
		issuer := issuerName(link.Issuer)
		if link.SelfSigned {
			issuer = "itself"
		}
		_, severity := inventory.ExpiryState(link.NotAfter, now)
		if severity != theme.SeverityCritical {
			severity = ""
		}
		table.Rows = append(table.Rows, Row{ID: link.CertificateID, Severity: severity, Cells: []string{link.Role(), link.Name, issuer, day(link.NotAfter), fmt.Sprintf("%s %d", link.KeyAlgorithm, link.KeyBits), origin}})
	}
	return Section{Title: "Chain", Table: table}
}

func changeSection(ctx context.Context, a *App, changes []inventory.Change) Section {
	table := &Table{Columns: []Column{{"when", "Observed", 17, false}, {"endpoint", "Endpoint", 24, false}, {"from", "Before", 34, false}, {"to", "After", 34, false}}, Empty: "The certificate did not change between observations."}
	for _, change := range changes {
		table.Rows = append(table.Rows, Row{ID: change.ObservationID, Cells: []string{a.FormatTime(ctx, change.At), change.Address,
			change.FromName + " " + tlscheck.ShortFingerprint(change.FromPrint), change.ToName + " " + tlscheck.ShortFingerprint(change.ToPrint)}})
	}
	return Section{Title: "Change history", Table: table}
}

func (a *App) certificateDetail(ctx context.Context, id int64) (Detail, error) {
	c, err := a.Inventory.Certificate(ctx, id)
	if err != nil {
		return Detail{}, err
	}
	now := a.Now()
	state, severity := inventory.ExpiryState(c.NotAfter, now)
	if severity == theme.SeverityInfo {
		severity = ""
	}
	tracked := "yes"
	if !c.Tracked {
		tracked = "no, kept as part of a chain or of the history"
	}
	kind := "end-entity certificate"
	switch {
	case c.IsCA && c.SelfSigned:
		kind = "root authority"
	case c.IsCA:
		kind = "intermediate authority"
	case c.SelfSigned:
		kind = "self-signed certificate"
	}
	detail := Detail{Title: c.Name, Subtitle: fmt.Sprintf("%s, %s", state, DaysText(c.NotAfter, now)), Severity: severity}
	detail.Sections = append(detail.Sections, Section{Title: "Certificate", Facts: [][2]string{
		{"Subject", c.Subject}, {"Issuer", c.Issuer}, {"Kind", kind}, {"Valid from", day(c.NotBefore)}, {"Valid until", day(c.NotAfter) + " (" + DaysText(c.NotAfter, now) + ")"},
		{"Key", fmt.Sprintf("%s %d bits", c.KeyAlgorithm, c.KeyBits)}, {"Signature", c.SignatureAlgorithm}, {"Serial", c.Serial}, {"SHA-256", c.Fingerprint},
		{"Source", sourceText(c)}, {"Tracked", tracked}, {"First seen", a.FormatTime(ctx, c.FirstSeen)}, {"Last seen", a.FormatTime(ctx, c.LastSeen)}}})
	names := "This certificate names no host."
	if len(c.Names) > 0 {
		names = strings.Join(c.Names, "\n")
	}
	detail.Sections = append(detail.Sections, Section{Title: fmt.Sprintf("Subject alternative names (%d)", len(c.Names)), Text: names})
	chain, err := a.Inventory.LatestChainOf(ctx, id)
	if err != nil {
		return detail, err
	}
	if len(chain) > 0 {
		detail.Sections = append(detail.Sections, chainSection(chain, now))
	}
	usage, err := a.Inventory.Usage(ctx, id)
	if err != nil {
		return detail, err
	}
	used := &Table{Columns: []Column{{"endpoint", "Endpoint", 26, false}, {"first", "First observed", 17, false}, {"last", "Last observed", 17, false}, {"count", "Observations", 12, true}, {"current", "Presented now", 13, false}}, Empty: "No endpoint presented this certificate."}
	for _, item := range usage {
		used.Rows = append(used.Rows, Row{ID: item.EndpointID, Cells: []string{item.Address, a.FormatTime(ctx, item.First), a.FormatTime(ctx, item.Last), strconv.Itoa(item.Count), yesNo(item.Current)}})
	}
	detail.Sections = append(detail.Sections, Section{Title: "Endpoints", Table: used})
	changes, err := a.Inventory.CertificateChanges(ctx, id, historyShown)
	if err != nil {
		return detail, err
	}
	detail.Sections = append(detail.Sections, changeSection(ctx, a, changes))
	return detail, nil
}

func findingSection(findings []inventory.Finding) Section {
	table := &Table{Columns: []Column{{"severity", "Severity", 12, false}, {"finding", "Finding", 28, false}, {"detail", "What it means", 80, false}}, Empty: "The last observation raised no findings."}
	for _, finding := range findings {
		table.Rows = append(table.Rows, Row{ID: finding.ID, Severity: finding.Severity, Cells: []string{finding.Severity.Glyph() + " " + finding.Severity.Label(), tlscheck.FindingTitle(finding.Code), finding.Detail}})
	}
	return Section{Title: fmt.Sprintf("Findings (%d)", len(findings)), Table: table}
}

func (a *App) endpointDetail(ctx context.Context, id int64, focus string) (Detail, error) {
	e, err := a.Inventory.Endpoint(ctx, id)
	if err != nil {
		return Detail{}, err
	}
	now := a.Now()
	status, severity := EndpointStatus(e)
	detail := Detail{Title: e.Address(), Subtitle: status, Severity: severity}
	facts := [][2]string{{"Verified name", e.VerifiedName()}, {"State", stateText(e.Enabled, e.Demo)}, {"Authorization confirmed", a.FormatTime(ctx, e.AuthorizedAt)}, {"Last check", a.when(ctx, e.ObservedAt)}, {"Result", status}}
	if e.Error != "" {
		facts = append(facts, [2]string{"Problem", e.Error})
	}
	if e.Outcome == tlscheck.OutcomeOK {
		chainText, nameText := "valid", "matches "+e.VerifiedName()
		if e.ChainStatus == tlscheck.StatusInvalid {
			chainText = "not trusted: " + e.ChainError
		}
		if e.HostnameStatus == tlscheck.HostnameMismatch {
			nameText = "mismatch: " + e.HostnameError
		}
		accepted := e.Accepted
		if !e.ProtocolsTested {
			accepted = "not probed"
		}
		facts = append(facts, [2]string{"Certificate", e.CertificateName}, [2]string{"Issued by", issuerName(e.CertificateIssue)}, [2]string{"Expires", day(e.NotAfter) + " (" + DaysText(e.NotAfter, now) + ")"},
			[2]string{"Chain", chainText}, [2]string{"Host name", nameText}, [2]string{"Negotiated", strings.TrimSpace(e.Version + " " + e.CipherSuite)}, [2]string{"Accepted versions", accepted})
	}
	if e.Note != "" {
		facts = append(facts, [2]string{"Note", e.Note})
	}
	if e.Demo {
		facts = append(facts, [2]string{"Demo", "This endpoint is demo data. It is never contacted."})
	}
	detail.Sections = append(detail.Sections, Section{Title: "Endpoint", Facts: facts})
	var chain Section
	if e.ObservationID != 0 && e.Outcome == tlscheck.OutcomeOK {
		links, err := a.Inventory.Chain(ctx, e.ObservationID)
		if err != nil {
			return detail, err
		}
		chain = chainSection(links, now)
	}
	findings, err := a.Inventory.Findings(ctx, id)
	if err != nil {
		return detail, err
	}
	history, total, err := a.Inventory.History(ctx, id, historyShown)
	if err != nil {
		return detail, err
	}
	observed := &Table{Columns: []Column{{"when", "Observed", 17, false}, {"result", "Result", 18, false}, {"certificate", "Certificate", 26, false}, {"expires", "Expires", 11, false}, {"tls", "TLS", 8, false}, {"note", "Note", 30, false}}, Empty: "This endpoint was not checked yet."}
	for _, item := range history {
		result, note := "ok", item.Error
		switch {
		case item.Outcome != tlscheck.OutcomeOK:
			result = strings.ReplaceAll(item.Outcome, "_", " ")
		case item.ChainStatus == tlscheck.StatusInvalid || item.HostnameStatus == tlscheck.HostnameMismatch:
			result = "validation failed"
		}
		if item.Changed {
			note = "certificate changed"
		}
		observed.Rows = append(observed.Rows, Row{ID: item.ID, Cells: []string{a.FormatTime(ctx, item.At), result, item.CertificateName, day(item.NotAfter), item.Version, note}})
	}
	changes, err := a.Inventory.EndpointChanges(ctx, id, historyShown)
	if err != nil {
		return detail, err
	}
	observations := Section{Title: fmt.Sprintf("Observations (latest %d of %d)", len(history), total), Table: observed}
	switch focus {
	case ViewChains:
		if chain.Table != nil {
			detail.Sections = append(detail.Sections, chain)
		}
		detail.Sections = append(detail.Sections, findingSection(findings))
	case ViewConfiguration:
		detail.Sections = append(detail.Sections, findingSection(findings))
	default:
		detail.Sections = append(detail.Sections, findingSection(findings))
		if chain.Table != nil {
			detail.Sections = append(detail.Sections, chain)
		}
		detail.Sections = append(detail.Sections, observations, changeSection(ctx, a, changes))
	}
	return detail, nil
}

func (a *App) reportDetail(ctx context.Context, id int64) (Detail, error) {
	r, err := a.Inventory.Report(ctx, id)
	if err != nil {
		return Detail{}, err
	}
	detail := Detail{Title: r.Title, Subtitle: r.Format}
	demo := "no"
	if r.IncludesDemo {
		demo = "yes, demo rows are marked in the report"
	}
	detail.Sections = append(detail.Sections, Section{Title: "Report", Facts: [][2]string{{"Content", r.Kind}, {"Format", r.Format}, {"Rows", strconv.Itoa(r.Rows)}, {"Size", sizeText(r.Size)}, {"Created", a.FormatTime(ctx, r.CreatedAt)}, {"Demo data", demo}, {"SHA-256", r.SHA256}, {"Suggested file name", r.FileName()}}})
	lines := strings.Split(strings.TrimRight(r.Content, "\n"), "\n")
	title := "Content"
	if len(lines) > 40 {
		title = fmt.Sprintf("Content (first 40 of %d lines)", len(lines))
		lines = lines[:40]
	}
	detail.Sections = append(detail.Sections, Section{Title: title, Text: strings.Join(lines, "\n")})
	return detail, nil
}

func (a *App) Detail(ctx context.Context, view string, id int64) (Detail, error) {
	switch view {
	case ViewCertificates, ViewTimeline:
		return a.certificateDetail(ctx, id)
	case ViewEndpoints, ViewChains, ViewConfiguration:
		return a.endpointDetail(ctx, id, view)
	case ViewReports:
		return a.reportDetail(ctx, id)
	}
	return Detail{}, &UsageError{fmt.Sprintf("unknown view %q", view)}
}

func (a *App) lookup(request Request) (Action, error) {
	view, found := LookupView(request.View)
	if !found {
		return Action{}, &UsageError{fmt.Sprintf("unknown view %q", request.View)}
	}
	action, found := view.Action(request.Action)
	if !found {
		return Action{}, &UsageError{fmt.Sprintf("%s has no action %q", view.Title, request.Action)}
	}
	if action.Row && request.ID <= 0 {
		return action, &UsageError{fmt.Sprintf("%s needs one entry: select a row or give its identifier", action.Label)}
	}
	for _, field := range action.Fields {
		_, given := request.Values[field.ID]
		if field.Required && field.Kind != FieldBool && request.value(field.ID) == "" && (given || !action.Row) {
			return action, &UsageError{fmt.Sprintf("%s is missing: %s", field.Label, strings.TrimSuffix(field.Help, "."))}
		}
		if field.Kind == FieldChoice && request.value(field.ID) != "" {
			known := false
			for _, option := range field.Options {
				known = known || option.ID == request.value(field.ID)
			}
			if !known {
				ids := make([]string, len(field.Options))
				for i, option := range field.Options {
					ids[i] = option.ID
				}
				return action, &UsageError{fmt.Sprintf("%s: %q is not one of %s", field.Label, request.value(field.ID), strings.Join(ids, ", "))}
			}
		}
	}
	return action, nil
}

func (a *App) Defaults(ctx context.Context, request Request) (map[string]string, error) {
	view, found := LookupView(request.View)
	if !found {
		return nil, &UsageError{fmt.Sprintf("unknown view %q", request.View)}
	}
	action, found := view.Action(request.Action)
	if !found {
		return nil, &UsageError{fmt.Sprintf("%s has no action %q", view.Title, request.Action)}
	}
	values := map[string]string{}
	for _, field := range action.Fields {
		values[field.ID] = field.Default
	}
	switch request.View + "/" + request.Action {
	case "endpoints/edit":
		e, err := a.Inventory.Endpoint(ctx, request.ID)
		if err != nil {
			return nil, err
		}
		values["host"], values["port"], values["server-name"], values["note"], values["enabled"] = e.Host, strconv.Itoa(e.Port), e.ServerName, e.Note, strconv.FormatBool(e.Enabled)
		values["revision"] = strconv.FormatInt(e.Revision, 10)
	case "reports/export":
		r, err := a.Inventory.Report(ctx, request.ID)
		if err != nil {
			return nil, err
		}
		values["file"] = r.FileName()
	case "certificates/export":
		c, err := a.Inventory.Certificate(ctx, request.ID)
		if err != nil {
			return nil, err
		}
		values["file"] = "certificate-" + tlscheck.ShortFingerprint(c.Fingerprint) + ".pem"
	case "endpoints/export":
		values["file"] = "certwatch-endpoints.csv"
	}
	return values, nil
}

func (a *App) confirms(ctx context.Context) bool {
	value, err := a.Settings.Get(ctx, settings.KeyConfirm)
	return err != nil || value.Bool()
}

func (a *App) endpointInput(request Request, base inventory.EndpointInput) (inventory.EndpointInput, error) {
	in := base
	if _, given := request.Values["host"]; given {
		in.Host = request.value("host")
	}
	if text := request.value("port"); text != "" {
		port, err := strconv.Atoi(text)
		if err != nil {
			return in, &inventory.ValidationError{Field: "port", Problem: fmt.Sprintf("%q is not a port", text), Fix: "Use a number from 1 to 65535, for example 443"}
		}
		in.Port = port
	}
	if _, given := request.Values["server-name"]; given {
		in.ServerName = request.value("server-name")
	}
	if _, given := request.Values["note"]; given {
		in.Note = request.Values["note"]
	}
	if _, given := request.Values["enabled"]; given {
		in.Enabled = request.flag("enabled")
	}
	return inventory.Normalize(in)
}

func (a *App) checkTargets(ctx context.Context, request Request) ([]inventory.Endpoint, bool, error) {
	probe := a.probe(ctx) || request.Action == "probe"
	if request.Action == "check-all" {
		list, err := a.Inventory.Endpoints(ctx, inventory.Query{Filter: "enabled"})
		return list, probe, err
	}
	e, err := a.Inventory.Endpoint(ctx, request.ID)
	return []inventory.Endpoint{e}, probe, err
}

func (a *App) Plan(ctx context.Context, request Request) (Plan, error) {
	action, err := a.lookup(request)
	if err != nil {
		return Plan{}, err
	}
	plan := Plan{Danger: action.Danger, Confirm: action.Network || (action.Danger && a.confirms(ctx))}
	switch request.View + "/" + request.Action {
	case "endpoints/add":
		in, err := a.endpointInput(request, inventory.EndpointInput{Enabled: true})
		if err != nil {
			return plan, err
		}
		plan.Statement = fmt.Sprintf("Add %s to the monitored endpoints. Nothing is contacted until you run a check.", inventory.Endpoint{Host: in.Host, Port: in.Port}.Address())
	case "endpoints/edit":
		e, err := a.Inventory.Endpoint(ctx, request.ID)
		if err != nil {
			return plan, err
		}
		in, err := a.endpointInput(request, e.Input())
		if err != nil {
			return plan, err
		}
		plan.Statement = fmt.Sprintf("Change endpoint %s to %s.", e.Address(), inventory.Endpoint{Host: in.Host, Port: in.Port}.Address())
	case "endpoints/enable", "endpoints/disable":
		e, err := a.Inventory.Endpoint(ctx, request.ID)
		if err != nil {
			return plan, err
		}
		plan.Statement = fmt.Sprintf("%s endpoint %s.", action.Label, e.Address())
	case "endpoints/remove":
		e, err := a.Inventory.Endpoint(ctx, request.ID)
		if err != nil {
			return plan, err
		}
		_, total, err := a.Inventory.History(ctx, request.ID, 1)
		if err != nil {
			return plan, err
		}
		plan.Statement = fmt.Sprintf("Remove endpoint %s and its %d %s. This cannot be undone.", e.Address(), total, plural(total, "observation", "observations"))
	case "endpoints/check", "endpoints/check-all", "configuration/probe":
		targets, probe, err := a.checkTargets(ctx, request)
		if err != nil {
			return plan, err
		}
		plan.Network = Disclosure(targets, probe)
		skipped := 0
		for _, target := range targets {
			if target.Demo {
				skipped++
			}
		}
		if len(plan.Network) == 0 {
			plan.Confirm = false
			plan.Statement = "There is nothing to check: no enabled endpoint that is not demo data. Demo endpoints are never contacted."
			return plan, nil
		}
		plan.Statement = fmt.Sprintf("Check %d %s over the network. Only the connections listed here are made; no request is sent after the handshake.", len(plan.Network), plural(len(plan.Network), "endpoint", "endpoints"))
		if skipped > 0 {
			plan.Statement += fmt.Sprintf(" %d demo %s skipped.", skipped, plural(skipped, "endpoint is", "endpoints are"))
		}
	case "endpoints/import":
		format, err := TransferFormat(request.value("file"), "")
		if err != nil {
			return plan, err
		}
		data, err := ReadFile(request.value("file"), inventory.MaxImportBytes)
		if err != nil {
			return plan, err
		}
		preview, err := a.Inventory.ImportEndpoints(ctx, bytes.NewReader(data), format, true, false)
		if err != nil {
			return plan, err
		}
		plan.Statement = "Import endpoints from " + request.value("file") + ": " + preview.Summary() + "."
		plan.Lines = append(plan.Lines, preview.Hosts...)
		for _, problem := range preview.Errors {
			plan.Lines = append(plan.Lines, problem.Error())
		}
	case "endpoints/export":
		if _, err := TransferFormat(request.value("file"), ""); err != nil {
			return plan, err
		}
		plan.Statement = "Write every endpoint to " + request.value("file") + "."
	case "certificates/import":
		data, err := ReadFile(request.value("file"), tlscheck.MaxPEMSize)
		if err != nil {
			return plan, err
		}
		preview, err := a.Inventory.ImportCertificates(ctx, data, true, inventory.Policy{})
		if err != nil {
			return plan, err
		}
		plan.Statement = "Import certificates from " + request.value("file") + ": " + preview.Summary() + "."
		plan.Lines = preview.Names
	case "certificates/export":
		c, err := a.Inventory.Certificate(ctx, request.ID)
		if err != nil {
			return plan, err
		}
		plan.Statement = fmt.Sprintf("Write the certificate %s to %s.", c.Name, request.value("file"))
	case "certificates/stop-tracking":
		c, err := a.Inventory.Certificate(ctx, request.ID)
		if err != nil {
			return plan, err
		}
		plan.Statement = fmt.Sprintf("Stop tracking the imported certificate %s. It is removed unless an observation still refers to it.", c.Name)
	case "reports/generate":
		kind, format := request.value("kind"), request.value("format")
		if kind == "" {
			kind = inventory.ReportInventory
		}
		if format == "" {
			format = inventory.FormatMarkdown
		}
		plan.Statement = fmt.Sprintf("Generate the report \"%s\" as %s and store it in the database.", inventory.ReportTitle(kind), format)
	case "reports/export":
		r, err := a.Inventory.Report(ctx, request.ID)
		if err != nil {
			return plan, err
		}
		plan.Statement = fmt.Sprintf("Write the report \"%s\" to %s.", r.Title, request.value("file"))
	case "reports/remove":
		r, err := a.Inventory.Report(ctx, request.ID)
		if err != nil {
			return plan, err
		}
		plan.Statement = fmt.Sprintf("Remove the report \"%s\" created %s from the database. Saved files stay.", r.Title, a.FormatTime(ctx, r.CreatedAt))
	}
	return plan, nil
}

func (a *App) Run(ctx context.Context, request Request) (Outcome, error) {
	if _, err := a.lookup(request); err != nil {
		return Outcome{}, err
	}
	defer a.touch()
	switch request.View + "/" + request.Action {
	case "endpoints/add":
		in, err := a.endpointInput(request, inventory.EndpointInput{Enabled: true})
		if err != nil {
			return Outcome{}, err
		}
		created, err := a.Inventory.CreateEndpoint(ctx, in, request.flag("authorized"))
		if err != nil {
			return Outcome{}, err
		}
		return Outcome{Message: fmt.Sprintf("Added endpoint %d: %s. Run a check to record its certificate.", created.ID, created.Address()), Select: created.ID}, nil
	case "endpoints/edit":
		e, err := a.Inventory.Endpoint(ctx, request.ID)
		if err != nil {
			return Outcome{}, err
		}
		in, err := a.endpointInput(request, e.Input())
		if err != nil {
			return Outcome{}, err
		}
		revision := e.Revision
		if text := request.value("revision"); text != "" {
			if revision, err = strconv.ParseInt(text, 10, 64); err != nil {
				return Outcome{}, &UsageError{fmt.Sprintf("%q is not a revision", text)}
			}
		}
		updated, err := a.Inventory.UpdateEndpoint(ctx, e.ID, revision, in, request.flag("authorized"))
		var conflict *inventory.ConflictError
		if errors.As(err, &conflict) {
			return Outcome{}, &UsageError{fmt.Sprintf("another window changed %s while you were editing. Nothing was overwritten: close this editor, open it again to see the current values, and repeat your change", e.Address())}
		}
		if err != nil {
			return Outcome{}, err
		}
		return Outcome{Message: fmt.Sprintf("Saved endpoint %d: %s.", updated.ID, updated.Address()), Select: updated.ID}, nil
	case "endpoints/enable", "endpoints/disable":
		updated, err := a.Inventory.SetEndpointEnabled(ctx, request.ID, request.Action == "enable")
		if err != nil {
			return Outcome{}, err
		}
		return Outcome{Message: fmt.Sprintf("Endpoint %s is now %s.", updated.Address(), stateText(updated.Enabled, false)), Select: updated.ID}, nil
	case "endpoints/remove":
		e, err := a.Inventory.Endpoint(ctx, request.ID)
		if err != nil {
			return Outcome{}, err
		}
		if err := a.Inventory.DeleteEndpoint(ctx, request.ID); err != nil {
			return Outcome{}, err
		}
		return Outcome{Message: fmt.Sprintf("Removed endpoint %s.", e.Address())}, nil
	case "endpoints/check", "endpoints/check-all", "configuration/probe":
		targets, probe, err := a.checkTargets(ctx, request)
		if err != nil {
			return Outcome{}, err
		}
		report, err := a.CheckEndpoints(ctx, targets, probe)
		if err != nil {
			return Outcome{}, err
		}
		outcome := Outcome{Message: strings.ToUpper(report.Summary()[:1]) + report.Summary()[1:] + ".", Select: request.ID}
		if report.Checked == 0 {
			outcome.Message = "Nothing was checked. Demo endpoints are never contacted."
		}
		for _, line := range report.Lines {
			outcome.Lines = append(outcome.Lines, fmt.Sprintf("%s: %s (%s)", line.Address, line.Outcome, line.Detail))
		}
		return outcome, nil
	case "endpoints/import":
		format, err := TransferFormat(request.value("file"), "")
		if err != nil {
			return Outcome{}, err
		}
		data, err := ReadFile(request.value("file"), inventory.MaxImportBytes)
		if err != nil {
			return Outcome{}, err
		}
		result, err := a.Inventory.ImportEndpoints(ctx, bytes.NewReader(data), format, false, request.flag("authorized"))
		if err != nil {
			return Outcome{}, err
		}
		outcome := Outcome{Message: strings.ToUpper(result.Summary()[:1]) + result.Summary()[1:] + "."}
		for _, problem := range result.Errors {
			outcome.Lines = append(outcome.Lines, problem.Error())
		}
		return outcome, nil
	case "endpoints/export":
		format, err := TransferFormat(request.value("file"), "")
		if err != nil {
			return Outcome{}, err
		}
		list, err := a.Inventory.Endpoints(ctx, inventory.Query{})
		if err != nil {
			return Outcome{}, err
		}
		var buffer bytes.Buffer
		if err := inventory.ExportEndpoints(&buffer, format, list); err != nil {
			return Outcome{}, err
		}
		if err := WriteFile(request.value("file"), buffer.Bytes(), request.flag("overwrite")); err != nil {
			return Outcome{}, err
		}
		return Outcome{Message: fmt.Sprintf("Wrote %d %s to %s.", len(list), plural(len(list), "endpoint", "endpoints"), request.value("file"))}, nil
	case "certificates/import":
		data, err := ReadFile(request.value("file"), tlscheck.MaxPEMSize)
		if err != nil {
			return Outcome{}, err
		}
		policy, err := a.Policy(ctx)
		if err != nil {
			return Outcome{}, err
		}
		result, err := a.Inventory.ImportCertificates(ctx, data, false, policy)
		if err != nil {
			return Outcome{}, err
		}
		return Outcome{Message: strings.ToUpper(result.Summary()[:1]) + result.Summary()[1:] + ".", Lines: result.Names}, nil
	case "certificates/export":
		c, err := a.Inventory.Certificate(ctx, request.ID)
		if err != nil {
			return Outcome{}, err
		}
		if err := WriteFile(request.value("file"), []byte(c.PEM), request.flag("overwrite")); err != nil {
			return Outcome{}, err
		}
		return Outcome{Message: fmt.Sprintf("Wrote the certificate %s to %s.", c.Name, request.value("file")), Select: c.ID}, nil
	case "certificates/stop-tracking":
		c, err := a.Inventory.Certificate(ctx, request.ID)
		if err != nil {
			return Outcome{}, err
		}
		if err := a.Inventory.StopTracking(ctx, request.ID); err != nil {
			return Outcome{}, err
		}
		return Outcome{Message: fmt.Sprintf("Stopped tracking %s.", c.Name)}, nil
	case "reports/generate":
		kind, format := request.value("kind"), request.value("format")
		if kind == "" {
			kind = inventory.ReportInventory
		}
		if format == "" {
			format = inventory.FormatMarkdown
		}
		report, err := a.Inventory.GenerateReport(ctx, kind, format)
		if err != nil {
			return Outcome{}, err
		}
		return Outcome{Message: fmt.Sprintf("Generated report %d: %s, %d %s.", report.ID, report.Title, report.Rows, plural(report.Rows, "row", "rows")), Select: report.ID}, nil
	case "reports/export":
		r, err := a.Inventory.Report(ctx, request.ID)
		if err != nil {
			return Outcome{}, err
		}
		if err := WriteFile(request.value("file"), []byte(r.Content), request.flag("overwrite")); err != nil {
			return Outcome{}, err
		}
		return Outcome{Message: fmt.Sprintf("Wrote the report to %s.", request.value("file")), Select: r.ID}, nil
	case "reports/remove":
		if err := a.Inventory.DeleteReport(ctx, request.ID); err != nil {
			return Outcome{}, err
		}
		return Outcome{Message: fmt.Sprintf("Removed report %d.", request.ID)}, nil
	}
	return Outcome{}, &UsageError{fmt.Sprintf("%s/%s is not implemented", request.View, request.Action)}
}

type Tile struct {
	ID       string
	Label    string
	Value    int
	Severity theme.Severity
	View     string
	Filter   string
}

type OverviewData struct {
	Counts  inventory.Overview
	Tiles   []Tile
	Soonest Table
	Alerts  Table
	Empty   bool
	Demo    bool
}

func (a *App) Overview(ctx context.Context) (OverviewData, error) {
	counts, err := a.Inventory.Overview(ctx)
	if err != nil {
		return OverviewData{}, err
	}
	data := OverviewData{Counts: counts, Empty: counts.Empty(), Demo: counts.DemoRows > 0}
	tone := func(value int, severity theme.Severity) theme.Severity {
		if value == 0 {
			return ""
		}
		return severity
	}
	data.Tiles = []Tile{
		{"expired", "Expired", counts.Expired, tone(counts.Expired, theme.SeverityCritical), ViewCertificates, "expired"},
		{"within7", "Expiring in 7 days", counts.Within7, tone(counts.Within7, theme.SeverityHigh), ViewCertificates, "expiring-7"},
		{"within30", "Expiring in 30 days", counts.Within30, tone(counts.Within30, theme.SeverityMedium), ViewCertificates, "expiring-30"},
		{"within90", "Expiring in 90 days", counts.Within90, tone(counts.Within90, theme.SeverityLow), ViewCertificates, "expiring-90"},
		{"chain", "Chain failures", counts.ChainFailures, tone(counts.ChainFailures, theme.SeverityHigh), ViewEndpoints, "chain"},
		{"hostname", "Name mismatches", counts.HostnameMismatches, tone(counts.HostnameMismatches, theme.SeverityHigh), ViewEndpoints, "hostname"},
		{"failed", "Failed checks", counts.FailedChecks, tone(counts.FailedChecks, theme.SeverityMedium), ViewEndpoints, "unreachable"},
		{"never", "Not checked yet", counts.NeverChecked, tone(counts.NeverChecked, theme.SeverityInfo), ViewEndpoints, "never"},
	}
	list, err := a.Inventory.Certificates(ctx, inventory.Query{Filter: "tracked", Limit: 8})
	if err != nil {
		return data, err
	}
	now := a.Now()
	data.Soonest = Table{Columns: []Column{{"certificate", "Certificate", 28, false}, {"expires", "Expires", 11, false}, {"days", "Days left", 18, true}, {"used", "Used by", 28, false}}, Empty: "No certificates yet."}
	for _, c := range list {
		_, severity := inventory.ExpiryState(c.NotAfter, now)
		if severity == theme.SeverityInfo {
			severity = ""
		}
		data.Soonest.Rows = append(data.Soonest.Rows, Row{ID: c.ID, Severity: severity, Demo: c.Demo, Cells: []string{labeled(c.Name, c.Demo), day(c.NotAfter), marked(severity, DaysText(c.NotAfter, now)), c.UsedBy}})
	}
	alerts, err := a.Notices.List(ctx, notify.Filter{Limit: 5})
	if err != nil {
		return data, err
	}
	data.Alerts = Table{Columns: []Column{{"when", "When", 17, false}, {"severity", "Severity", 12, false}, {"alert", "Alert", 60, false}}, Empty: "No alerts yet."}
	for _, alert := range alerts {
		data.Alerts.Rows = append(data.Alerts.Rows, Row{ID: alert.ID, Severity: alert.Severity, Muted: alert.ReadAt != nil, Cells: []string{a.FormatTime(ctx, alert.CreatedAt), alert.Severity.Glyph() + " " + alert.Severity.Label(), alert.Title}})
	}
	return data, nil
}

func (a *App) LoadDemo(ctx context.Context) (int, error) {
	policy, err := a.Policy(ctx)
	if err != nil {
		return 0, err
	}
	defer a.touch()
	return a.Inventory.LoadDemo(ctx, policy)
}
