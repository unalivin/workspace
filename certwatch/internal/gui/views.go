package gui

import (
	"errors"
	"fmt"
	"strings"

	"fyne.io/fyne/v2"
	"fyne.io/fyne/v2/canvas"
	"fyne.io/fyne/v2/container"
	"fyne.io/fyne/v2/layout"
	fynetheme "fyne.io/fyne/v2/theme"
	"fyne.io/fyne/v2/widget"

	"github.com/NexzaDev/certwatch/internal/app"
	"github.com/NexzaDev/certwatch/internal/inventory"
	"github.com/NexzaDev/certwatch/internal/theme"
	"github.com/NexzaDev/certwatch/internal/words"
)

const (
	listLimit   = 1000
	timelineID  = "timeline"
	maximumTick = 4
)

var actionIcons = map[string]string{
	"add": "plus", "generate": "plus", "check": "refresh", "check-all": "refresh", "probe": "refresh", "edit": "pencil",
	"enable": "check", "disable": "circle-minus", "remove": "trash", "stop-tracking": "trash", "import": "upload", "export": "download",
}

type barLayout struct {
	fraction float32
	ticks    []float32
}

func (b *barLayout) Layout(objects []fyne.CanvasObject, size fyne.Size) {
	height := min(size.Height, float32(10))
	top := (size.Height - height) / 2
	for index, object := range objects {
		switch {
		case index == 0:
			object.Move(fyne.NewPos(0, top))
			object.Resize(fyne.NewSize(size.Width, height))
		case index == 1:
			object.Move(fyne.NewPos(0, top))
			object.Resize(fyne.NewSize(size.Width*b.fraction, height))
		case index-2 < len(b.ticks):
			object.Show()
			object.Move(fyne.NewPos(size.Width*b.ticks[index-2], top-3))
			object.Resize(fyne.NewSize(1, height+6))
		default:
			object.Hide()
		}
	}
}

func (b *barLayout) MinSize([]fyne.CanvasObject) fyne.Size { return fyne.NewSize(120, 16) }

func barFraction(text string) float32 {
	filled, total := 0, 0
	for _, symbol := range text {
		switch symbol {
		case '█':
			filled++
			total++
		case '░':
			total++
		}
	}
	if total == 0 {
		return 0
	}
	return float32(filled) / float32(total)
}

func (d *dashboard) newBar() *fyne.Container {
	objects := []fyne.CanvasObject{canvas.NewRectangle(rgb(d.tokens.Palette.Border)), canvas.NewRectangle(rgb(d.tokens.Palette.Accent))}
	for range maximumTick {
		objects = append(objects, canvas.NewRectangle(rgb(d.tokens.Palette.TextMuted)))
	}
	return container.New(&barLayout{}, objects...)
}

func (d *dashboard) fillBar(bar *fyne.Container, table app.Table, row app.Row, text string) {
	shape := bar.Layout.(*barLayout)
	shape.fraction = barFraction(text)
	shape.ticks = shape.ticks[:0]
	for _, mark := range table.Marks {
		if table.Window > 0 && mark.Days > 0 && mark.Days < table.Window && len(shape.ticks) < maximumTick {
			shape.ticks = append(shape.ticks, float32(mark.Days)/float32(table.Window))
		}
	}
	tint := d.tokens.Palette.Accent
	if row.Severity != "" {
		tint = d.tokens.Semantic.Severity(row.Severity)
	}
	bar.Objects[0].(*canvas.Rectangle).FillColor = rgb(d.tokens.Palette.Border)
	bar.Objects[1].(*canvas.Rectangle).FillColor = rgb(tint)
	for _, tick := range bar.Objects[2:] {
		tick.(*canvas.Rectangle).FillColor = rgb(d.tokens.Palette.TextMuted)
	}
	bar.Refresh()
}

func (d *dashboard) severityIcon(severity theme.Severity) fyne.Resource {
	if severity == "" {
		return icon("status-open", d.tokens.Palette.Border)
	}
	return icon(severity.Icon(), d.tokens.Semantic.Severity(severity))
}

func tableLayout(table app.Table) *columns {
	shape := &columns{}
	for index, column := range table.Columns {
		fixed := float32(min(column.Width, 15)) * 8
		if index == 0 {
			fixed = 170
		}
		if column.ID == timelineID {
			fixed = 160
		}
		shape.fixed = append(shape.fixed, fixed)
		shape.weights = append(shape.weights, float32(column.Width))
	}
	return shape
}

func (d *dashboard) tableHeader(table app.Table, shape *columns) *fyne.Container {
	row := container.New(shape)
	for index, column := range table.Columns {
		label := cell(column.Title)
		label.TextStyle = fyne.TextStyle{Bold: true}
		label.Importance = widget.LowImportance
		if column.Right {
			label.Alignment = fyne.TextAlignTrailing
		}
		if index == 0 {
			row.Add(container.NewBorder(nil, nil, widget.NewIcon(icon("status-open", d.tokens.Palette.Background)), nil, label))
			continue
		}
		row.Add(label)
	}
	return row
}

func (d *dashboard) tableCells(table app.Table, shape *columns) *fyne.Container {
	row := container.New(shape)
	for index, column := range table.Columns {
		switch {
		case index == 0:
			first, _, _ := iconLabel(d.severityIcon(""), "")
			row.Add(first)
		case column.ID == timelineID:
			row.Add(d.newBar())
		default:
			label := cell("")
			if column.Right {
				label.Alignment = fyne.TextAlignTrailing
			}
			row.Add(label)
		}
	}
	return row
}

func (d *dashboard) fillCells(object fyne.CanvasObject, table app.Table, row app.Row) {
	cells := object.(*fyne.Container).Objects
	for index := range table.Columns {
		if index >= len(cells) || index >= len(row.Cells) {
			break
		}
		_, text := app.SplitMark(row.Cells[index])
		importance := widget.MediumImportance
		if row.Muted {
			importance = widget.LowImportance
		}
		switch {
		case index == 0:
			first := cells[0].(*fyne.Container)
			label := first.Objects[0].(*widget.Label)
			label.Importance = importance
			label.SetText(text)
			first.Objects[1].(*widget.Icon).SetResource(d.severityIcon(row.Severity))
		case table.Columns[index].ID == timelineID:
			d.fillBar(cells[index].(*fyne.Container), table, row, row.Cells[index])
		default:
			label := cells[index].(*widget.Label)
			label.Importance = importance
			label.SetText(text)
		}
	}
}

func (d *dashboard) staticTable(table app.Table) fyne.CanvasObject {
	if len(table.Rows) == 0 {
		return muted(table.Empty)
	}
	shape := tableLayout(table)
	box := container.NewVBox(d.tableHeader(table, shape))
	for _, row := range table.Rows {
		cells := d.tableCells(table, shape)
		d.fillCells(cells, table, row)
		box.Add(cells)
	}
	return box
}

type viewPage struct {
	d        *dashboard
	view     app.View
	root     fyne.CanvasObject
	query    app.ViewQuery
	data     app.Table
	selected int64
	search   *widget.Entry
	filter   *widget.Select
	holder   *fyne.Container
	list     *widget.List
	shape    *columns
	columns  string
	empty    *widget.Label
	count    *widget.Label
	detail   *fyne.Container
	name     *widget.Label
	state    *widget.Icon
	subtitle *widget.Label
	sections *fyne.Container
	body     *fyne.Container
	upper    fyne.CanvasObject
	side     fyne.CanvasObject
	shown    app.Detail
	loading  bool

	rowButtons map[string]*widget.Button
}

func (d *dashboard) actionButton(view app.View, action app.Action, id func() int64) *widget.Button {
	button := widget.NewButtonWithIcon(action.Label, glyph(actionIcons[action.ID]), func() { d.begin(view, action, id()) })
	if action.Danger {
		button.Importance = widget.DangerImportance
	}
	return button
}

func newViewPage(d *dashboard, view app.View) *viewPage {
	p := &viewPage{d: d, view: view, rowButtons: map[string]*widget.Button{}}
	reload := func() {
		if !p.loading {
			p.refresh()
		}
	}
	p.search = widget.NewEntry()
	p.search.SetPlaceHolder("Search " + strings.ToLower(view.Title))
	p.search.OnChanged = func(text string) {
		p.query.Search = text
		reload()
	}
	var labels []string
	for _, choice := range view.Filters {
		labels = append(labels, choice.Label)
	}
	p.filter = widget.NewSelect(labels, func(label string) {
		for _, choice := range view.Filters {
			if choice.Label == label {
				p.query.Filter = choice.ID
			}
		}
		reload()
	})
	p.loading = true
	if len(labels) > 0 {
		p.filter.SetSelectedIndex(0)
	}
	p.loading = false
	general := container.NewHBox()
	chosen := container.NewHBox()
	first := true
	for _, action := range view.Actions {
		if action.Row {
			button := d.actionButton(view, action, func() int64 { return p.selected })
			p.rowButtons[action.ID] = button
			chosen.Add(button)
			continue
		}
		button := d.actionButton(view, action, func() int64 { return 0 })
		if first {
			button.Importance = widget.HighImportance
			first = false
		}
		general.Add(button)
	}
	filterLabel := widget.NewLabel(view.FilterLabel)
	filterLabel.Importance = widget.LowImportance
	toolbar := container.NewVBox(container.NewBorder(nil, nil, container.NewHBox(filterLabel, p.filter), general, p.search), muted(view.Help))
	p.empty = wrapped("")
	p.count = muted("")
	p.holder = container.NewStack()
	p.name = heading("")
	p.name.Wrapping = fyne.TextWrapWord
	p.state = widget.NewIcon(d.severityIcon(""))
	p.subtitle = wrapped("")
	p.sections = container.NewVBox()
	chosen.Add(widget.NewButtonWithIcon("Copy", glyph("copy"), p.copy))
	closeButton := widget.NewButtonWithIcon("", glyph("x"), p.unselect)
	closeButton.Importance = widget.LowImportance
	top := container.NewBorder(nil, nil, nil, closeButton, container.NewVBox(p.name, container.NewBorder(nil, nil, p.state, nil, p.subtitle)))
	p.detail = container.NewBorder(top, container.NewHScroll(chosen), nil, nil, container.NewVScroll(p.sections))
	p.detail.Hide()
	p.upper = p.holder
	p.side = panel(p.detail)
	p.body = container.NewStack(p.upper)
	p.root = container.NewBorder(toolbar, nil, nil, nil, p.body)
	return p
}

func (p *viewPage) object() fyne.CanvasObject { return p.root }

func (p *viewPage) rebuild() {
	p.shape = tableLayout(p.data)
	p.list = widget.NewList(
		func() int { return len(p.data.Rows) },
		func() fyne.CanvasObject { return p.d.tableCells(p.data, p.shape) },
		func(id widget.ListItemID, object fyne.CanvasObject) {
			if id < len(p.data.Rows) {
				p.d.fillCells(object, p.data, p.data.Rows[id])
			}
		})
	p.list.OnSelected = func(id widget.ListItemID) {
		if id < len(p.data.Rows) && p.data.Rows[id].ID != p.selected {
			p.selected = p.data.Rows[id].ID
			p.showDetail()
		}
	}
	p.holder.Objects = []fyne.CanvasObject{container.NewBorder(p.d.tableHeader(p.data, p.shape), p.count, nil, nil, container.NewStack(p.list, container.NewVBox(p.empty)))}
	p.holder.Refresh()
}

func (p *viewPage) filtered() bool {
	return p.query.Search != "" || (len(p.view.Filters) > 0 && p.query.Filter != "" && p.query.Filter != p.view.Filters[0].ID)
}

func (p *viewPage) refresh() {
	table, err := p.d.app.Table(p.d.ctx, p.view.ID, p.query)
	if p.d.fail(err) {
		return
	}
	var ids []string
	for _, column := range table.Columns {
		ids = append(ids, column.ID)
	}
	p.data = table
	if shape := strings.Join(ids, ","); shape != p.columns || p.list == nil {
		p.columns = shape
		p.rebuild()
	}
	p.list.Refresh()
	footer, _, _ := strings.Cut(table.Footer, "; one block")
	p.count.SetText(footer)
	if len(table.Rows) > 0 {
		p.empty.Hide()
	} else {
		text := table.Empty
		if p.filtered() {
			text += " Clear the search box and choose \"" + p.view.Filters[0].Label + "\" to see everything."
		}
		p.empty.SetText(text)
		p.empty.Show()
	}
	for index, row := range table.Rows {
		if row.ID == p.selected {
			p.list.Select(index)
			p.showDetail()
			return
		}
	}
	p.unselect()
}

func (p *viewPage) reset(filter string) {
	p.loading = true
	p.search.SetText("")
	p.query = app.ViewQuery{}
	for index, choice := range p.view.Filters {
		if choice.ID == filter {
			p.filter.SetSelectedIndex(index)
			p.query.Filter = filter
		}
	}
	if p.query.Filter == "" && len(p.view.Filters) > 0 {
		p.filter.SetSelectedIndex(0)
		p.query.Filter = p.view.Filters[0].ID
	}
	p.loading = false
}

func (p *viewPage) selectID(id int64) {
	p.selected = id
	for index, row := range p.data.Rows {
		if row.ID == id {
			p.list.Select(index)
			p.list.ScrollTo(index)
		}
	}
	p.showDetail()
}

func (p *viewPage) unselect() {
	p.selected = 0
	if p.list != nil {
		p.list.UnselectAll()
	}
	if p.detail.Visible() {
		p.detail.Hide()
		p.body.Objects = []fyne.CanvasObject{p.upper}
		p.body.Refresh()
	}
}

func (p *viewPage) showDetail() {
	if p.selected == 0 {
		return
	}
	detail, err := p.d.app.Detail(p.d.ctx, p.view.ID, p.selected)
	if errors.Is(err, inventory.ErrNotFound) {
		p.unselect()
		return
	}
	if p.d.fail(err) {
		return
	}
	p.shown = detail
	p.name.SetText(detail.Title)
	_, subtitle := app.SplitMark(detail.Subtitle)
	p.subtitle.SetText(subtitle)
	p.state.SetResource(p.d.severityIcon(detail.Severity))
	for id, button := range p.rowButtons {
		if detail.Usable(id) {
			button.Show()
		} else {
			button.Hide()
		}
	}
	p.sections.RemoveAll()
	for _, section := range detail.Sections {
		p.sections.Add(heading(section.Title))
		if len(section.Facts) > 0 {
			facts := container.New(layout.NewFormLayout())
			for _, fact := range section.Facts {
				key := widget.NewLabel(fact[0])
				key.Importance = widget.LowImportance
				facts.Add(key)
				facts.Add(wrapped(fact[1]))
			}
			p.sections.Add(facts)
		}
		if section.Table != nil {
			p.sections.Add(p.d.staticTable(*section.Table))
		}
		if section.Text != "" {
			p.sections.Add(wrapped(section.Text))
		}
	}
	p.sections.Refresh()
	if !p.detail.Visible() {
		p.detail.Show()
		split := container.NewVSplit(p.upper, p.side)
		split.SetOffset(0.34)
		p.body.Objects = []fyne.CanvasObject{split}
		p.body.Refresh()
	}
}

func detailText(detail app.Detail) string {
	var text strings.Builder
	text.WriteString(detail.Title + "\n")
	if detail.Subtitle != "" {
		text.WriteString(detail.Subtitle + "\n")
	}
	for _, section := range detail.Sections {
		text.WriteString("\n" + section.Title + "\n")
		for _, fact := range section.Facts {
			text.WriteString(fact[0] + ": " + fact[1] + "\n")
		}
		if section.Table != nil {
			var titles []string
			for _, column := range section.Table.Columns {
				titles = append(titles, column.Title)
			}
			text.WriteString(strings.Join(titles, "\t") + "\n")
			for _, row := range section.Table.Rows {
				text.WriteString(strings.Join(row.Cells, "\t") + "\n")
			}
			if len(section.Table.Rows) == 0 {
				text.WriteString(section.Table.Empty + "\n")
			}
		}
		if section.Text != "" {
			text.WriteString(section.Text + "\n")
		}
	}
	return text.String()
}

func (p *viewPage) copy() {
	if p.selected == 0 {
		return
	}
	p.d.fyneApp.Clipboard().SetContent(detailText(p.shown))
	p.d.say(theme.SeverityLow, "Copied %s to the clipboard", p.shown.Title)
}

func (p *viewPage) remove() {
	if p.selected == 0 {
		return
	}
	for _, action := range p.view.Actions {
		if action.Row && action.Danger {
			p.d.begin(p.view, action, p.selected)
			return
		}
	}
}

func (d *dashboard) begin(view app.View, action app.Action, id int64) {
	if d.busy != "" {
		d.say(theme.SeverityInfo, "%s is still running. Wait for it to finish.", d.busy)
		return
	}
	request := app.Request{View: view.ID, Action: action.ID, ID: id, Values: map[string]string{}}
	title := view.Title + ", " + action.Label
	if len(action.Fields) == 0 {
		d.fail(d.planned(title, request, action))
		return
	}
	defaults, err := d.app.Defaults(d.ctx, request)
	if d.fail(err) {
		return
	}
	var fields []formField
	for _, item := range action.Fields {
		field := formField{key: item.ID, label: item.Label, hint: item.Help, value: defaults[item.ID]}
		switch item.Kind {
		case app.FieldBool:
			field.options, field.labels = []string{"false", "true"}, []string{"No", "Yes"}
		case app.FieldChoice:
			for _, option := range item.Options {
				field.options, field.labels = append(field.options, option.ID), append(field.labels, option.Label)
			}
		case app.FieldPath:
			field.browse = "save"
			if action.ID == "import" {
				field.browse = "open"
			}
		}
		fields = append(fields, field)
	}
	d.showForm(title, action.Label, fields, func(values map[string]string) error {
		request.Values = map[string]string{}
		for key, value := range defaults {
			request.Values[key] = value
		}
		for key, value := range values {
			request.Values[key] = value
		}
		return d.planned(title, request, action)
	})
}

func (d *dashboard) planned(title string, request app.Request, action app.Action) error {
	plan, err := d.app.Plan(d.ctx, request)
	if err != nil {
		return err
	}
	if !plan.Confirm && !action.Preview {
		if action.Network {
			d.execute(request, action)
			return nil
		}
		outcome, err := d.app.Run(d.ctx, request)
		if err != nil {
			return err
		}
		d.finish(request.View, outcome, nil)
		return nil
	}
	var text strings.Builder
	text.WriteString(plan.Statement)
	if len(plan.Network) > 0 {
		text.WriteString("\n\nNetwork connections this opens:")
		for _, line := range plan.Network {
			text.WriteString("\n  " + line)
		}
	}
	if len(plan.Lines) > 0 {
		text.WriteString("\n\n" + strings.Join(plan.Lines, "\n"))
	}
	d.confirm = newConfirmDialog(d.window, title, text.String(), action.Label, plan.Danger, func() { d.execute(request, action) }, func() { d.confirm = nil })
	return nil
}

func (d *dashboard) execute(request app.Request, action app.Action) {
	if !action.Network {
		outcome, err := d.app.Run(d.ctx, request)
		d.finish(request.View, outcome, err)
		return
	}
	d.setBusy(action.Label)
	d.spawn(func() {
		outcome, err := d.app.Run(d.ctx, request)
		d.onMain(func() { d.finish(request.View, outcome, err) })
	})
}

func (d *dashboard) finish(view string, outcome app.Outcome, err error) {
	d.setBusy("")
	if d.fail(err) {
		return
	}
	if page := d.views[view]; page != nil && outcome.Select != 0 {
		page.selected = outcome.Select
	}
	d.done("%s", outcome.Message)
	if len(outcome.Lines) > 0 {
		d.confirm = newConfirmDialog(d.window, "Result", outcome.Message+"\n\n"+strings.Join(outcome.Lines, "\n"), "Close", false, func() {}, func() { d.confirm = nil })
	}
}

func (d *dashboard) setBusy(what string) {
	d.busy = what
	if d.busyBox == nil {
		return
	}
	if what == "" {
		d.busyBar.Stop()
		d.busyBox.Hide()
	} else {
		d.busyText.SetText(what + ": waiting for the answers. Each endpoint can take up to the check timeout.")
		d.busyBox.Show()
		if d.tokens.Motion {
			d.busyBar.Show()
			d.busyBar.Start()
		} else {
			d.busyBar.Hide()
		}
	}
	d.body.Refresh()
}

func (d *dashboard) openRow(view string, id int64) {
	page := d.views[view]
	if page == nil {
		return
	}
	page.reset("all")
	page.selected = id
	d.refresh()
	d.show(view)
	page.selectID(id)
}

func (d *dashboard) openList(view, filter string) {
	page := d.views[view]
	if page == nil {
		return
	}
	page.reset(filter)
	page.unselect()
	page.refresh()
	d.show(view)
}

type tileCard struct {
	number *widget.Label
	state  *widget.Icon
	open   *widget.Button
}

type overviewPage struct {
	d       *dashboard
	root    fyne.CanvasObject
	tiles   map[string]*tileCard
	cards   *fyne.Container
	summary *widget.Label
	content *fyne.Container
	soonest *fyne.Container
	alerts  *fyne.Container
	tables  *fyne.Container
	empty   *fyne.Container
	demo    *widget.Label
}

func newOverviewPage(d *dashboard) *overviewPage {
	p := &overviewPage{d: d, tiles: map[string]*tileCard{}}
	p.cards = container.NewGridWithColumns(4)
	p.summary = muted("")
	p.soonest = container.NewVBox()
	p.alerts = container.NewVBox()
	p.tables = container.NewVBox(heading("Soonest to expire"), p.soonest, heading("Recent alerts"), p.alerts)
	add := widget.NewButtonWithIcon("Add endpoint", glyph("plus"), func() {
		d.show(app.ViewEndpoints)
		d.beginNamed(app.ViewEndpoints, "add")
	})
	add.Importance = widget.HighImportance
	p.empty = container.NewVBox(
		wrapped("Nothing is monitored yet. Add a host you are authorized to test, import a PEM file under Certificates, or load labeled demo data to look around. Demo hosts are never contacted."),
		container.NewHBox(add, widget.NewButtonWithIcon("Load demo data", glyph("download"), d.loadDemo)))
	p.demo = muted("Demo data is loaded. Demo hosts end in .demo.invalid and are never contacted. Remove them in Settings, Data, Remove demo data.")
	p.content = container.NewVBox(heading("What needs attention"), p.cards, p.summary, p.empty, p.demo, p.tables)
	p.root = container.NewVScroll(p.content)
	return p
}

func (p *overviewPage) object() fyne.CanvasObject { return p.root }

func (p *overviewPage) refresh() {
	o := p.d.overview
	for _, tile := range o.Tiles {
		card, found := p.tiles[tile.ID]
		if !found {
			number := widget.NewLabel("0")
			number.TextStyle = fyne.TextStyle{Bold: true}
			number.SizeName = fynetheme.SizeNameHeadingText
			card = &tileCard{number: number, state: widget.NewIcon(p.d.severityIcon(""))}
			view, filter := tile.View, tile.Filter
			card.open = widget.NewButtonWithIcon("Show", glyph("arrow-right"), func() { p.d.openList(view, filter) })
			card.open.Importance = widget.LowImportance
			p.tiles[tile.ID] = card
			p.cards.Add(panel(container.NewVBox(container.NewBorder(nil, nil, card.state, nil, widget.NewLabel(tile.Label)), container.NewBorder(nil, nil, card.number, card.open))))
		}
		card.number.SetText(fmt.Sprintf("%d", tile.Value))
		card.state.SetResource(p.d.severityIcon(tile.Severity))
	}
	p.summary.SetText(fmt.Sprintf("%s · %s, %d disabled · %s", words.Count(o.Counts.Certificates, "tracked certificate", "tracked certificates"), words.Count(o.Counts.Endpoints, "endpoint", "endpoints"), o.Counts.DisabledEndpoints, words.Count(o.Counts.UnreadAlerts, "unread alert", "unread alerts")))
	p.soonest.Objects = []fyne.CanvasObject{p.d.staticTable(o.Soonest)}
	p.alerts.Objects = []fyne.CanvasObject{p.d.staticTable(o.Alerts)}
	if o.Empty {
		p.empty.Show()
		p.tables.Hide()
	} else {
		p.empty.Hide()
		p.tables.Show()
	}
	if o.Demo {
		p.demo.Show()
	} else {
		p.demo.Hide()
	}
	p.content.Refresh()
}

func (d *dashboard) beginNamed(view, action string) {
	found, known := app.LookupView(view)
	if !known {
		return
	}
	for _, item := range found.Actions {
		if item.ID == action {
			d.begin(found, item, 0)
		}
	}
}
