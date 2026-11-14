package gui

import (
	"fmt"
	"image/color"
	"strings"

	"fyne.io/fyne/v2"
	"fyne.io/fyne/v2/canvas"
	"fyne.io/fyne/v2/container"
	"fyne.io/fyne/v2/dialog"
	"fyne.io/fyne/v2/widget"

	"github.com/NexzaDev/templateapp/internal/app"
	"github.com/NexzaDev/templateapp/internal/guide"
	"github.com/NexzaDev/templateapp/internal/meta"
	"github.com/NexzaDev/templateapp/internal/settings"
	"github.com/NexzaDev/templateapp/internal/theme"
	"github.com/NexzaDev/templateapp/internal/userconfig"
)

type settingRow struct {
	key     string
	entry   *widget.Entry
	choice  *widget.Select
	check   *widget.Check
	labels  []string
	options []string
	source  *widget.Label
	problem *widget.Label
	reset   *widget.Button
	dirty   bool
	loading bool
}

type settingsPage struct {
	d         *dashboard
	root      fyne.CanvasObject
	rows      map[string]*settingRow
	location  *widget.Label
	locSource *widget.Label
	demo      *widget.Label
	notifier  *widget.Check
	notifierL *widget.Label
	launcher  *widget.Select
	launcherL *widget.Label
	loading   bool
}

var frontEndOptions = []string{"", userconfig.FrontEndGUI, userconfig.FrontEndTUI}
var frontEndLabels = []string{"Ask every time", "Dashboard window", "Terminal UI"}

func optionLabel(key, option string) string {
	if key == settings.KeyTheme {
		return theme.Label(option)
	}
	if option == "" {
		return option
	}
	return strings.ToUpper(option[:1]) + option[1:]
}

func newSettingsPage(d *dashboard) *settingsPage {
	p := &settingsPage{d: d, rows: map[string]*settingRow{}}
	list, err := d.app.Settings.List(d.ctx)
	d.fail(err)
	groups := map[string]*fyne.Container{}
	var order []string
	group := func(name string) *fyne.Container {
		if box, found := groups[name]; found {
			return box
		}
		box := container.NewVBox()
		groups[name] = box
		order = append(order, name)
		return box
	}
	layout := &columns{weights: []float32{2, 3, 2, 0}, fixed: []float32{170, 180, 120, 80}}
	for _, value := range list {
		if value.Unknown {
			continue
		}
		row := p.newRow(value)
		var control fyne.CanvasObject
		switch {
		case row.check != nil:
			control = row.check
		case row.choice != nil:
			control = row.choice
		default:
			control = row.entry
			if value.Key == settings.KeyAccent {
				pick := widget.NewButtonWithIcon("Pick", glyph("palette"), func() {
					picker := dialog.NewColorPicker("Custom accent", "The color is adjusted when needed to stay readable.", func(chosen color.Color) {
						r, g, b, _ := chosen.RGBA()
						row.entry.SetText(theme.Color{R: uint8(r >> 8), G: uint8(g >> 8), B: uint8(b >> 8)}.Hex())
						p.save(row)
					}, d.window)
					picker.Advanced = true
					picker.Show()
				})
				control = container.NewBorder(nil, nil, nil, pick, row.entry)
			}
		}
		label := widget.NewLabel(value.Label)
		label.Truncation = fyne.TextTruncateEllipsis
		box := group(value.Group)
		box.Add(container.New(layout, label, control, row.source, row.reset))
		help := muted(value.Help)
		box.Add(help)
		box.Add(row.problem)
	}
	p.location = wrapped("")
	p.locSource = muted("")
	change := widget.NewButton("Change…", p.moveDatabase)
	p.demo = widget.NewLabel("")
	removeDemo := widget.NewButtonWithIcon("Remove demo data", glyph("trash"), p.removeDemo)
	data := group("Data")
	data.Add(container.NewBorder(nil, nil, widget.NewLabel("Database location"), change, p.location))
	data.Add(p.locSource)
	data.Add(container.NewBorder(nil, nil, nil, removeDemo, p.demo))
	p.notifier = widget.NewCheck("Start at sign-in", func(on bool) {
		if !p.loading {
			p.setNotifier(on)
		}
	})
	p.notifierL = muted("")
	notifications := group("Notifications")
	notifications.Add(p.notifier)
	notifications.Add(p.notifierL)
	p.launcher = widget.NewSelect(frontEndLabels, func(label string) {
		if p.loading {
			return
		}
		for index, known := range frontEndLabels {
			if known == label {
				if d.fail(d.app.RememberFrontEnd(frontEndOptions[index])) {
					return
				}
				d.done("Open with: %s", label)
			}
		}
	})
	p.launcherL = muted("")
	launcherBox := group("Launcher")
	launcherBox.Add(container.NewBorder(nil, nil, widget.NewLabel("Open with"), nil, p.launcher))
	launcherBox.Add(p.launcherL)
	content := container.NewVBox()
	for _, name := range order {
		content.Add(heading(name))
		content.Add(panel(groups[name]))
	}
	p.root = container.NewVScroll(content)
	return p
}

func (p *settingsPage) newRow(value settings.Value) *settingRow {
	row := &settingRow{key: value.Key}
	row.source = muted("")
	row.source.Wrapping = fyne.TextWrapOff
	row.source.Truncation = fyne.TextTruncateEllipsis
	row.problem = widget.NewLabel("")
	row.problem.Importance = widget.DangerImportance
	row.problem.Wrapping = fyne.TextWrapWord
	row.problem.Hide()
	row.reset = widget.NewButton("Reset", func() {
		row.dirty = false
		row.problem.Hide()
		reset, err := p.d.app.Settings.Reset(p.d.ctx, row.key)
		if p.d.fail(err) {
			return
		}
		p.d.done("%s is back to its default", reset.Label)
	})
	switch value.Kind {
	case settings.KindBool:
		row.check = widget.NewCheck("", func(on bool) {
			if !row.loading {
				row.dirty = true
				p.save(row)
			}
		})
	case settings.KindEnum:
		row.options = value.Options
		for _, option := range value.Options {
			row.labels = append(row.labels, optionLabel(value.Key, option))
		}
		row.choice = widget.NewSelect(row.labels, func(string) {
			if !row.loading {
				row.dirty = true
				p.save(row)
			}
		})
	default:
		row.entry = widget.NewEntry()
		row.entry.OnChanged = func(string) {
			if !row.loading {
				row.dirty = true
			}
		}
		row.entry.OnSubmitted = func(string) { p.save(row) }
	}
	p.rows[value.Key] = row
	return row
}

func (row *settingRow) text() string {
	switch {
	case row.check != nil:
		if row.check.Checked {
			return "true"
		}
		return "false"
	case row.choice != nil:
		for index, label := range row.labels {
			if label == row.choice.Selected {
				return row.options[index]
			}
		}
		return ""
	}
	return row.entry.Text
}

func (row *settingRow) show(value settings.Value) {
	stored := value.Default
	if value.HasStored {
		stored = value.Stored
	}
	row.loading = true
	switch {
	case row.check != nil:
		row.check.SetChecked(stored == "true")
	case row.choice != nil:
		for index, option := range row.options {
			if option == stored {
				row.choice.SetSelected(row.labels[index])
			}
		}
	default:
		row.entry.SetText(stored)
	}
	row.loading = false
}

func (p *settingsPage) save(row *settingRow) {
	saved, err := p.d.app.Settings.Set(p.d.ctx, row.key, row.text())
	if err != nil {
		row.problem.SetText("Not saved: " + err.Error())
		row.problem.Show()
		return
	}
	row.dirty = false
	row.problem.Hide()
	p.d.done("Saved %s", saved.Label)
	if saved.OverriddenBy != "" {
		p.d.say(theme.SeverityHigh, "Saved %s, but the %s still takes precedence", saved.Label, saved.OverriddenBy)
	}
	if saved.Key == settings.KeyAccent {
		for _, warning := range p.d.tokens.Warnings {
			p.d.say(theme.SeverityHigh, "%s", warning)
		}
	}
}

func (p *settingsPage) object() fyne.CanvasObject { return p.root }

func (p *settingsPage) refresh() {
	list, err := p.d.app.Settings.List(p.d.ctx)
	if p.d.fail(err) {
		return
	}
	for _, value := range list {
		row, found := p.rows[value.Key]
		if !found {
			continue
		}
		if !row.dirty {
			row.show(value)
		}
		source := string(value.Source)
		if value.OverriddenBy != "" {
			source = "overridden by " + value.OverriddenBy + ": " + value.Raw
		}
		row.source.SetText(source)
		if value.OverrideError != "" {
			row.problem.SetText(value.OverrideError)
			row.problem.Show()
		}
		if value.HasStored {
			row.reset.Enable()
		} else {
			row.reset.Disable()
		}
	}
	p.loading = true
	defer func() { p.loading = false }()
	location := p.d.app.Location()
	p.location.SetText(location.Path)
	if location.OverriddenBy != "" {
		p.locSource.SetText("Set by the " + location.OverriddenBy + ". Remove the override to change the location here.")
	} else {
		p.locSource.SetText("Change copies the database to a new place and uses it, or switches to another database. The old file is never deleted.")
	}
	p.demo.SetText(fmt.Sprintf("%d demo records", p.d.overview.Demo))
	if p.d.options.Autostart == nil {
		p.notifier.Disable()
		p.notifierL.SetText("This build cannot manage the sign-in entry.")
	} else {
		installed, path, err := p.d.options.Autostart().Status()
		p.notifier.SetChecked(installed)
		switch {
		case err != nil:
			p.notifierL.SetText(err.Error())
		case installed:
			p.notifierL.SetText("On: " + path + ". The notifier shows system notifications while the application is closed. It needs no administrator rights and makes no network requests.")
		default:
			p.notifierL.SetText("Off. Turn it on to see system notifications while the application is closed. It adds a per-user entry, needs no administrator rights and makes no network requests.")
		}
	}
	for index, option := range frontEndOptions {
		if option == location.Config.FrontEnd {
			p.launcher.SetSelected(frontEndLabels[index])
		}
	}
	if value := p.d.options.Getenv(meta.EnvName("ui")); value != "" {
		p.launcherL.SetText("Overridden by the environment variable " + meta.EnvName("ui") + "=" + value + ".")
	} else {
		p.launcherL.SetText("Which front end opens when the application starts without a command.")
	}
}

func (p *settingsPage) moveDatabase() {
	p.d.showForm("Database location", "Continue", []formField{
		{key: "path", label: "New location", browse: "save", hint: "Full path of the database file. The current file stays where it is."},
		{key: "mode", label: "What to do", value: "copy", options: []string{"copy", "switch"}, labels: []string{"Copy my data there and use it", "Use the database that is there, or start an empty one"}},
	}, func(values map[string]string) error {
		mode := app.MoveCopy
		if values["mode"] == "switch" {
			mode = app.MoveSwitch
		}
		plan, err := p.d.app.PlanMove(strings.TrimSpace(values["path"]), mode)
		if err != nil {
			return err
		}
		p.d.proceed("Database location", plan.Statement, "Continue", func() (string, error) {
			if err := p.d.app.Move(p.d.ctx, plan); err != nil {
				return "", err
			}
			return "Now using " + p.d.app.Location().Path, nil
		})
		return nil
	})
}

func (p *settingsPage) removeDemo() {
	count, err := p.d.app.Records.RemoveDemo(p.d.ctx, true)
	if p.d.fail(err) {
		return
	}
	if count == 0 {
		p.d.say(theme.SeverityInfo, "There is no demo data to remove")
		return
	}
	p.d.ask("Remove demo data", fmt.Sprintf("This removes %d demo records. Your own records are not touched.", count), "Remove", func() (string, error) {
		removed, err := p.d.app.Records.RemoveDemo(p.d.ctx, false)
		if err != nil {
			return "", err
		}
		return fmt.Sprintf("Removed %d demo records", removed), nil
	})
}

func (p *settingsPage) setNotifier(on bool) {
	entry := p.d.options.Autostart()
	revert := func() {
		p.loading = true
		p.notifier.SetChecked(!on)
		p.loading = false
	}
	if !on {
		if _, _, err := entry.Uninstall(false); p.d.fail(err) {
			revert()
			return
		}
		p.d.done("The notifier no longer starts at sign-in")
		return
	}
	target, err := entry.Install(true)
	if p.d.fail(err) {
		revert()
		return
	}
	p.d.confirm = newConfirmDialog(p.d.window, "Start at sign-in", "This writes "+target+" so that \""+meta.Slug+" notifier run\" starts when you sign in. It needs no administrator rights and can be turned off here at any time.", "Turn on", false, func() {
		if _, err := entry.Install(false); p.d.fail(err) {
			revert()
			return
		}
		p.d.done("The notifier starts the next time you sign in")
	}, func() {
		p.d.confirm = nil
		p.refresh()
	})
}

const (
	aboutOverview = iota
	aboutGuide
	aboutShortcuts
	aboutDiagnostics
	aboutLicenses
)

type aboutPage struct {
	icon        *canvas.Image
	d           *dashboard
	root        fyne.CanvasObject
	tabs        *container.AppTabs
	usage       guide.Guide
	hits        []int
	search      *widget.Entry
	sections    *widget.List
	body        *widget.RichText
	bodyScroll  *container.Scroll
	noHits      *widget.Label
	diagnostics *widget.Label
	report      app.Diagnostics
}

func newAboutPage(d *dashboard) *aboutPage {
	p := &aboutPage{d: d, usage: app.UsageGuide(), icon: appIconImage(64)}
	link := func(label string, target func() app.Link) *widget.Button {
		return widget.NewButtonWithIcon(label, glyph("monitor"), func() { p.open(target()) })
	}
	source := link("Source Code", app.SourceLink)
	advisory := link("Report a vulnerability privately", app.AdvisoryLink)
	bug := widget.NewButtonWithIcon("Found a bug? Please report it on GitHub.", glyph("message"), p.reportBug)
	overview := container.NewVBox(
		container.NewBorder(nil, nil, p.icon, nil, container.NewVBox(heading(meta.AppName+" "+meta.Version), wrapped(meta.Tagline))), widget.NewLabel("License: "+meta.License),
		container.NewHBox(source), muted(app.SourceLink().URL),
		container.NewHBox(bug), muted("The exact text of the report is shown before your browser opens."),
		container.NewHBox(advisory), muted(app.AdvisoryLink().URL),
		widget.NewSeparator(), muted("The application makes no network requests. Links open in your browser only when you choose them."))
	p.search = widget.NewEntry()
	p.search.SetPlaceHolder("Search the guide")
	p.search.OnChanged = func(string) { p.filter() }
	p.sections = widget.NewList(
		func() int { return len(p.hits) },
		func() fyne.CanvasObject { return cell("") },
		func(id widget.ListItemID, object fyne.CanvasObject) {
			if id >= len(p.hits) {
				return
			}
			section := p.usage.Sections[p.hits[id]]
			object.(*widget.Label).SetText(strings.Repeat("   ", max(0, section.Level-1)) + section.Title)
		})
	p.sections.OnSelected = func(id widget.ListItemID) {
		if id < len(p.hits) {
			p.read(p.hits[id])
		}
	}
	p.body = widget.NewRichText()
	p.body.Wrapping = fyne.TextWrapWord
	p.bodyScroll = container.NewVScroll(p.body)
	p.noHits = wrapped("No section mentions these words. Clear the search box to see the contents.")
	p.noHits.Hide()
	contents := container.NewBorder(p.search, nil, nil, nil, container.NewStack(p.sections, container.NewVBox(p.noHits)))
	usage := container.NewHSplit(contents, p.bodyScroll)
	usage.SetOffset(0.3)
	shortcutLayout := &columns{weights: []float32{1, 1, 3}, fixed: []float32{140, 120, 200}}
	shortcuts := container.NewVBox(headerRow(shortcutLayout, "Dashboard", "Terminal UI", "Action"))
	for _, shortcut := range app.Shortcuts() {
		shortcuts.Add(container.New(shortcutLayout, cell(shortcut.Keys), cell(shortcut.Terminal), cell(shortcut.Action)))
	}
	p.diagnostics = widget.NewLabel("")
	p.diagnostics.TextStyle = fyne.TextStyle{Monospace: true}
	p.diagnostics.Selectable = true
	copyButton := widget.NewButtonWithIcon("Copy diagnostics", glyph("copy"), func() {
		d.fyneApp.Clipboard().SetContent(p.diagnostics.Text)
		d.say(theme.SeverityLow, "Diagnostics copied")
	})
	check := widget.NewButton("Check the database", func() {
		p.refresh()
		if p.report.Integrity == "ok" {
			d.say(theme.SeverityLow, "The database is healthy")
		} else {
			d.say(theme.SeverityCritical, "The integrity check failed: %s", p.report.Integrity)
		}
	})
	diagnostics := container.NewBorder(container.NewHBox(check, copyButton), muted("Diagnostics stay on this computer. A bug report includes only the lines shown in its preview."), nil, nil, container.NewVScroll(p.diagnostics))
	licenses := widget.NewLabel(app.LicenseText() + "\n\n" + app.NoticesText())
	licenses.Wrapping = fyne.TextWrapWord
	p.tabs = container.NewAppTabs(
		container.NewTabItem("Overview", container.NewVScroll(overview)),
		container.NewTabItem("Usage guide", usage),
		container.NewTabItem("Shortcuts", container.NewVScroll(shortcuts)),
		container.NewTabItem("Diagnostics", diagnostics),
		container.NewTabItem("Licenses and notices", container.NewVScroll(licenses)))
	p.root = p.tabs
	p.filter()
	p.read(0)
	return p
}

func (p *aboutPage) object() fyne.CanvasObject { return p.root }

func (p *aboutPage) refresh() {
	p.report = p.d.app.Diagnostics(p.d.ctx, true)
	var text strings.Builder
	for _, line := range p.report.Lines() {
		fmt.Fprintf(&text, "%-30s %s\n", line[0], line[1])
	}
	p.diagnostics.SetText(text.String())
}

func (p *aboutPage) filter() {
	p.hits = p.hits[:0]
	query := strings.TrimSpace(p.search.Text)
	if query == "" {
		for index := range p.usage.Sections {
			p.hits = append(p.hits, index)
		}
	} else {
		for _, hit := range p.usage.Search(query) {
			p.hits = append(p.hits, hit.Section)
		}
	}
	if len(p.hits) == 0 {
		p.noHits.Show()
	} else {
		p.noHits.Hide()
	}
	p.sections.UnselectAll()
	p.sections.Refresh()
}

func (p *aboutPage) read(section int) {
	if section >= len(p.usage.Sections) {
		return
	}
	chosen := p.usage.Sections[section]
	p.body.ParseMarkdown("## " + chosen.Title + "\n\n" + chosen.Body)
	p.bodyScroll.ScrollToTop()
}

func (p *aboutPage) open(link app.Link) {
	if !link.Allowed {
		p.d.say(theme.SeverityCritical, "%s", link.Reason)
		return
	}
	if p.d.options.OpenURL == nil {
		p.d.say(theme.SeverityHigh, "No browser can be started from here. Copy this link: %s", link.URL)
		return
	}
	if err := p.d.options.OpenURL(link.URL); err != nil {
		p.d.say(theme.SeverityHigh, "%s: %s", err.Error(), link.URL)
		return
	}
	p.d.say(theme.SeverityLow, "Opened %s in your browser", link.Label)
}

func (p *aboutPage) reportBug() {
	issue := p.d.app.Diagnostics(p.d.ctx, true).IssueReport()
	text := "The report form opens in your browser, prefilled with exactly this text and nothing else. You can edit it before you submit it.\n\n" + issue.Body
	p.d.confirm = newConfirmDialog(p.d.window, "Report a bug", text, "Open the report form", false, func() {
		p.open(app.ExternalLink("the report form", issue.URL))
	}, func() { p.d.confirm = nil })
}
