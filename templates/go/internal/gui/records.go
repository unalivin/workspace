package gui

import (
	"errors"
	"fmt"
	"strconv"
	"strings"

	"fyne.io/fyne/v2"
	"fyne.io/fyne/v2/container"
	"fyne.io/fyne/v2/widget"

	"github.com/unalivin/templateapp/internal/app"
	"github.com/unalivin/templateapp/internal/notify"
	"github.com/unalivin/templateapp/internal/records"
	"github.com/unalivin/templateapp/internal/settings"
	"github.com/unalivin/templateapp/internal/theme"
)

const listLimit = 1000

func severityChoices() ([]string, []string) {
	var options, labels []string
	for _, level := range theme.Severities {
		options = append(options, string(level))
		labels = append(labels, level.Label())
	}
	return options, labels
}

func statusChoices() ([]string, []string) {
	var labels []string
	for _, status := range records.Statuses {
		labels = append(labels, records.StatusLabel(status))
	}
	return records.Statuses, labels
}

func enabledText(enabled bool) string {
	if enabled {
		return "Enabled"
	}
	return "Disabled"
}

type recordsPage struct {
	d        *dashboard
	root     fyne.CanvasObject
	query    records.Query
	rows     []records.Record
	selected int64
	list     *widget.List
	layout   *columns
	header   fyne.CanvasObject
	sortKeys map[string]string
	search   *widget.Entry
	status   *widget.Select
	severity *widget.Select
	sort     *widget.Select
	disabled *widget.Check
	reverse  *widget.Check
	empty    *widget.Label
	count    *widget.Label
	detail   *fyne.Container
	name     *widget.Label
	facts    *widget.Label
	note     *widget.Label
	toggle   *widget.Button
	table    fyne.CanvasObject
	side     fyne.CanvasObject
	body     *fyne.Container
	loading  bool
}

func newRecordsPage(d *dashboard) *recordsPage {
	p := &recordsPage{d: d, query: records.Query{Sort: "updated", Descending: true}}
	p.layout = &columns{weights: []float32{0, 3, 0, 0, 0, 0}, fixed: []float32{48, 120, 130, 120, 140, 80}}
	reload := func() {
		if !p.loading {
			p.refresh()
		}
	}
	p.search = widget.NewEntry()
	p.search.SetPlaceHolder("Search names and notes")
	p.search.OnChanged = func(text string) {
		p.query.Search = text
		reload()
	}
	statuses, statusLabels := statusChoices()
	p.status = widget.NewSelect(append([]string{"All statuses"}, statusLabels...), func(label string) {
		p.query.Status = ""
		for index, known := range statusLabels {
			if known == label {
				p.query.Status = statuses[index]
			}
		}
		reload()
	})
	severities, severityLabels := severityChoices()
	p.severity = widget.NewSelect(append([]string{"All severities"}, severityLabels...), func(label string) {
		p.query.Severity = ""
		for index, known := range severityLabels {
			if known == label {
				p.query.Severity = theme.Severity(severities[index])
			}
		}
		reload()
	})
	p.sortKeys = map[string]string{"name": "Sort by name", "status": "Sort by status", "severity": "Sort by severity", "updated": "Sort by update"}
	var sortOptions []string
	for _, key := range records.SortKeys {
		sortOptions = append(sortOptions, p.sortKeys[key])
	}
	p.sort = widget.NewSelect(sortOptions, func(label string) {
		for key, known := range p.sortKeys {
			if known == label && !p.loading {
				p.keep(settings.KeyRecordSort, settings.JoinSort(key, p.query.Descending), "Sort order")
			}
		}
	})
	p.reverse = widget.NewCheck("Descending", func(on bool) {
		if !p.loading {
			p.keep(settings.KeyRecordSort, settings.JoinSort(p.query.Sort, on), "Sort order")
		}
	})
	p.disabled = widget.NewCheck("Show disabled", func(on bool) {
		p.query.IncludeDisabled = on
		reload()
	})
	p.loading = true
	p.status.SetSelectedIndex(0)
	p.severity.SetSelectedIndex(0)
	p.sort.SetSelected(p.sortKeys["updated"])
	p.reverse.SetChecked(true)
	p.loading = false
	add := widget.NewButtonWithIcon("Add record", glyph("plus"), p.add)
	add.Importance = widget.HighImportance
	importButton := widget.NewButtonWithIcon("Import", glyph("upload"), p.importFile)
	exportButton := widget.NewButtonWithIcon("Export", glyph("download"), p.exportFile)
	columnsButton := widget.NewButtonWithIcon("Columns", glyph("columns"), p.chooseColumns)
	filters := container.NewHBox(p.status, p.severity, p.sort, p.reverse, p.disabled)
	toolbar := container.NewVBox(container.NewBorder(nil, nil, nil, container.NewHBox(add, importButton, exportButton, columnsButton), p.search), filters)
	p.list = widget.NewList(
		func() int { return len(p.rows) },
		func() fyne.CanvasObject {
			status, _, _ := iconLabel(icon("status-open", d.tokens.Palette.TextMuted), "")
			severity, _, _ := iconLabel(icon("info", d.tokens.Palette.TextMuted), "")
			return container.New(p.layout, cell(""), cell(""), status, severity, cell(""), cell(""))
		},
		func(id widget.ListItemID, object fyne.CanvasObject) {
			if id >= len(p.rows) {
				return
			}
			record := p.rows[id]
			cells := object.(*fyne.Container).Objects
			cells[0].(*widget.Label).SetText(strconv.FormatInt(record.ID, 10))
			cells[1].(*widget.Label).SetText(record.Name)
			status := cells[2].(*fyne.Container)
			status.Objects[0].(*widget.Label).SetText(records.StatusLabel(record.Status))
			status.Objects[1].(*widget.Icon).SetResource(icon(statusIcon(record.Status), d.tokens.Palette.TextMuted))
			severity := cells[3].(*fyne.Container)
			severity.Objects[0].(*widget.Label).SetText(record.Severity.Label())
			severity.Objects[1].(*widget.Icon).SetResource(icon(record.Severity.Icon(), d.tokens.Semantic.Severity(record.Severity)))
			cells[4].(*widget.Label).SetText(d.app.FormatTime(d.ctx, record.UpdatedAt))
			cells[5].(*widget.Label).SetText(enabledText(record.Enabled))
			for index, item := range cells {
				if item.Visible() != p.layout.shows(index) {
					object.Refresh()
					break
				}
			}
		})
	p.list.OnSelected = func(id widget.ListItemID) {
		if id < len(p.rows) {
			p.selected = p.rows[id].ID
			p.showDetail()
		}
	}
	p.empty = wrapped("")
	p.count = muted("")
	p.name = heading("")
	p.name.Wrapping = fyne.TextWrapWord
	p.facts = wrapped("")
	p.note = wrapped("")
	edit := widget.NewButtonWithIcon("Edit", glyph("pencil"), p.edit)
	p.toggle = widget.NewButton("Disable", p.toggleEnabled)
	removeButton := widget.NewButtonWithIcon("Remove", glyph("trash"), p.remove)
	removeButton.Importance = widget.DangerImportance
	copyButton := widget.NewButtonWithIcon("Copy", glyph("copy"), p.copy)
	closeButton := widget.NewButtonWithIcon("", glyph("x"), p.unselect)
	closeButton.Importance = widget.LowImportance
	p.detail = container.NewBorder(container.NewBorder(nil, nil, nil, closeButton, p.name), container.NewHBox(edit, p.toggle, copyButton, removeButton), nil, nil, container.NewVScroll(container.NewVBox(p.facts, widget.NewSeparator(), p.note)))
	p.detail.Hide()
	var titles []string
	for _, column := range settings.RecordColumns {
		titles = append(titles, records.ColumnLabel(column))
	}
	p.header = headerRow(p.layout, titles...)
	table := container.NewBorder(p.header, p.count, nil, nil, container.NewStack(p.list, container.NewVBox(p.empty)))
	p.table = table
	p.side = panel(p.detail)
	p.body = container.NewStack(p.table)
	p.root = container.NewBorder(toolbar, nil, nil, nil, p.body)
	return p
}

func (p *recordsPage) object() fyne.CanvasObject { return p.root }

func (p *recordsPage) current() (records.Record, bool) {
	for _, record := range p.rows {
		if record.ID == p.selected {
			return record, true
		}
	}
	return records.Record{}, false
}

func (p *recordsPage) keep(key, raw, what string) {
	saved, err := p.d.app.Settings.Set(p.d.ctx, key, raw)
	if p.d.fail(err) {
		return
	}
	p.d.refresh()
	if saved.OverriddenBy != "" {
		p.d.say(theme.SeverityHigh, "%s saved, but the %s still takes precedence", what, saved.OverriddenBy)
	}
}

func (p *recordsPage) preferences() {
	if key, descending := p.d.values[settings.KeyRecordSort].Sort(); key != "" {
		p.query.Sort, p.query.Descending = key, descending
		p.loading = true
		p.sort.SetSelected(p.sortKeys[key])
		p.reverse.SetChecked(descending)
		p.loading = false
	}
	chosen := p.d.values[settings.KeyRecordColumns]
	hidden := make([]bool, len(settings.RecordColumns))
	changed := len(p.layout.hidden) != len(hidden)
	for index, column := range settings.RecordColumns {
		hidden[index] = !chosen.Shows(column)
		changed = changed || p.layout.hidden[index] != hidden[index]
	}
	if changed {
		p.layout.hidden = hidden
		p.header.Refresh()
	}
}

func (p *recordsPage) chooseColumns() {
	value := p.d.values[settings.KeyRecordColumns]
	pinned := map[string]bool{}
	for _, column := range value.Pinned {
		pinned[column] = true
	}
	stored := settings.Value{Raw: value.Default}
	if value.HasStored {
		stored.Raw = value.Stored
	}
	var fields []formField
	for _, column := range value.Options {
		if pinned[column] {
			continue
		}
		current := "hide"
		if stored.Shows(column) {
			current = "show"
		}
		fields = append(fields, formField{key: column, label: records.ColumnLabel(column), value: current, options: []string{"show", "hide"}, labels: []string{"Shown", "Hidden"}})
	}
	p.d.showForm("Columns of the record table", "Save", fields, func(values map[string]string) error {
		var shown []string
		for _, column := range value.Options {
			if values[column] == "show" {
				shown = append(shown, column)
			}
		}
		saved, err := p.d.app.Settings.Set(p.d.ctx, settings.KeyRecordColumns, strings.Join(shown, ","))
		if err != nil {
			return err
		}
		p.d.done("Saved the columns of the record table")
		if saved.OverriddenBy != "" {
			p.d.say(theme.SeverityHigh, "Columns saved, but the %s still takes precedence", saved.OverriddenBy)
		}
		return nil
	})
}

func (p *recordsPage) copy() {
	record, found := p.current()
	if !found {
		return
	}
	text := fmt.Sprintf("Record %d: %s\nStatus: %s\nSeverity: %s\nState: %s\nUpdated: %s", record.ID, record.Name, records.StatusLabel(record.Status), record.Severity.Label(), enabledText(record.Enabled), p.d.app.FormatTime(p.d.ctx, record.UpdatedAt))
	if record.Note != "" {
		text += "\nNote: " + record.Note
	}
	p.d.fyneApp.Clipboard().SetContent(text)
	p.d.say(theme.SeverityLow, "Copied record %d to the clipboard", record.ID)
}

func (p *recordsPage) refresh() {
	p.preferences()
	query := p.query
	query.Limit = listLimit
	rows, err := p.d.app.Records.List(p.d.ctx, query)
	if p.d.fail(err) {
		return
	}
	p.rows = rows
	p.list.Refresh()
	p.count.SetText(fmt.Sprintf("%d records shown", len(rows)))
	switch {
	case len(rows) > 0:
		p.empty.Hide()
	case query.Search != "" || query.Status != "" || query.Severity != "":
		p.empty.SetText("No records match these filters. Clear the search box and the filters to see every record.")
		p.empty.Show()
	case !query.IncludeDisabled && p.d.overview.Disabled > 0:
		p.empty.SetText("Every record is disabled. Select Show disabled to see them.")
		p.empty.Show()
	default:
		p.empty.SetText("No records yet. Choose Add record to create the first one, Import to read a CSV or JSON file, or load demo data on the Overview.")
		p.empty.Show()
	}
	found := false
	for index, record := range rows {
		if record.ID == p.selected {
			found = true
			p.list.Select(index)
		}
	}
	if !found {
		p.unselect()
		return
	}
	p.showDetail()
}

func (p *recordsPage) selectID(id int64) {
	p.selected = id
	for index, record := range p.rows {
		if record.ID == id {
			p.list.Select(index)
			p.list.ScrollTo(index)
		}
	}
	p.showDetail()
}

func (p *recordsPage) unselect() {
	p.selected = 0
	p.list.UnselectAll()
	if p.detail.Visible() {
		p.detail.Hide()
		p.body.Objects = []fyne.CanvasObject{p.table}
		p.body.Refresh()
	}
}

func (p *recordsPage) showDetail() {
	record, found := p.current()
	if !found {
		return
	}
	p.name.SetText(record.Name)
	p.facts.SetText(fmt.Sprintf("Record %d\nStatus: %s\nSeverity: %s\nState: %s\nCreated: %s\nUpdated: %s\nRevision: %d", record.ID, records.StatusLabel(record.Status), record.Severity.Label(), enabledText(record.Enabled), p.d.app.FormatTime(p.d.ctx, record.CreatedAt), p.d.app.FormatTime(p.d.ctx, record.UpdatedAt), record.Revision))
	if record.Note == "" {
		p.note.SetText("No note.")
	} else {
		p.note.SetText(record.Note)
	}
	if record.Enabled {
		p.toggle.SetText("Disable")
	} else {
		p.toggle.SetText("Enable")
	}
	if !p.detail.Visible() {
		p.detail.Show()
		split := container.NewHSplit(p.table, p.side)
		split.SetOffset(0.64)
		p.body.Objects = []fyne.CanvasObject{split}
		p.body.Refresh()
	}
}

func recordFields(in records.Input) []formField {
	severities, severityLabels := severityChoices()
	statuses, statusLabels := statusChoices()
	return []formField{
		{key: "name", label: "Name", value: in.Name, hint: "Required, unique, at most " + strconv.Itoa(records.MaxNameLength) + " characters."},
		{key: "status", label: "Status", value: in.Status, options: statuses, labels: statusLabels},
		{key: "severity", label: "Severity", value: string(in.Severity), options: severities, labels: severityLabels, hint: "High and critical open records raise a notification."},
		{key: "note", label: "Note", value: in.Note, multiline: true, hint: "Optional free text, searchable."},
		{key: "enabled", label: "Enabled", value: strconv.FormatBool(in.Enabled), options: []string{"true", "false"}, labels: []string{"Yes", "No"}, hint: "Disabled records are kept but not counted."},
	}
}

func recordInput(values map[string]string) records.Input {
	return records.Input{Name: values["name"], Status: values["status"], Severity: theme.Severity(values["severity"]), Note: values["note"], Enabled: values["enabled"] == "true"}
}

func (p *recordsPage) add() {
	p.d.showForm("Add record", "Add record", recordFields(records.Input{Status: records.StatusOpen, Severity: theme.SeverityInfo, Enabled: true}), func(values map[string]string) error {
		created, err := p.d.app.Records.Create(p.d.ctx, recordInput(values))
		if err != nil {
			return err
		}
		if !created.Enabled {
			p.disabled.SetChecked(true)
		}
		p.selected = created.ID
		p.d.done("Added record %d: %s", created.ID, created.Name)
		return nil
	})
}

func (p *recordsPage) edit() {
	record, found := p.current()
	if !found {
		return
	}
	p.d.showForm(fmt.Sprintf("Edit record %d", record.ID), "Save", recordFields(record.Input()), func(values map[string]string) error {
		updated, err := p.d.app.Records.Update(p.d.ctx, record.ID, record.Revision, recordInput(values))
		var conflict *records.ConflictError
		if errors.As(err, &conflict) {
			p.d.refresh()
			return errors.New("another window changed this record while you were editing. Your text is still here: copy what you need, choose Cancel, open Edit again to load the new version and repeat your change")
		}
		if err != nil {
			return err
		}
		p.d.done("Updated record %d", updated.ID)
		return nil
	})
}

func (p *recordsPage) toggleEnabled() {
	record, found := p.current()
	if !found {
		return
	}
	updated, err := p.d.app.Records.SetEnabled(p.d.ctx, record.ID, !record.Enabled)
	if p.d.fail(err) {
		return
	}
	if !updated.Enabled {
		p.disabled.SetChecked(true)
	}
	p.d.done("Record %d is %s", updated.ID, strings.ToLower(enabledText(updated.Enabled)))
}

func (p *recordsPage) remove() {
	record, found := p.current()
	if !found {
		return
	}
	p.d.ask("Remove record", fmt.Sprintf("This removes record %d (%s) permanently. Its notifications stay in the list without a link.", record.ID, record.Name), "Remove", func() (string, error) {
		if err := p.d.app.Records.Delete(p.d.ctx, record.ID); err != nil {
			return "", err
		}
		return fmt.Sprintf("Removed record %d", record.ID), nil
	})
}

func formatField() formField {
	return formField{key: "format", label: "Format", options: []string{"", records.FormatCSV, records.FormatJSON}, labels: []string{"From the file name", "CSV", "JSON"}}
}

func (p *recordsPage) importFile() {
	p.d.showForm("Import records", "Preview", []formField{
		{key: "path", label: "File", browse: "open", hint: "A CSV or JSON file. Rows are matched by name: new names are added, known names are updated. Nothing is written until you confirm the preview."},
		formatField(),
	}, func(values map[string]string) error {
		path, format := strings.TrimSpace(values["path"]), values["format"]
		if path == "" {
			return errors.New("choose the file to import")
		}
		result, err := p.d.app.ImportFile(p.d.ctx, path, format, true)
		if err != nil {
			return err
		}
		var text strings.Builder
		fmt.Fprintf(&text, "Dry run of %s:\n\n%s\n", path, result.Summary())
		for _, problem := range result.Problems {
			text.WriteString("\nRejected " + problem.Error())
		}
		if result.Created+result.Updated == 0 {
			text.WriteString("\n\nThere is nothing to import.")
			p.d.confirm = newConfirmDialog(p.d.window, "Import preview", text.String(), "Close", false, func() {}, func() { p.d.confirm = nil })
			return nil
		}
		p.d.proceed("Import preview", text.String(), "Import these rows", func() (string, error) {
			done, err := p.d.app.ImportFile(p.d.ctx, path, format, false)
			if err != nil {
				return "", err
			}
			p.disabled.SetChecked(true)
			return "Import finished: " + done.Summary(), nil
		})
		return nil
	})
}

func (p *recordsPage) exportFile() {
	p.d.showForm("Export records", "Export", []formField{
		{key: "path", label: "File", browse: "save", hint: "Every record is exported, including disabled ones. The file is readable only by your user account."},
		formatField(),
		{key: "overwrite", label: "If the file exists", value: "false", options: []string{"false", "true"}, labels: []string{"Stop and keep it", "Overwrite it"}},
	}, func(values map[string]string) error {
		path := strings.TrimSpace(values["path"])
		count, err := p.d.app.ExportFile(p.d.ctx, path, values["format"], values["overwrite"] == "true")
		var exists *app.ExistsError
		if errors.As(err, &exists) {
			return errors.New(path + " already exists. Choose another name, or set \"If the file exists\" to \"Overwrite it\"")
		}
		if err != nil {
			return err
		}
		p.d.say(theme.SeverityLow, "Exported %d records to %s", count, path)
		return nil
	})
}

type inboxPage struct {
	d          *dashboard
	root       fyne.CanvasObject
	tabs       *container.AppTabs
	items      []notify.Notification
	rules      []notify.Rule
	list       *widget.List
	ruleList   *widget.List
	selected   int64
	rule       string
	unreadOnly *widget.Check
	summary    *widget.Label
	empty      *widget.Label
	about      *widget.Label
}

func eventLabel(id string) string {
	for _, event := range notify.EventTypes() {
		if event.ID == id {
			return event.Label
		}
	}
	return id
}

func newInboxPage(d *dashboard) *inboxPage {
	p := &inboxPage{d: d}
	layout := &columns{weights: []float32{0, 0, 3, 0, 0}, fixed: []float32{140, 120, 160, 70, 90}}
	p.list = widget.NewList(
		func() int { return len(p.items) },
		func() fyne.CanvasObject {
			severity, _, _ := iconLabel(icon("info", d.tokens.Palette.TextMuted), "")
			return container.New(layout, cell(""), severity, cell(""), cell(""), cell(""))
		},
		func(id widget.ListItemID, object fyne.CanvasObject) {
			if id >= len(p.items) {
				return
			}
			item := p.items[id]
			cells := object.(*fyne.Container).Objects
			when := cells[0].(*widget.Label)
			title := cells[2].(*widget.Label)
			when.SetText(d.app.FormatTime(d.ctx, item.CreatedAt))
			severity := cells[1].(*fyne.Container)
			severity.Objects[0].(*widget.Label).SetText(item.Severity.Label())
			severity.Objects[1].(*widget.Icon).SetResource(icon(item.Severity.Icon(), d.tokens.Semantic.Severity(item.Severity)))
			title.TextStyle = fyne.TextStyle{Bold: item.Unread()}
			title.SetText(item.Title)
			state, link := "Unread", "-"
			if !item.Unread() {
				state = "Read"
			}
			if item.RecordID != 0 {
				link = fmt.Sprintf("%s %d", item.RecordKind, item.RecordID)
			}
			cells[3].(*widget.Label).SetText(state)
			cells[4].(*widget.Label).SetText(link)
		})
	p.list.OnSelected = func(id widget.ListItemID) {
		if id < len(p.items) {
			p.selected = p.items[id].ID
		}
	}
	p.unreadOnly = widget.NewCheck("Unread only", func(bool) { p.refresh() })
	p.summary = muted("")
	p.empty = wrapped("")
	markRead := widget.NewButtonWithIcon("Mark as read", glyph("check"), p.markRead)
	markAll := widget.NewButton("Mark all as read", p.markAll)
	openRecord := widget.NewButtonWithIcon("Open record", glyph("arrow-right"), p.openRecord)
	inbox := container.NewBorder(
		container.NewVBox(container.NewHBox(p.unreadOnly, markRead, markAll, openRecord), headerRow(layout, "When", "Severity", "Title", "State", "Link")),
		p.summary, nil, nil, container.NewStack(p.list, container.NewVBox(p.empty)))
	ruleLayout := &columns{weights: []float32{2, 0, 0, 0, 0, 0}, fixed: []float32{160, 80, 130, 130, 110, 80}}
	p.ruleList = widget.NewList(
		func() int { return len(p.rules) },
		func() fyne.CanvasObject {
			return container.New(ruleLayout, cell(""), cell(""), cell(""), cell(""), cell(""), cell(""))
		},
		func(id widget.ListItemID, object fyne.CanvasObject) {
			if id >= len(p.rules) {
				return
			}
			rule := p.rules[id]
			quiet, source, enabled := "None", "Default", "Yes"
			if rule.QuietStart != "" {
				quiet = rule.QuietStart + " to " + rule.QuietEnd
			}
			if rule.Customized {
				source = "Changed"
			}
			if !rule.Enabled {
				enabled = "No"
			}
			cells := object.(*fyne.Container).Objects
			for index, text := range []string{eventLabel(rule.EventType), enabled, rule.MinSeverity.Label(), quiet, fmt.Sprintf("%d minutes", rule.DedupMinutes), source} {
				cells[index].(*widget.Label).SetText(text)
			}
		})
	p.about = muted("Select an event type to change its rule.")
	p.ruleList.OnSelected = func(id widget.ListItemID) {
		if id >= len(p.rules) {
			return
		}
		p.rule = p.rules[id].EventType
		for _, event := range notify.EventTypes() {
			if event.ID == p.rule {
				p.about.SetText(event.Description)
			}
		}
	}
	editRule := widget.NewButtonWithIcon("Edit rule", glyph("pencil"), p.editRule)
	resetRule := widget.NewButton("Reset", p.resetRule)
	rules := container.NewBorder(
		container.NewVBox(container.NewHBox(editRule, resetRule), headerRow(ruleLayout, "Event", "Enabled", "Minimum severity", "Quiet hours", "De-duplication", "Source")),
		p.about, nil, nil, p.ruleList)
	p.tabs = container.NewAppTabs(container.NewTabItem("Inbox", inbox), container.NewTabItem("Rules", rules))
	p.root = p.tabs
	return p
}

func (p *inboxPage) object() fyne.CanvasObject { return p.root }

func (p *inboxPage) refresh() {
	items, err := p.d.app.Notices.List(p.d.ctx, notify.Filter{UnreadOnly: p.unreadOnly.Checked, Limit: listLimit})
	if p.d.fail(err) {
		return
	}
	rules, err := p.d.app.Notices.Rules(p.d.ctx)
	if p.d.fail(err) {
		return
	}
	p.items, p.rules = items, rules
	p.list.Refresh()
	p.ruleList.Refresh()
	p.summary.SetText(fmt.Sprintf("%d shown, %d unread", len(items), p.d.overview.UnreadNotifications))
	switch {
	case len(items) > 0:
		p.empty.Hide()
	case p.unreadOnly.Checked:
		p.empty.SetText("No unread notifications. Clear Unread only to see all of them.")
		p.empty.Show()
	default:
		p.empty.SetText("No notifications. They appear when a record needs attention or is completed.")
		p.empty.Show()
	}
	found := false
	for index, item := range items {
		if item.ID == p.selected {
			found = true
			p.list.Select(index)
		}
	}
	if !found {
		p.selected = 0
		p.list.UnselectAll()
	}
}

func (p *inboxPage) chosen() (notify.Notification, bool) {
	for _, item := range p.items {
		if item.ID == p.selected {
			return item, true
		}
	}
	p.d.say(theme.SeverityInfo, "Select a notification first")
	return notify.Notification{}, false
}

func (p *inboxPage) markRead() {
	item, found := p.chosen()
	if !found {
		return
	}
	if p.d.fail(p.d.app.Notices.MarkRead(p.d.ctx, item.ID)) {
		return
	}
	p.d.done("Marked the notification as read")
}

func (p *inboxPage) markAll() {
	changed, err := p.d.app.Notices.MarkAllRead(p.d.ctx)
	if p.d.fail(err) {
		return
	}
	p.d.done("Marked %d notifications as read", changed)
}

func (p *inboxPage) openRecord() {
	item, found := p.chosen()
	if !found {
		return
	}
	if item.RecordID == 0 {
		p.d.say(theme.SeverityInfo, "The record of this notification was removed")
		return
	}
	if item.Unread() {
		if p.d.fail(p.d.app.Notices.MarkRead(p.d.ctx, item.ID)) {
			return
		}
	}
	records := p.d.recordsP
	records.loading = true
	records.search.SetText("")
	records.status.SetSelectedIndex(0)
	records.severity.SetSelectedIndex(0)
	records.disabled.SetChecked(true)
	records.loading = false
	records.query.Search, records.query.Status, records.query.Severity, records.query.IncludeDisabled = "", "", "", true
	records.selected = item.RecordID
	p.d.refresh()
	p.d.show(app.NavRecords)
	records.selectID(item.RecordID)
}

func (p *inboxPage) chosenRule() (notify.Rule, bool) {
	for _, rule := range p.rules {
		if rule.EventType == p.rule {
			return rule, true
		}
	}
	p.d.say(theme.SeverityInfo, "Select an event type first")
	return notify.Rule{}, false
}

func (p *inboxPage) editRule() {
	rule, found := p.chosenRule()
	if !found {
		return
	}
	severities, labels := severityChoices()
	p.d.showForm("Rule for "+eventLabel(rule.EventType), "Save", []formField{
		{key: "enabled", label: "Enabled", value: strconv.FormatBool(rule.Enabled), options: []string{"true", "false"}, labels: []string{"Yes", "No"}, hint: "Off stops notifications of this event type."},
		{key: "severity", label: "Minimum severity", value: string(rule.MinSeverity), options: severities, labels: labels, hint: "Events below this severity raise no notification."},
		{key: "start", label: "Quiet hours start", value: rule.QuietStart, hint: "Time of day as HH:MM. Leave both times empty for no quiet hours."},
		{key: "end", label: "Quiet hours end", value: rule.QuietEnd, hint: "Time of day as HH:MM. May be earlier than the start to span midnight."},
		{key: "dedup", label: "De-duplication", value: strconv.Itoa(rule.DedupMinutes), hint: "Minutes in which the same event for the same record is not raised again. 0 turns it off."},
	}, func(values map[string]string) error {
		minutes, err := strconv.Atoi(strings.TrimSpace(values["dedup"]))
		if err != nil {
			return errors.New("de-duplication needs a whole number of minutes, for example 60")
		}
		changed := notify.Rule{EventType: rule.EventType, Enabled: values["enabled"] == "true", MinSeverity: theme.Severity(values["severity"]), QuietStart: strings.TrimSpace(values["start"]), QuietEnd: strings.TrimSpace(values["end"]), DedupMinutes: minutes}
		if err := p.d.app.Notices.SetRule(p.d.ctx, changed); err != nil {
			return err
		}
		p.d.done("Saved the rule for %s", eventLabel(rule.EventType))
		return nil
	})
}

func (p *inboxPage) resetRule() {
	rule, found := p.chosenRule()
	if !found {
		return
	}
	if p.d.fail(p.d.app.Notices.ResetRule(p.d.ctx, rule.EventType)) {
		return
	}
	p.d.done("The rule for %s is back to its defaults", eventLabel(rule.EventType))
}
