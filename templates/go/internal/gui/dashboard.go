package gui

import (
	"context"
	"fmt"
	"image/color"
	"strconv"
	"sync"
	"time"

	"fyne.io/fyne/v2"
	"fyne.io/fyne/v2/canvas"
	"fyne.io/fyne/v2/container"
	"fyne.io/fyne/v2/driver/desktop"
	fynetheme "fyne.io/fyne/v2/theme"
	"fyne.io/fyne/v2/widget"

	"github.com/unalivin/templateapp/internal/app"
	"github.com/unalivin/templateapp/internal/launcher"
	"github.com/unalivin/templateapp/internal/meta"
	"github.com/unalivin/templateapp/internal/notify"
	"github.com/unalivin/templateapp/internal/records"
	"github.com/unalivin/templateapp/internal/settings"
	"github.com/unalivin/templateapp/internal/store"
	"github.com/unalivin/templateapp/internal/theme"
)

const pollInterval = time.Second

type Options struct {
	App          *app.App
	Getenv       func(string) string
	Autostart    func() notify.Autostart
	OpenURL      func(url string) error
	OpenTerminal func() error
	InTerminal   bool
}

type page interface {
	object() fyne.CanvasObject
	refresh()
}

type dashboard struct {
	ctx        context.Context
	options    Options
	app        *app.App
	fyneApp    fyne.App
	window     fyne.Window
	tokens     theme.Tokens
	values     map[string]settings.Value
	overview   records.Overview
	nav        []app.NavItem
	current    string
	pages      map[string]page
	buttons    map[string]*widget.Button
	hinted     map[string]*hintButton
	stack      *fyne.Container
	sidebar    *fyne.Container
	title      *widget.Label
	notice     *widget.Label
	noticeIc   *widget.Icon
	noticeBg   *fyne.Container
	body       *fyne.Container
	root       *fyne.Container
	header     *fyne.Container
	search     *widget.Entry
	chip       *widget.Button
	bell       *hintButton
	help       *hintButton
	hintBox    *fyne.Container
	hintFill   *canvas.Rectangle
	hintText   *canvas.Text
	hintLabel  string
	noticeFill *canvas.Rectangle
	fades      int
	early      func()
	collapse   bool
	form       *formDialog
	confirm    *confirmDialog
	switchTo   string
	stop       chan struct{}
	stopOnce   sync.Once
	finished   func(switchTo string)
	recordsP   *recordsPage
	inboxP     *inboxPage
	settingP   *settingsPage
	aboutP     *aboutPage
	overP      *overviewPage
}

func newDashboard(ctx context.Context, fyneApp fyne.App, window fyne.Window, options Options, finished func(string)) *dashboard {
	if options.Getenv == nil {
		options.Getenv = func(string) string { return "" }
	}
	d := &dashboard{ctx: ctx, options: options, app: options.App, fyneApp: fyneApp, window: window, nav: app.Navigation(), pages: map[string]page{}, buttons: map[string]*widget.Button{}, hinted: map[string]*hintButton{}, stop: make(chan struct{}), finished: finished}
	d.restyle()
	d.collapse = d.values[settings.KeySidebarCollapsed].Bool()
	d.build()
	d.refresh()
	d.show(app.NavOverview)
	if d.early != nil {
		d.early()
		d.early = nil
	}
	if d.app.CreatedDatabase() {
		d.welcome()
	}
	return d
}

func (d *dashboard) restyle() {
	values, err := d.app.Settings.Effective(d.ctx)
	if d.fail(err) {
		return
	}
	d.values = values
	tokens, err := d.app.Appearance(d.ctx, d.fyneApp.Settings().ThemeVariant() == fynetheme.VariantDark)
	if d.fail(err) {
		return
	}
	changed := d.tokens.Name != tokens.Name || d.tokens.Dark != tokens.Dark || d.tokens.Palette != tokens.Palette || d.tokens.Type != tokens.Type || d.tokens.Spacing != tokens.Spacing
	d.tokens = tokens
	if changed {
		d.fyneApp.Settings().SetTheme(skin{tokens: tokens})
		d.tintNavigation()
	}
}

func (d *dashboard) say(kind theme.Severity, format string, arguments ...any) {
	if d.notice == nil || d.body == nil {
		d.early = func() { d.say(kind, format, arguments...) }
		return
	}
	name, tint := "info", d.tokens.Semantic.Info
	switch kind {
	case theme.SeverityHigh:
		name, tint = "triangle-alert", d.tokens.Semantic.Warning
	case theme.SeverityCritical:
		name, tint = "octagon-alert", d.tokens.Semantic.Error
	case theme.SeverityLow:
		name, tint = "status-done", d.tokens.Semantic.Success
	}
	d.noticeIc.SetResource(icon(name, tint))
	d.notice.SetText(fmt.Sprintf(format, arguments...))
	d.noticeBg.Show()
	d.flashNotice()
	d.body.Refresh()
}

func (d *dashboard) fail(err error) bool {
	if err == nil {
		return false
	}
	if store.IsBusy(err) {
		d.say(theme.SeverityHigh, "The database is busy in another window. Nothing was lost; the view refreshes when it is free.")
		return true
	}
	d.say(theme.SeverityCritical, "%s", err.Error())
	return true
}

func (d *dashboard) done(format string, arguments ...any) {
	d.refresh()
	d.say(theme.SeverityLow, format, arguments...)
}

func (d *dashboard) refresh() {
	d.restyle()
	overview, err := d.app.Records.Overview(d.ctx)
	if d.fail(err) {
		return
	}
	d.overview = overview
	for _, item := range d.pages {
		item.refresh()
	}
	d.labelNavigation()
	d.fail(d.app.Acknowledge(d.ctx))
}

func (d *dashboard) poll() {
	change, err := d.app.Poll(d.ctx)
	if err != nil {
		if !store.IsBusy(err) {
			d.fail(err)
		}
		return
	}
	if change.Changed {
		d.refresh()
		d.say(theme.SeverityInfo, "Updated by another window")
	}
}

func (d *dashboard) watch() {
	go func() {
		ticker := time.NewTicker(pollInterval)
		defer ticker.Stop()
		for {
			select {
			case <-d.stop:
				return
			case <-d.ctx.Done():
				fyne.Do(func() { d.close("") })
				return
			case <-ticker.C:
				fyne.Do(d.poll)
			}
		}
	}()
}

func (d *dashboard) build() {
	d.title = widget.NewLabel("")
	d.title.TextStyle = fyne.TextStyle{Bold: true}
	d.title.SizeName = fynetheme.SizeNameHeadingText
	d.notice = widget.NewLabel("")
	d.notice.Wrapping = fyne.TextWrapWord
	d.noticeIc = widget.NewIcon(icon("info", d.tokens.Semantic.Info))
	dismiss := widget.NewButtonWithIcon("", glyph("x"), func() {
		d.noticeBg.Hide()
		d.body.Refresh()
	})
	dismiss.Importance = widget.LowImportance
	d.noticeFill = canvas.NewRectangle(color.NRGBA{})
	d.noticeBg = container.NewStack(d.noticeFill, container.NewBorder(widget.NewSeparator(), nil, d.noticeIc, dismiss, d.notice))
	d.noticeBg.Hide()
	d.overP = newOverviewPage(d)
	d.recordsP = newRecordsPage(d)
	d.inboxP = newInboxPage(d)
	d.settingP = newSettingsPage(d)
	d.aboutP = newAboutPage(d)
	d.pages[app.NavOverview] = d.overP
	d.pages[app.NavRecords] = d.recordsP
	d.pages[app.NavNotifications] = d.inboxP
	d.pages[app.NavSettings] = d.settingP
	d.pages[app.NavAbout] = d.aboutP
	d.stack = container.NewStack()
	for _, item := range d.nav {
		object := d.pages[item.ID].object()
		object.Hide()
		d.stack.Add(object)
	}
	top := container.NewVBox()
	bottom := container.NewVBox()
	for _, item := range d.nav {
		id := item.ID
		label := item.Label
		button := newHintButton(d, label, icon(item.Icon, d.tokens.Palette.Text), func() string { return d.navigationHint(id, label) }, func() { d.show(id) })
		button.Alignment = widget.ButtonAlignLeading
		button.Importance = widget.LowImportance
		d.buttons[id] = &button.Button
		d.hinted[id] = button
		if item.Bottom {
			bottom.Add(button)
		} else {
			top.Add(button)
		}
	}
	toggle := newHintButton(d, "", icon("sidebar", d.tokens.Palette.TextMuted), func() string {
		if d.collapse {
			return "Expand the sidebar"
		}
		return "Collapse the sidebar to icons"
	}, d.toggleSidebar)
	toggle.Importance = widget.LowImportance
	toggle.Alignment = widget.ButtonAlignLeading
	d.buttons["sidebar"] = &toggle.Button
	d.hinted["sidebar"] = toggle
	bottom.Add(toggle)
	d.sidebar = container.NewBorder(top, bottom, nil, nil)
	d.header = container.NewBorder(nil, widget.NewSeparator(), nil, nil, container.NewPadded(d.buildTopBar()))
	d.body = container.NewBorder(d.header, d.noticeBg, nil, nil, container.NewPadded(d.stack))
	d.root = container.NewBorder(nil, nil, container.NewBorder(nil, nil, nil, widget.NewSeparator(), container.NewPadded(d.sidebar)), nil, d.body)
	d.window.SetContent(container.NewStack(d.root, d.buildHints()))
	d.window.SetMainMenu(d.menu())
	d.shortcuts()
	d.labelNavigation()
}

func (d *dashboard) tintNavigation() {
	for _, item := range d.nav {
		if button, found := d.buttons[item.ID]; found {
			tint := d.tokens.Palette.Text
			if item.ID == d.current {
				tint = d.tokens.OnAccent
			}
			button.SetIcon(icon(item.Icon, tint))
		}
	}
	if toggle, found := d.buttons["sidebar"]; found {
		toggle.SetIcon(icon("sidebar", d.tokens.Palette.TextMuted))
	}
}

func (d *dashboard) labelNavigation() {
	changed := false
	for _, item := range d.nav {
		button := d.buttons[item.ID]
		label := item.Label
		if item.ID == app.NavNotifications && d.overview.UnreadNotifications > 0 {
			label += " (" + strconv.Itoa(d.overview.UnreadNotifications) + ")"
		}
		if d.collapse {
			label = ""
		}
		if button.Text != label {
			button.SetText(label)
			changed = true
		}
	}
	if changed && d.root != nil {
		d.root.Refresh()
	}
	d.refreshTopBar()
}

func (d *dashboard) sidebarButton(id string) *hintButton { return d.hinted[id] }

func (d *dashboard) navigationHint(id, label string) string {
	if !d.collapse {
		return ""
	}
	if id == app.NavNotifications {
		return d.bellHint()
	}
	return label
}

func (d *dashboard) toggleSidebar() {
	d.hideHint()
	d.collapse = !d.collapse
	if _, err := d.app.Settings.Set(d.ctx, settings.KeySidebarCollapsed, strconv.FormatBool(d.collapse)); d.fail(err) {
		return
	}
	d.fail(d.app.Acknowledge(d.ctx))
	d.labelNavigation()
	d.settingP.refresh()
}

func (d *dashboard) show(id string) {
	for _, item := range d.nav {
		object := d.pages[item.ID].object()
		button := d.buttons[item.ID]
		if item.ID == id {
			object.Show()
			button.Importance = widget.HighImportance
			d.title.SetText(item.Label)
		} else {
			object.Hide()
			button.Importance = widget.LowImportance
		}
		button.Refresh()
	}
	d.current = id
	d.tintNavigation()
	if d.root != nil {
		d.root.Refresh()
	}
}

func (d *dashboard) menu() *fyne.MainMenu {
	terminal := fyne.NewMenuItem("Open terminal UI", d.openTerminal)
	quit := fyne.NewMenuItem("Quit", func() { d.close("") })
	quit.IsQuit = true
	view := fyne.NewMenu("View")
	for index, item := range d.nav {
		id := item.ID
		entry := fyne.NewMenuItem(item.Label, func() { d.show(id) })
		entry.Shortcut = &desktop.CustomShortcut{KeyName: fyne.KeyName(strconv.Itoa(index + 1)), Modifier: fyne.KeyModifierShortcutDefault}
		view.Items = append(view.Items, entry)
	}
	view.Items = append(view.Items, fyne.NewMenuItemSeparator(),
		fyne.NewMenuItem("Collapse or expand the sidebar", d.toggleSidebar),
		fyne.NewMenuItem("Refresh now", func() { d.done("Refreshed") }))
	help := fyne.NewMenu("Help",
		fyne.NewMenuItem("Usage guide", func() { d.showAbout(aboutGuide) }),
		fyne.NewMenuItem("Keyboard shortcuts", func() { d.showAbout(aboutShortcuts) }),
		fyne.NewMenuItem("Diagnostics", func() { d.showAbout(aboutDiagnostics) }),
		fyne.NewMenuItem("About "+meta.AppName, func() { d.showAbout(aboutOverview) }))
	return fyne.NewMainMenu(fyne.NewMenu("Menu", terminal, fyne.NewMenuItemSeparator(), quit), view, help)
}

func (d *dashboard) showAbout(tab int) {
	d.show(app.NavAbout)
	d.aboutP.tabs.SelectIndex(tab)
}

func (d *dashboard) openTerminal() {
	if d.options.InTerminal {
		d.close(launcher.ModeTUI)
		return
	}
	if d.options.OpenTerminal == nil {
		d.say(theme.SeverityHigh, "No terminal window can be opened from here. Start %s --ui tui in a terminal.", meta.Slug)
		return
	}
	if err := d.options.OpenTerminal(); d.fail(err) {
		return
	}
	d.close("")
}

func (d *dashboard) shortcuts() {
	surface := d.window.Canvas()
	bind := func(key fyne.KeyName, action func()) {
		surface.AddShortcut(&desktop.CustomShortcut{KeyName: key, Modifier: fyne.KeyModifierShortcutDefault}, func(fyne.Shortcut) { action() })
	}
	for index, item := range d.nav {
		id := item.ID
		bind(fyne.KeyName(strconv.Itoa(index+1)), func() { d.show(id) })
	}
	bind(fyne.KeyF, func() { surface.Focus(d.search) })
	bind(fyne.KeyComma, func() { d.show(app.NavSettings) })
	bind(fyne.KeyN, func() {
		d.show(app.NavRecords)
		d.recordsP.add()
	})
	bind(fyne.KeyR, func() { d.done("Refreshed") })
	bind(fyne.KeyB, d.toggleSidebar)
	bind(fyne.KeyQ, func() { d.close("") })
	surface.SetOnTypedKey(func(event *fyne.KeyEvent) {
		switch event.Name {
		case fyne.KeyF1:
			d.showAbout(aboutGuide)
		case fyne.KeyDelete:
			if d.current == app.NavRecords {
				d.recordsP.remove()
			}
		case fyne.KeyEscape:
			if d.current == app.NavRecords {
				d.recordsP.unselect()
			}
		}
	})
}

func (d *dashboard) saveWindow() {
	size := d.window.Canvas().Size()
	for key, value := range map[string]int{settings.KeyWindowWidth: int(size.Width), settings.KeyWindowHeight: int(size.Height)} {
		definition, _ := settings.Lookup(key)
		if value < definition.Min || value > definition.Max || d.values[key].Int() == value {
			continue
		}
		if _, err := d.app.Settings.Set(d.ctx, key, strconv.Itoa(value)); err != nil {
			return
		}
	}
}

func (d *dashboard) close(switchTo string) {
	d.stopOnce.Do(func() {
		close(d.stop)
		d.saveWindow()
		d.switchTo = switchTo
		if d.finished != nil {
			d.finished(switchTo)
		}
	})
}

func (d *dashboard) ask(title, statement, accept string, action func() (string, error)) {
	run := func() {
		text, err := action()
		if d.fail(err) {
			return
		}
		d.done("%s", text)
	}
	if !d.values[settings.KeyConfirm].Bool() {
		run()
		return
	}
	d.confirm = newConfirmDialog(d.window, title, statement, accept, true, run, func() { d.confirm = nil })
}

func (d *dashboard) proceed(title, statement, accept string, action func() (string, error)) {
	d.confirm = newConfirmDialog(d.window, title, statement, accept, false, func() {
		text, err := action()
		if d.fail(err) {
			return
		}
		d.done("%s", text)
	}, func() { d.confirm = nil })
}

func (d *dashboard) showForm(title, saveLabel string, fields []formField, submit func(values map[string]string) error) {
	d.form = newFormDialog(d.window, title, saveLabel, fields, submit)
	d.form.closed = func() { d.form = nil }
}

func (d *dashboard) welcome() {
	d.confirm = newConfirmDialog(d.window, "Welcome to "+meta.AppName,
		"A new database was created at "+d.app.Location().Path+".\n\nEverything stays in this one file on your computer. You can move it later in Settings, Data, Database location.\n\nLoad demo data to look around with labeled sample records, or cancel to start with an empty database.",
		"Load demo data", false, d.loadDemo, func() { d.confirm = nil })
}

func (d *dashboard) loadDemo() {
	added, err := d.app.Records.LoadDemo(d.ctx)
	if d.fail(err) {
		return
	}
	if added == 0 {
		d.done("The demo data is already loaded")
		return
	}
	d.done("Added %d demo records. Remove them in Settings, Data, Remove demo data.", added)
}

type overviewPage struct {
	d       *dashboard
	root    fyne.CanvasObject
	numbers map[string]*widget.Label
	content *fyne.Container
	table   *fyne.Container
	empty   *widget.Label
	demo    *widget.Label
	load    *widget.Button
}

func newOverviewPage(d *dashboard) *overviewPage {
	p := &overviewPage{d: d, numbers: map[string]*widget.Label{}}
	cards := container.NewGridWithColumns(4)
	for _, item := range [][3]string{{"open", "Open records", "not done and enabled"}, {"urgent", "Urgent records", "open with high or critical severity"}, {"unread", "Unread notifications", "see Notifications"}, {"disabled", "Disabled records", "kept but not counted"}} {
		number := widget.NewLabel("0")
		number.TextStyle = fyne.TextStyle{Bold: true}
		number.SizeName = fynetheme.SizeNameHeadingText
		p.numbers[item[0]] = number
		cards.Add(panel(container.NewVBox(widget.NewLabel(item[1]), number, muted(item[2]))))
	}
	p.table = container.NewVBox()
	p.empty = wrapped("No records yet. Open Records and choose Add record, or load labeled demo data to look around.")
	p.demo = muted("")
	p.load = widget.NewButtonWithIcon("Load demo data", glyph("download"), d.loadDemo)
	p.content = container.NewVBox(heading("What needs attention"), cards, heading("Enabled records by status and severity"), p.empty, p.table, p.demo, container.NewHBox(p.load))
	p.root = container.NewVScroll(p.content)
	return p
}

func (p *overviewPage) object() fyne.CanvasObject { return p.root }

func (p *overviewPage) refresh() {
	o := p.d.overview
	p.numbers["open"].SetText(strconv.Itoa(o.Open))
	p.numbers["urgent"].SetText(strconv.Itoa(o.Urgent))
	p.numbers["unread"].SetText(strconv.Itoa(o.UnreadNotifications))
	p.numbers["disabled"].SetText(strconv.Itoa(o.Disabled))
	layout := &columns{weights: []float32{1, 1, 1}, fixed: []float32{120, 120, 60}}
	p.table.RemoveAll()
	if o.Total() == 0 {
		p.empty.Show()
	} else {
		p.empty.Hide()
		p.table.Add(headerRow(layout, "Status", "Severity", "Records"))
		for _, row := range o.Breakdown {
			status, _, _ := iconLabel(icon(statusIcon(row.Status), p.d.tokens.Palette.TextMuted), records.StatusLabel(row.Status))
			severity, _, _ := iconLabel(icon(row.Severity.Icon(), p.d.tokens.Semantic.Severity(row.Severity)), row.Severity.Label())
			p.table.Add(container.New(layout, status, severity, cell(strconv.Itoa(row.Total))))
		}
	}
	if o.Demo > 0 {
		p.demo.SetText(fmt.Sprintf("Demo data is loaded: %d sample records whose names start with \"Demo:\". Remove them in Settings, Data, Remove demo data.", o.Demo))
		p.demo.Show()
		p.load.Hide()
	} else {
		p.demo.Hide()
		p.load.Show()
	}
	p.table.Refresh()
	p.content.Refresh()
}
