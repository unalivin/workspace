package tui

import (
	"fmt"
	"strings"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"

	"github.com/NexzaDev/certwatch/internal/app"
	"github.com/NexzaDev/certwatch/internal/theme"
)

type actionDoneMsg struct {
	view    string
	outcome app.Outcome
	err     error
}

type housekeepMsg struct {
	done app.Housekeeping
	err  error
}

type viewPage struct {
	view      app.View
	query     app.ViewQuery
	table     app.Table
	cursor    int
	offset    int
	detail    *app.Detail
	detailAt  int
	searching bool
	search    input
}

func newViewPage(view app.View) *viewPage {
	return &viewPage{view: view, query: app.ViewQuery{Filter: view.Filters[0].ID}, search: newInput("", 200)}
}

func (p *viewPage) selected() (app.Row, bool) {
	if p.cursor < 0 || p.cursor >= len(p.table.Rows) {
		return app.Row{}, false
	}
	return p.table.Rows[p.cursor], true
}

func (p *viewPage) selectID(id int64) bool {
	for index, row := range p.table.Rows {
		if row.ID == id {
			p.cursor = index
			return true
		}
	}
	return false
}

func (p *viewPage) load(m *Model) {
	previous, had := p.selected()
	table, err := m.app.Table(m.ctx, p.view.ID, p.query)
	if m.fail(err) {
		return
	}
	p.table = table
	if !had || !p.selectID(previous.ID) {
		p.cursor = max(0, min(p.cursor, len(table.Rows)-1))
	}
	if p.detail == nil {
		return
	}
	row, has := p.selected()
	if !has || (had && row.ID != previous.ID) {
		p.detail = nil
		return
	}
	detail, err := m.app.Detail(m.ctx, p.view.ID, row.ID)
	if err != nil {
		p.detail = nil
		return
	}
	p.detail = &detail
}

func (p *viewPage) openDetail(m *Model) {
	row, has := p.selected()
	if !has {
		return
	}
	detail, err := m.app.Detail(m.ctx, p.view.ID, row.ID)
	if m.fail(err) {
		return
	}
	p.detail, p.detailAt = &detail, 0
}

func (p *viewPage) filterLabel() string {
	for _, filter := range p.view.Filters {
		if filter.ID == p.query.Filter {
			return filter.Label
		}
	}
	return p.query.Filter
}

func (p *viewPage) filtered() bool {
	return p.query.Search != "" || p.query.Filter != p.view.Filters[0].ID
}

func (p *viewPage) cycle(m *Model, step int) {
	at := 0
	for index, filter := range p.view.Filters {
		if filter.ID == p.query.Filter {
			at = index
		}
	}
	p.query.Filter = p.view.Filters[(at+step+len(p.view.Filters))%len(p.view.Filters)].ID
	p.cursor = 0
	p.load(m)
}

func (p *viewPage) key(m *Model, key string, typed []rune) tea.Cmd {
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
		return nil
	}
	if key == "delete" {
		key = "d"
	}
	for _, action := range p.view.Actions {
		if action.Key == key {
			return m.begin(p, action)
		}
	}
	if p.detail != nil {
		switch key {
		case "up", "k":
			p.detailAt--
		case "down", "j":
			p.detailAt++
		case "pgup":
			p.detailAt -= 10
		case "pgdown", " ":
			p.detailAt += 10
		case "home", "g":
			p.detailAt = 0
		case "esc", "enter", "left":
			p.detail = nil
		}
		p.detailAt = max(0, p.detailAt)
		return nil
	}
	last := len(p.table.Rows) - 1
	switch key {
	case "up", "k":
		p.cursor = max(0, p.cursor-1)
	case "down", "j":
		p.cursor = max(0, min(last, p.cursor+1))
	case "pgup":
		p.cursor = max(0, p.cursor-10)
	case "pgdown":
		p.cursor = max(0, min(last, p.cursor+10))
	case "home", "g":
		p.cursor = 0
	case "end", "G":
		p.cursor = max(0, last)
	case "enter":
		p.openDetail(m)
	case "esc":
		if p.filtered() {
			p.query, p.search, p.cursor = app.ViewQuery{Filter: p.view.Filters[0].ID}, newInput("", 200), 0
			p.load(m)
		}
	case "/":
		p.searching, p.search = true, newInput(p.query.Search, 200)
	case "f":
		p.cycle(m, 1)
	case "F":
		p.cycle(m, -1)
	}
	return nil
}

func (p *viewPage) hints() string {
	if p.searching {
		return "Type to search · Enter keep · Esc clear"
	}
	parts := []string{"Enter details", "/ search", "f " + strings.ToLower(p.view.FilterLabel) + " next", "F previous"}
	if p.detail != nil {
		parts = []string{"Esc back to the table", "Up/Down scroll"}
	}
	for _, action := range p.view.Actions {
		parts = append(parts, action.Key+" "+strings.ToLower(action.Label))
	}
	return strings.Join(parts, " · ")
}

func fitRight(text string, width int) string {
	if lipgloss.Width(text) >= width {
		return fit(text, width)
	}
	return strings.Repeat(" ", width-lipgloss.Width(text)) + text
}

func tableWidths(table app.Table, width int) []int {
	columns := table.Columns
	natural := make([]int, len(columns))
	least := make([]int, len(columns))
	widths := make([]int, len(columns))
	used := 0
	for index, item := range columns {
		title := lipgloss.Width(item.Title)
		natural[index] = title
		for _, row := range table.Rows {
			if index < len(row.Cells) {
				natural[index] = max(natural[index], lipgloss.Width(row.Cells[index]))
			}
		}
		natural[index] = min(natural[index], max(item.Width*2, title))
		least[index] = min(natural[index], max(title, 12))
		switch {
		case item.ID == "timeline":
			natural[index] = max(item.Width, title)
			least[index] = natural[index]
		case item.Right:
			least[index] = natural[index]
		case index == 0:
			least[index] = min(natural[index], 22)
		}
		if index == 0 || item.ID == "timeline" {
			widths[index] = least[index]
			used += least[index] + 2
		}
	}
	for index := range columns {
		if widths[index] > 0 {
			continue
		}
		if used+least[index]+2 > width {
			break
		}
		widths[index] = least[index]
		used += least[index] + 2
	}
	for index := range columns {
		if widths[index] == 0 {
			continue
		}
		more := max(0, min(natural[index]-widths[index], width-used))
		widths[index] += more
		used += more
	}
	if used > width {
		widths[0] = max(4, widths[0]-(used-width))
	}
	return widths
}

func tableRow(columns []app.Column, widths []int, cells []string) string {
	var builder strings.Builder
	for index, cell := range cells {
		if index >= len(widths) || widths[index] == 0 {
			continue
		}
		if cell == "" {
			cell = "-"
		}
		if columns[index].Right {
			builder.WriteString(fitRight(cell, widths[index]))
		} else {
			builder.WriteString(fit(cell, widths[index]))
		}
		builder.WriteString("  ")
	}
	return strings.TrimRight(builder.String(), " ")
}

func tableHeader(columns []app.Column, widths []int) string {
	titles := make([]string, len(columns))
	for index, item := range columns {
		titles[index] = strings.ToUpper(item.Title)
	}
	return tableRow(columns, widths, titles)
}

func (m *Model) rowStyle(row app.Row) lipgloss.Style {
	switch {
	case row.Muted:
		return m.st.muted
	case row.Severity == theme.SeverityCritical || row.Severity == theme.SeverityHigh:
		return m.st.failure
	case row.Severity == theme.SeverityMedium:
		return m.st.warning
	}
	return lipgloss.NewStyle()
}

func (m *Model) staticTable(table app.Table, width int) []string {
	if len(table.Rows) == 0 {
		return wrap(table.Empty, width)
	}
	widths := tableWidths(table, width-2)
	lines := []string{m.st.muted.Render("  " + tableHeader(table.Columns, widths))}
	for _, row := range table.Rows {
		lines = append(lines, m.rowStyle(row).Render("  "+tableRow(table.Columns, widths, row.Cells)))
	}
	return lines
}

func (m *Model) detailLines(detail app.Detail, width int) []string {
	lines := []string{m.st.heading.Render(fit(detail.Title, width))}
	subtitle := detail.Subtitle
	if detail.Severity != "" {
		subtitle = detail.Severity.Glyph() + " " + subtitle
	}
	lines = append(lines, m.st.muted.Render(fit(subtitle, width)))
	for _, section := range detail.Sections {
		lines = append(lines, "", m.st.accent.Render(section.Title))
		labelWidth := 0
		for _, fact := range section.Facts {
			labelWidth = max(labelWidth, lipgloss.Width(fact[0]))
		}
		for _, fact := range section.Facts {
			value := fact[1]
			if value == "" {
				value = "-"
			}
			for index, part := range wrap(value, width-labelWidth-4) {
				label := ""
				if index == 0 {
					label = fact[0]
				}
				lines = append(lines, "  "+m.st.muted.Render(fit(label, labelWidth))+"  "+part)
			}
		}
		if section.Table != nil {
			lines = append(lines, m.staticTable(*section.Table, width)...)
		}
		if section.Text != "" {
			for _, part := range wrap(section.Text, width-2) {
				lines = append(lines, "  "+part)
			}
		}
	}
	return lines
}

func (p *viewPage) render(m *Model, width, height int) []string {
	if p.detail != nil {
		body := m.detailLines(*p.detail, width)
		shown, offset := window(body, p.detailAt, height-2)
		p.detailAt = offset
		position := fmt.Sprintf("Lines %d to %d of %d", offset+1, offset+len(shown), len(body))
		return append(append([]string{}, shown...), "", m.st.muted.Render(position))
	}
	search := p.query.Search
	if p.searching {
		search = strings.TrimRight(p.search.view(24, true), " ")
	} else if search == "" {
		search = "none"
	}
	var lines []string
	for _, line := range wrap(fmt.Sprintf("Search: %s · %s: %s", search, p.view.FilterLabel, p.filterLabel()), width) {
		lines = append(lines, m.st.muted.Render(line))
	}
	lines = append(lines, "")
	if len(p.table.Rows) == 0 {
		message := p.table.Empty
		if p.filtered() {
			message += " Press Esc to clear the search and the filter."
		}
		return append(lines, wrap(message, width)...)
	}
	widths := tableWidths(p.table, width-2)
	lines = append(lines, m.st.muted.Render("  "+tableHeader(p.table.Columns, widths)))
	rows := max(3, height-len(lines)-3)
	if p.cursor < p.offset {
		p.offset = p.cursor
	}
	if p.cursor >= p.offset+rows {
		p.offset = p.cursor - rows + 1
	}
	p.offset = max(0, min(p.offset, len(p.table.Rows)-rows))
	for index := p.offset; index < len(p.table.Rows) && index < p.offset+rows; index++ {
		row := p.table.Rows[index]
		text := tableRow(p.table.Columns, widths, row.Cells)
		if index == p.cursor {
			lines = append(lines, m.st.selected.Render(fit("▌ "+text, width)))
			continue
		}
		lines = append(lines, m.rowStyle(row).Render("  "+text))
	}
	footer := fmt.Sprintf("Row %d of %d", p.cursor+1, len(p.table.Rows))
	if p.table.Footer != "" {
		footer += " · " + p.table.Footer
	}
	hidden := 0
	for _, size := range widths {
		if size == 0 {
			hidden++
		}
	}
	if hidden == 1 {
		footer += " · 1 column is hidden: Enter shows it, b hides the sidebar"
	} else if hidden > 1 {
		footer += fmt.Sprintf(" · %d columns are hidden: Enter shows them, b hides the sidebar", hidden)
	}
	lines = append(lines, m.st.muted.Render(wrap(footer, width)[0]))
	if rest := wrap(footer, width); len(rest) > 1 {
		lines = append(lines, m.st.muted.Render(fit(strings.Join(rest[1:], " "), width)))
	}
	return lines
}

func fieldLimit(item app.Field) int {
	switch {
	case item.Kind == app.FieldPath:
		return 1024
	case item.Kind == app.FieldNumber:
		return 6
	case item.ID == "note":
		return 4000
	}
	return 253
}

func (m *Model) begin(p *viewPage, action app.Action) tea.Cmd {
	request := app.Request{View: p.view.ID, Action: action.ID, Values: map[string]string{}}
	if action.Row {
		row, has := p.selected()
		if !has {
			m.say(noticeInfo, "%s needs a selected row, and this table is empty", action.Label)
			return nil
		}
		request.ID = row.ID
	}
	if len(action.Fields) == 0 {
		if err := m.planned(request, action); m.fail(err) {
			return nil
		}
		return m.takeAfter()
	}
	defaults, err := m.app.Defaults(m.ctx, request)
	if m.fail(err) {
		return nil
	}
	var fields []field
	for _, item := range action.Fields {
		switch item.Kind {
		case app.FieldBool:
			fields = append(fields, choiceField(item.ID, item.Label, defaults[item.ID], item.Help, []string{"false", "true"}, []string{"No", "Yes"}))
		case app.FieldChoice:
			var ids, labels []string
			for _, option := range item.Options {
				ids, labels = append(ids, option.ID), append(labels, option.Label)
			}
			fields = append(fields, choiceField(item.ID, item.Label, defaults[item.ID], item.Help, ids, labels))
		default:
			fields = append(fields, textField(item.ID, item.Label, defaults[item.ID], item.Help, fieldLimit(item)))
		}
	}
	m.form = &form{title: p.view.Title + ", " + action.Label, fields: fields, submit: func(values map[string]string) error {
		request.Values = map[string]string{}
		for key, value := range defaults {
			request.Values[key] = value
		}
		for key, value := range values {
			request.Values[key] = value
		}
		return m.planned(request, action)
	}}
	return nil
}

func (m *Model) takeAfter() tea.Cmd {
	command := m.after
	m.after = nil
	return command
}

func (m *Model) planned(request app.Request, action app.Action) error {
	plan, err := m.app.Plan(m.ctx, request)
	if err != nil {
		return err
	}
	if !plan.Confirm && !action.Preview {
		if action.Network {
			m.after = m.execute(request, action)
			return nil
		}
		outcome, err := m.app.Run(m.ctx, request)
		if err != nil {
			return err
		}
		m.done(request.View, outcome, nil)
		return nil
	}
	lines := []string{plan.Statement}
	if len(plan.Network) > 0 {
		lines = append(lines, "", "Network connections this opens:")
		for _, line := range plan.Network {
			lines = append(lines, "  "+line)
		}
	}
	if len(plan.Lines) > 0 {
		lines = append(lines, "")
		lines = append(lines, plan.Lines...)
	}
	title := action.Label
	if view, found := app.LookupView(request.View); found {
		title = view.Title + ", " + action.Label
	}
	m.preview = &preview{title: title, lines: lines, accept: strings.ToLower(action.Label), command: func() tea.Cmd { return m.execute(request, action) }}
	return nil
}

func (m *Model) execute(request app.Request, action app.Action) tea.Cmd {
	if !action.Network {
		outcome, err := m.app.Run(m.ctx, request)
		m.done(request.View, outcome, err)
		return nil
	}
	m.busy = action.Label + ": waiting for the answers. Each endpoint can take up to the check timeout."
	return func() tea.Msg {
		outcome, err := m.app.Run(m.ctx, request)
		return actionDoneMsg{view: request.View, outcome: outcome, err: err}
	}
}

func (m *Model) done(view string, outcome app.Outcome, err error) {
	m.busy = ""
	if m.fail(err) {
		return
	}
	m.changed("%s", outcome.Message)
	if page := m.pages[view]; page != nil && outcome.Select != 0 {
		page.selectID(outcome.Select)
	}
	if len(outcome.Lines) > 0 {
		m.preview = &preview{title: "Result", lines: append([]string{outcome.Message, ""}, outcome.Lines...)}
	}
}

func (m *Model) openTile(tile app.Tile) {
	page := m.pages[tile.View]
	if page == nil {
		return
	}
	page.query, page.cursor, page.detail = app.ViewQuery{Filter: tile.Filter}, 0, nil
	page.load(m)
	m.open(tile.View)
}

func (m *Model) overviewKey(key string) {
	tiles := m.overview.Tiles
	switch key {
	case "l":
		m.loadDemo()
	case "up", "k":
		m.tile = max(0, m.tile-1)
	case "down", "j":
		m.tile = max(0, min(len(tiles)-1, m.tile+1))
	case "enter":
		if m.tile < len(tiles) {
			m.openTile(tiles[m.tile])
		}
	}
}

func (m *Model) overviewView(width int) []string {
	o := m.overview
	lines := []string{m.st.heading.Render("What needs attention"), ""}
	for index, tile := range o.Tiles {
		glyph := "  "
		if tile.Severity != "" {
			glyph = tile.Severity.Glyph()
		}
		text := fit(glyph+" "+tile.Label, 26) + fitRight(fmt.Sprintf("%d", tile.Value), 5)
		switch {
		case index == m.tile:
			lines = append(lines, m.st.selected.Render(fit("▌ "+text, 36))+m.st.muted.Render("  Enter opens the list"))
		case tile.Severity == theme.SeverityCritical || tile.Severity == theme.SeverityHigh:
			lines = append(lines, m.st.failure.Render("  "+text))
		case tile.Severity == theme.SeverityMedium:
			lines = append(lines, m.st.warning.Render("  "+text))
		default:
			lines = append(lines, "  "+text)
		}
	}
	lines = append(lines, "", m.st.muted.Render(fit(fmt.Sprintf("  %d tracked certificates · %d endpoints, %d disabled · %d unread alerts", o.Counts.Certificates, o.Counts.Endpoints, o.Counts.DisabledEndpoints, o.Counts.UnreadAlerts), width)))
	if o.Empty {
		lines = append(lines, "")
		return append(lines, wrap("Nothing is monitored yet. Press 3 and then n to add a host you are authorized to test, press 2 and then i to import a PEM file, or press l to load labeled demo data.", width)...)
	}
	if o.Demo {
		lines = append(lines, wrap("ℹ Demo data is loaded. Demo hosts end in .demo.invalid and are never contacted. Remove them in Settings, Remove demo data.", width)...)
	}
	lines = append(lines, "", m.st.heading.Render("Soonest to expire"))
	lines = append(lines, m.staticTable(o.Soonest, width)...)
	lines = append(lines, "", m.st.heading.Render("Recent alerts"))
	lines = append(lines, m.staticTable(o.Alerts, width)...)
	return lines
}
