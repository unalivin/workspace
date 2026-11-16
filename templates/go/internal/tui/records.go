package tui

import (
	"errors"
	"fmt"
	"strconv"
	"strings"

	"github.com/unalivin/templateapp/internal/app"
	"github.com/unalivin/templateapp/internal/notify"
	"github.com/unalivin/templateapp/internal/records"
	"github.com/unalivin/templateapp/internal/settings"
	"github.com/unalivin/templateapp/internal/theme"
)

const listLimit = 500

type recordsPage struct {
	query     records.Query
	list      []records.Record
	cursor    int
	offset    int
	detail    bool
	searching bool
	search    input
}

func (p *recordsPage) selected() (records.Record, bool) {
	if p.cursor < 0 || p.cursor >= len(p.list) {
		return records.Record{}, false
	}
	return p.list[p.cursor], true
}

func (p *recordsPage) load(m *Model) {
	previous, had := p.selected()
	if key, descending := m.values[settings.KeyRecordSort].Sort(); key != "" {
		p.query.Sort, p.query.Descending = key, descending
	}
	query := p.query
	query.Limit = listLimit
	list, err := m.app.Records.List(m.ctx, query)
	if m.fail(err) {
		return
	}
	p.list = list
	if had {
		for index, record := range list {
			if record.ID == previous.ID {
				p.cursor = index
				return
			}
		}
	}
	if p.cursor >= len(list) {
		p.cursor = len(list) - 1
	}
	if p.cursor < 0 {
		p.cursor = 0
	}
}

func (p *recordsPage) selectID(id int64) {
	for index, record := range p.list {
		if record.ID == id {
			p.cursor = index
		}
	}
}

func severityOptions() ([]string, []string) {
	var options, labels []string
	for _, level := range theme.Severities {
		options = append(options, string(level))
		labels = append(labels, level.Glyph()+" "+level.Label())
	}
	return options, labels
}

func statusOptions() ([]string, []string) {
	var labels []string
	for _, status := range records.Statuses {
		labels = append(labels, records.StatusGlyph(status)+" "+records.StatusLabel(status))
	}
	return records.Statuses, labels
}

func recordForm(title string, in records.Input, submit func(records.Input) error) *form {
	severities, severityLabels := severityOptions()
	statuses, statusLabels := statusOptions()
	enabled := "true"
	if !in.Enabled {
		enabled = "false"
	}
	return &form{title: title, fields: []field{
		textField("name", "Name", in.Name, "Required, unique, at most "+strconv.Itoa(records.MaxNameLength)+" characters.", records.MaxNameLength),
		choiceField("status", "Status", in.Status, "", statuses, statusLabels),
		choiceField("severity", "Severity", string(in.Severity), "High and critical open records raise a notification.", severities, severityLabels),
		textField("note", "Note", in.Note, "Optional free text, searchable.", records.MaxNoteLength),
		choiceField("enabled", "Enabled", enabled, "Disabled records are kept but not counted.", []string{"true", "false"}, []string{"Yes", "No"}),
	}, submit: func(values map[string]string) error {
		return submit(records.Input{Name: values["name"], Status: values["status"], Severity: theme.Severity(values["severity"]), Note: values["note"], Enabled: values["enabled"] == "true"})
	}}
}

func (p *recordsPage) key(m *Model, key string, typed []rune) {
	if p.searching {
		switch key {
		case "enter":
			p.searching = false
		case "esc":
			p.searching, p.search, p.query.Search = false, newInput("", 200), ""
			p.load(m)
		default:
			if p.search.handle(key, typed) {
				p.query.Search = p.search.value()
				p.cursor = 0
				p.load(m)
			}
		}
		return
	}
	record, has := p.selected()
	switch key {
	case "up", "k":
		if p.cursor > 0 {
			p.cursor--
		}
	case "down", "j":
		if p.cursor < len(p.list)-1 {
			p.cursor++
		}
	case "pgup":
		p.cursor = max(0, p.cursor-10)
	case "pgdown":
		p.cursor = max(0, min(len(p.list)-1, p.cursor+10))
	case "home", "g":
		p.cursor = 0
	case "end", "G":
		p.cursor = max(0, len(p.list)-1)
	case "enter":
		p.detail = has && !p.detail
	case "esc":
		switch {
		case p.detail:
			p.detail = false
		case p.query.Search != "" || p.query.Status != "" || p.query.Severity != "":
			p.query.Search, p.query.Status, p.query.Severity, p.search = "", "", "", newInput("", 200)
			p.load(m)
		}
	case "/":
		p.searching, p.search = true, newInput(p.query.Search, 200)
	case "f":
		p.query.Status = next(append([]string{""}, records.Statuses...), p.query.Status)
		p.load(m)
	case "v":
		options, _ := severityOptions()
		p.query.Severity = theme.Severity(next(append([]string{""}, options...), string(p.query.Severity)))
		p.load(m)
	case "s":
		p.sortBy(m, next(records.SortKeys, p.query.Sort), p.query.Descending)
	case "S":
		p.sortBy(m, p.query.Sort, !p.query.Descending)
	case "c":
		p.columnForm(m)
	case "a":
		p.query.IncludeDisabled = !p.query.IncludeDisabled
		p.load(m)
	case "n":
		m.form = recordForm("Add record", records.Input{Status: records.StatusOpen, Severity: theme.SeverityInfo, Enabled: true}, func(in records.Input) error {
			created, err := m.app.Records.Create(m.ctx, in)
			if err != nil {
				return err
			}
			if !created.Enabled {
				p.query.IncludeDisabled = true
			}
			m.changed("Added record %d: %s", created.ID, created.Name)
			p.selectID(created.ID)
			return nil
		})
	case "e":
		if !has {
			return
		}
		m.form = recordForm(fmt.Sprintf("Edit record %d", record.ID), record.Input(), func(in records.Input) error {
			updated, err := m.app.Records.Update(m.ctx, record.ID, record.Revision, in)
			var conflict *records.ConflictError
			if errors.As(err, &conflict) {
				m.reload()
				return errors.New("another window changed this record while you were editing. Your text is still here: press Esc, press e again to load the new version, and repeat your change")
			}
			if err != nil {
				return err
			}
			m.changed("Updated record %d", updated.ID)
			return nil
		})
	case "x":
		if !has {
			return
		}
		updated, err := m.app.Records.SetEnabled(m.ctx, record.ID, !record.Enabled)
		if m.fail(err) {
			return
		}
		if !updated.Enabled {
			p.query.IncludeDisabled = true
		}
		m.changed("Record %d is %s", updated.ID, map[bool]string{true: "enabled", false: "disabled"}[updated.Enabled])
	case "d", "delete":
		if !has {
			return
		}
		m.ask("Remove record", fmt.Sprintf("This removes record %d (%s) permanently. Its notifications stay in the list without a link.", record.ID, record.Name), "remove it", func() (string, error) {
			if err := m.app.Records.Delete(m.ctx, record.ID); err != nil {
				return "", err
			}
			return fmt.Sprintf("Removed record %d", record.ID), nil
		})
	case "i":
		p.importForm(m)
	case "o":
		p.exportForm(m)
	}
}

func (p *recordsPage) keep(m *Model, key, raw, what string) error {
	saved, err := m.app.Settings.Set(m.ctx, key, raw)
	if err != nil {
		return err
	}
	m.reload()
	if saved.OverriddenBy != "" {
		m.say(noticeWarning, "%s saved, but the %s still takes precedence", what, saved.OverriddenBy)
	}
	return nil
}

func (p *recordsPage) sortBy(m *Model, key string, descending bool) {
	m.fail(p.keep(m, settings.KeyRecordSort, settings.JoinSort(key, descending), "Sort order"))
}

func (p *recordsPage) columnForm(m *Model) {
	value := m.values[settings.KeyRecordColumns]
	pinned := map[string]bool{}
	for _, column := range value.Pinned {
		pinned[column] = true
	}
	stored := settings.Value{Raw: value.Default}
	if value.HasStored {
		stored.Raw = value.Stored
	}
	var fields []field
	for _, column := range value.Options {
		if pinned[column] {
			continue
		}
		current := "hide"
		if stored.Shows(column) {
			current = "show"
		}
		fields = append(fields, choiceField(column, records.ColumnLabel(column), current, "", []string{"show", "hide"}, []string{"Shown", "Hidden"}))
	}
	m.form = &form{title: "Columns of the record table", fields: fields, submit: func(values map[string]string) error {
		var shown []string
		for _, column := range value.Options {
			if values[column] == "show" {
				shown = append(shown, column)
			}
		}
		return p.keep(m, settings.KeyRecordColumns, strings.Join(shown, ","), "Columns")
	}}
}

func next(options []string, current string) string {
	for index, option := range options {
		if option == current {
			return options[(index+1)%len(options)]
		}
	}
	return options[0]
}

func (p *recordsPage) importForm(m *Model) {
	m.form = &form{title: "Import records", fields: []field{
		textField("path", "File", "", "Path of a CSV or JSON file. Rows are matched by name: new names are added, known names are updated. Nothing is written until you confirm the preview.", 1024),
		choiceField("format", "Format", "", "", []string{"", records.FormatCSV, records.FormatJSON}, []string{"From the file name", "CSV", "JSON"}),
	}, submit: func(values map[string]string) error {
		path, format := strings.TrimSpace(values["path"]), values["format"]
		if path == "" {
			return errors.New("give the path of the file to import")
		}
		result, err := m.app.ImportFile(m.ctx, path, format, true)
		if err != nil {
			return err
		}
		lines := []string{"Dry run of " + path + ":", "", result.Summary(), ""}
		for _, problem := range result.Problems {
			lines = append(lines, "✖ rejected "+problem.Error())
		}
		if result.Created+result.Updated == 0 {
			lines = append(lines, "There is nothing to import.")
			m.preview = &preview{title: "Import preview", lines: lines}
			return nil
		}
		m.preview = &preview{title: "Import preview", lines: lines, accept: "import these rows", action: func() (string, error) {
			done, err := m.app.ImportFile(m.ctx, path, format, false)
			if err != nil {
				return "", err
			}
			p.query.IncludeDisabled = true
			return "Import finished: " + done.Summary(), nil
		}}
		return nil
	}}
}

func (p *recordsPage) exportForm(m *Model) {
	m.form = &form{title: "Export records", fields: []field{
		textField("path", "File", "records.csv", "Path of the file to write. Every record is exported, including disabled ones. The file is readable only by you.", 1024),
		choiceField("format", "Format", "", "", []string{"", records.FormatCSV, records.FormatJSON}, []string{"From the file name", "CSV", "JSON"}),
		choiceField("overwrite", "If the file exists", "false", "", []string{"false", "true"}, []string{"Stop and keep it", "Overwrite it"}),
	}, submit: func(values map[string]string) error {
		path := strings.TrimSpace(values["path"])
		count, err := m.app.ExportFile(m.ctx, path, values["format"], values["overwrite"] == "true")
		var exists *app.ExistsError
		if errors.As(err, &exists) {
			return errors.New(path + " already exists. Choose another name, or set \"If the file exists\" to \"Overwrite it\"")
		}
		if err != nil {
			return err
		}
		m.say(noticeSuccess, "Exported %d records to %s", count, path)
		return nil
	}}
}

func (p *recordsPage) filterLine(m *Model) string {
	direction := "ascending"
	if p.query.Descending {
		direction = "descending"
	}
	search := p.query.Search
	if p.searching {
		search = p.search.view(24, true)
	} else if search == "" {
		search = "none"
	}
	status, severity := "all", "all"
	if p.query.Status != "" {
		status = records.StatusLabel(p.query.Status)
	}
	if p.query.Severity != "" {
		severity = p.query.Severity.Label()
	}
	disabled := "hidden"
	if p.query.IncludeDisabled {
		disabled = "shown"
	}
	return fmt.Sprintf("Search: %s · Status: %s · Severity: %s · Disabled: %s · Sort: %s %s", strings.TrimRight(search, " "), status, severity, disabled, p.query.Sort, direction)
}

func (p *recordsPage) view(m *Model, width, height int) []string {
	var lines []string
	for _, line := range wrap(p.filterLine(m), width) {
		lines = append(lines, m.st.muted.Render(line))
	}
	lines = append(lines, "")
	if len(p.list) == 0 {
		message := "No records yet. Press n to add the first record, i to import a file, or 1 and then l to load demo data."
		if p.query.Search != "" || p.query.Status != "" || p.query.Severity != "" {
			message = "No records match these filters. Press Esc to clear them."
		} else if !p.query.IncludeDisabled && m.overview.Disabled > 0 {
			message = "Every record is disabled. Press a to show disabled records."
		}
		return append(lines, wrap(message, width)...)
	}
	chosen := m.values[settings.KeyRecordColumns]
	var columns []column
	for _, item := range []column{{"id", "", 5, false}, {"name", "", 16, true}, {"status", "", 13, false}, {"severity", "", 12, false}, {"updated", "", 16, false}, {"state", "", 8, false}} {
		if !chosen.Shows(item.key) || (width < 84 && item.key == "updated") {
			continue
		}
		item.title = strings.ToUpper(records.ColumnLabel(item.key))
		columns = append(columns, item)
	}
	widths := layoutColumns(columns, width-2)
	titles := make([]string, len(columns))
	for index, item := range columns {
		titles[index] = item.title
	}
	lines = append(lines, m.st.muted.Render("  "+renderRow(titles, widths)))
	detailHeight := 0
	if p.detail {
		detailHeight = 8
	}
	rows := height - len(lines) - detailHeight - 1
	if rows < 3 {
		rows = 3
	}
	if p.cursor < p.offset {
		p.offset = p.cursor
	}
	if p.cursor >= p.offset+rows {
		p.offset = p.cursor - rows + 1
	}
	if p.offset > max(0, len(p.list)-rows) {
		p.offset = max(0, len(p.list)-rows)
	}
	for index := p.offset; index < len(p.list) && index < p.offset+rows; index++ {
		record := p.list[index]
		cells := make([]string, 0, len(columns))
		for _, item := range columns {
			switch item.key {
			case "id":
				cells = append(cells, strconv.FormatInt(record.ID, 10))
			case "name":
				cells = append(cells, record.Name)
			case "status":
				cells = append(cells, records.StatusGlyph(record.Status)+" "+records.StatusLabel(record.Status))
			case "severity":
				cells = append(cells, record.Severity.Glyph()+" "+record.Severity.Label())
			case "updated":
				cells = append(cells, m.app.FormatTime(m.ctx, record.UpdatedAt))
			case "state":
				cells = append(cells, map[bool]string{true: "Enabled", false: "Disabled"}[record.Enabled])
			}
		}
		row := renderRow(cells, widths)
		if index == p.cursor {
			lines = append(lines, m.st.selected.Render(fit("▌ "+row, width)))
		} else {
			lines = append(lines, "  "+row)
		}
	}
	lines = append(lines, m.st.muted.Render(fmt.Sprintf("Row %d of %d", p.cursor+1, len(p.list))))
	if record, has := p.selected(); p.detail && has {
		lines = append(lines, m.st.border.Render(strings.Repeat("─", width)))
		lines = append(lines, m.st.heading.Render(fit(fmt.Sprintf("Record %d: %s", record.ID, record.Name), width)))
		lines = append(lines, fmt.Sprintf("Status %s %s · Severity %s · %s · Revision %d", records.StatusGlyph(record.Status), records.StatusLabel(record.Status), m.st.severity(record.Severity), map[bool]string{true: "Enabled", false: "Disabled"}[record.Enabled], record.Revision))
		lines = append(lines, m.st.muted.Render(fmt.Sprintf("Created %s · Updated %s", m.app.FormatTime(m.ctx, record.CreatedAt), m.app.FormatTime(m.ctx, record.UpdatedAt))))
		note := record.Note
		if note == "" {
			note = "No note."
		}
		noteLines := wrap(note, width)
		if len(noteLines) > 3 {
			noteLines = append(noteLines[:2], "… press e to read the whole note")
		}
		lines = append(lines, noteLines...)
	}
	return lines
}

type inboxPage struct {
	rules      bool
	unreadOnly bool
	list       []notify.Notification
	cursor     int
	ruleList   []notify.Rule
	ruleCursor int
}

func (p *inboxPage) load(m *Model) {
	var previous int64
	if p.cursor < len(p.list) {
		previous = p.list[p.cursor].ID
	}
	list, err := m.app.Notices.List(m.ctx, notify.Filter{UnreadOnly: p.unreadOnly, Limit: listLimit})
	if m.fail(err) {
		return
	}
	rules, err := m.app.Notices.Rules(m.ctx)
	if m.fail(err) {
		return
	}
	p.list, p.ruleList = list, rules
	for index, item := range list {
		if item.ID == previous {
			p.cursor = index
		}
	}
	p.cursor = max(0, min(p.cursor, len(list)-1))
	p.ruleCursor = max(0, min(p.ruleCursor, len(rules)-1))
}

func (p *inboxPage) key(m *Model, key string) {
	switch key {
	case "right", "]":
		p.rules = true
		return
	case "left", "[":
		p.rules = false
		return
	}
	if p.rules {
		p.ruleKey(m, key)
		return
	}
	switch key {
	case "up", "k":
		p.cursor = max(0, p.cursor-1)
	case "down", "j":
		p.cursor = max(0, min(len(p.list)-1, p.cursor+1))
	case "u":
		p.unreadOnly = !p.unreadOnly
		p.load(m)
	case "M":
		changed, err := m.app.Notices.MarkAllRead(m.ctx)
		if m.fail(err) {
			return
		}
		m.changed("Marked %d notifications as read", changed)
	case "m", "enter":
		if p.cursor >= len(p.list) {
			return
		}
		item := p.list[p.cursor]
		if item.Unread() {
			if m.fail(m.app.Notices.MarkRead(m.ctx, item.ID)) {
				return
			}
			m.changed("Marked notification %d as read", item.ID)
		}
		if key != "enter" {
			return
		}
		if item.RecordID == 0 {
			m.say(noticeInfo, "The record of this notification was removed")
			return
		}
		m.rec.query = records.Query{Sort: "updated", Descending: true, IncludeDisabled: true}
		m.rec.load(m)
		m.rec.selectID(item.RecordID)
		m.rec.detail = true
		m.open(app.NavRecords)
	}
}

func (p *inboxPage) ruleKey(m *Model, key string) {
	switch key {
	case "up", "k":
		p.ruleCursor = max(0, p.ruleCursor-1)
	case "down", "j":
		p.ruleCursor = max(0, min(len(p.ruleList)-1, p.ruleCursor+1))
	}
	if p.ruleCursor >= len(p.ruleList) {
		return
	}
	rule := p.ruleList[p.ruleCursor]
	switch key {
	case "u":
		if m.fail(m.app.Notices.ResetRule(m.ctx, rule.EventType)) {
			return
		}
		m.changed("Rule for %s is back to its defaults", rule.EventType)
	case "e", "enter":
		severities, labels := severityOptions()
		enabled := "true"
		if !rule.Enabled {
			enabled = "false"
		}
		m.form = &form{title: "Rule for " + eventLabel(rule.EventType), fields: []field{
			choiceField("enabled", "Enabled", enabled, "Off stops notifications of this event type.", []string{"true", "false"}, []string{"Yes", "No"}),
			choiceField("severity", "Minimum severity", string(rule.MinSeverity), "Events below this severity raise no notification.", severities, labels),
			textField("start", "Quiet hours start", rule.QuietStart, "Time of day as HH:MM. Leave both times empty for no quiet hours.", 5),
			textField("end", "Quiet hours end", rule.QuietEnd, "Time of day as HH:MM. May be earlier than the start to span midnight.", 5),
			textField("dedup", "De-duplication", strconv.Itoa(rule.DedupMinutes), "Minutes in which the same event for the same record is not raised again. 0 turns it off.", 5),
		}, submit: func(values map[string]string) error {
			minutes, err := strconv.Atoi(strings.TrimSpace(values["dedup"]))
			if err != nil {
				return errors.New("de-duplication needs a whole number of minutes, for example 60")
			}
			changed := notify.Rule{EventType: rule.EventType, Enabled: values["enabled"] == "true", MinSeverity: theme.Severity(values["severity"]), QuietStart: strings.TrimSpace(values["start"]), QuietEnd: strings.TrimSpace(values["end"]), DedupMinutes: minutes}
			if err := m.app.Notices.SetRule(m.ctx, changed); err != nil {
				return err
			}
			m.changed("Saved the rule for %s", rule.EventType)
			return nil
		}}
	}
}

func eventLabel(id string) string {
	for _, event := range notify.EventTypes() {
		if event.ID == id {
			return event.Label
		}
	}
	return id
}

func (p *inboxPage) view(m *Model, width, height int) []string {
	inbox, rules := "  Inbox  ", "  Rules  "
	if p.rules {
		rules = m.st.selected.Render("▌ Rules ")
	} else {
		inbox = m.st.selected.Render("▌ Inbox ")
	}
	lines := []string{inbox + " " + rules, ""}
	if p.rules {
		widths := layoutColumns([]column{{"", "", 22, true}, {"", "", 7, false}, {"", "", 12, false}, {"", "", 11, false}, {"", "", 7, false}, {"", "", 7, false}}, width-2)
		lines = append(lines, m.st.muted.Render("  "+renderRow([]string{"EVENT", "ENABLED", "MIN SEVERITY", "QUIET HOURS", "DEDUP", "SOURCE"}, widths)))
		for index, rule := range p.ruleList {
			quiet := "none"
			if rule.QuietStart != "" {
				quiet = rule.QuietStart + "-" + rule.QuietEnd
			}
			source := "default"
			if rule.Customized {
				source = "changed"
			}
			row := renderRow([]string{eventLabel(rule.EventType), map[bool]string{true: "Yes", false: "No"}[rule.Enabled], rule.MinSeverity.Glyph() + " " + rule.MinSeverity.Label(), quiet, fmt.Sprintf("%d min", rule.DedupMinutes), source}, widths)
			if index == p.ruleCursor {
				lines = append(lines, m.st.selected.Render(fit("▌ "+row, width)))
			} else {
				lines = append(lines, "  "+row)
			}
		}
		if p.ruleCursor < len(p.ruleList) {
			for _, event := range notify.EventTypes() {
				if event.ID == p.ruleList[p.ruleCursor].EventType {
					lines = append(lines, "")
					lines = append(lines, wrap(event.Description, width)...)
				}
			}
		}
		return lines
	}
	filter := "all notifications"
	if p.unreadOnly {
		filter = "unread only"
	}
	lines = append(lines, m.st.muted.Render(fmt.Sprintf("Showing %s · %d unread", filter, m.overview.UnreadNotifications)), "")
	if len(p.list) == 0 {
		message := "No notifications. They appear when a record needs attention or is completed."
		if p.unreadOnly {
			message = "No unread notifications. Press u to show all of them."
		}
		return append(lines, wrap(message, width)...)
	}
	widths := layoutColumns([]column{{"", "", 4, false}, {"", "", 16, false}, {"", "", 12, false}, {"", "", 6, false}, {"", "", 20, true}, {"", "", 10, false}}, width-2)
	lines = append(lines, m.st.muted.Render("  "+renderRow([]string{"ID", "WHEN", "SEVERITY", "STATE", "TITLE", "LINK"}, widths)))
	rows := max(3, height-len(lines)-1)
	shown, offset := window(make([]string, len(p.list)), max(0, p.cursor-rows+1), rows)
	for index := offset; index < offset+len(shown); index++ {
		item := p.list[index]
		state, link := "Unread", "-"
		if !item.Unread() {
			state = "Read"
		}
		if item.RecordID != 0 {
			link = fmt.Sprintf("%s %d", item.RecordKind, item.RecordID)
		}
		row := renderRow([]string{strconv.FormatInt(item.ID, 10), m.app.FormatTime(m.ctx, item.CreatedAt), item.Severity.Glyph() + " " + item.Severity.Label(), state, item.Title, link}, widths)
		switch {
		case index == p.cursor:
			lines = append(lines, m.st.selected.Render(fit("▌ "+row, width)))
		case item.Unread():
			lines = append(lines, "● "+row)
		default:
			lines = append(lines, "  "+row)
		}
	}
	return lines
}
