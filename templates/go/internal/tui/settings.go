package tui

import (
	"errors"
	"fmt"
	"runtime"
	"strings"

	"github.com/NexzaDev/templateapp/internal/app"
	"github.com/NexzaDev/templateapp/internal/guide"
	"github.com/NexzaDev/templateapp/internal/meta"
	"github.com/NexzaDev/templateapp/internal/settings"
	"github.com/NexzaDev/templateapp/internal/theme"
	"github.com/NexzaDev/templateapp/internal/userconfig"
)

type settingRow struct {
	group  string
	label  string
	value  string
	source string
	help   string
	key    string
	action string
}

const (
	actionDatabase = "database"
	actionDemo     = "demo"
	actionNotifier = "notifier"
	actionLauncher = "launcher"
)

type settingsPage struct {
	rows   []settingRow
	values map[string]settings.Value
	cursor int
	offset int
}

func yesNo(value string) string {
	switch value {
	case "true":
		return "Yes"
	case "false":
		return "No"
	}
	return value
}

func frontEndLabel(mode string) string {
	switch mode {
	case userconfig.FrontEndGUI:
		return "Dashboard window"
	case userconfig.FrontEndTUI:
		return "Terminal UI"
	}
	return "Ask every time"
}

func (p *settingsPage) load(m *Model) {
	list, err := m.app.Settings.List(m.ctx)
	if m.fail(err) {
		return
	}
	p.values = map[string]settings.Value{}
	byGroup := map[string][]settingRow{}
	for _, value := range list {
		p.values[value.Key] = value
		shown := yesNo(value.Raw)
		if value.Key == settings.KeyTheme {
			shown = theme.Label(value.Raw)
		}
		if shown == "" {
			shown = "(empty)"
		}
		source := string(value.Source)
		help := value.Help
		if value.OverriddenBy != "" {
			source = "overridden"
			stored := value.Default
			if value.HasStored {
				stored = value.Stored
			}
			help += fmt.Sprintf(" Overridden by the %s. The saved value is %q and applies when the override is removed.", value.OverriddenBy, stored)
		}
		if value.OverrideError != "" {
			help += " " + value.OverrideError
		}
		byGroup[value.Group] = append(byGroup[value.Group], settingRow{group: value.Group, label: value.Label, value: shown, source: source, help: help, key: value.Key})
	}
	location := m.app.Location()
	source := location.Source
	databaseHelp := "In use: " + location.Path + ". Enter copies the database to a new place and uses it, or switches to another database. The old file is never deleted."
	if location.OverriddenBy != "" {
		source = "overridden"
		databaseHelp = "In use: " + location.Path + ". Set by the " + location.OverriddenBy + ". Remove the override to change the location here."
	}
	byGroup["Data"] = append(byGroup["Data"],
		settingRow{group: "Data", label: "Database location", value: location.Path, source: source, help: databaseHelp, action: actionDatabase},
		settingRow{group: "Data", label: "Remove demo data", value: fmt.Sprintf("%d demo records", m.overview.Demo), source: "action", help: "Enter removes the sample records whose names start with \"Demo:\" after confirmation. Your own records are not touched.", action: actionDemo})
	notifier, notifierHelp := "Not available", ""
	if m.options.Autostart != nil {
		installed, path, err := m.options.Autostart().Status()
		switch {
		case err != nil:
			notifier, notifierHelp = "Unknown", err.Error()+" "
		case installed:
			notifier, notifierHelp = "On", "Entry: "+path+". "
		default:
			notifier = "Off"
		}
	}
	byGroup["Notifications"] = append(byGroup["Notifications"], settingRow{group: "Notifications", label: "Notifier at sign-in", value: notifier, source: "action", help: notifierHelp + "Enter turns the per-user sign-in entry on or off. The notifier shows system notifications while the application is closed. It needs no administrator rights and makes no network requests.", action: actionNotifier})
	launcherSource, launcherHelp := "default", ""
	if location.Config.FrontEnd != "" {
		launcherSource = "saved"
	}
	if value := m.options.Getenv(meta.EnvName("ui")); value != "" {
		launcherSource = "overridden"
		launcherHelp = "Overridden by the environment variable " + meta.EnvName("ui") + "=" + value + ". "
	}
	byGroup["Launcher"] = append(byGroup["Launcher"], settingRow{group: "Launcher", label: "Open with", value: frontEndLabel(location.Config.FrontEnd), source: launcherSource, help: launcherHelp + "Which front end opens when the application starts without a command. Enter changes it, u asks again at every start.", action: actionLauncher})
	groups := app.SettingGroups(list)
	for _, extra := range []string{"Data", "Notifications", "Launcher"} {
		found := false
		for _, group := range groups {
			found = found || group == extra
		}
		if !found {
			groups = append(groups, extra)
		}
	}
	p.rows = p.rows[:0]
	for _, group := range groups {
		p.rows = append(p.rows, byGroup[group]...)
	}
	p.cursor = max(0, min(p.cursor, len(p.rows)-1))
}

func (p *settingsPage) key(m *Model, key string) {
	switch key {
	case "up", "k":
		p.cursor = max(0, p.cursor-1)
		return
	case "down", "j":
		p.cursor = max(0, min(len(p.rows)-1, p.cursor+1))
		return
	case "pgup":
		p.cursor = max(0, p.cursor-10)
		return
	case "pgdown":
		p.cursor = max(0, min(len(p.rows)-1, p.cursor+10))
		return
	}
	if p.cursor >= len(p.rows) {
		return
	}
	row := p.rows[p.cursor]
	switch {
	case row.key != "" && key == "enter":
		p.edit(m, p.values[row.key])
	case row.key != "" && key == "u":
		value, err := m.app.Settings.Reset(m.ctx, row.key)
		if m.fail(err) {
			return
		}
		m.changed("%s is back to its default", value.Label)
		if value.OverriddenBy != "" {
			m.say(noticeWarning, "%s is back to its default, but the %s still takes precedence", value.Label, value.OverriddenBy)
		}
	case row.action == actionDatabase && key == "enter":
		p.moveForm(m)
	case row.action == actionDemo && key == "enter":
		count, err := m.app.Records.RemoveDemo(m.ctx, true)
		if m.fail(err) {
			return
		}
		if count == 0 {
			m.say(noticeInfo, "There is no demo data to remove")
			return
		}
		m.ask("Remove demo data", fmt.Sprintf("This removes %d demo records. Your own records are not touched.", count), "remove them", func() (string, error) {
			removed, err := m.app.Records.RemoveDemo(m.ctx, false)
			if err != nil {
				return "", err
			}
			return fmt.Sprintf("Removed %d demo records", removed), nil
		})
	case row.action == actionNotifier && key == "enter":
		p.toggleNotifier(m)
	case row.action == actionLauncher && (key == "enter" || key == "u"):
		target := ""
		if key == "enter" {
			target = next([]string{"", userconfig.FrontEndGUI, userconfig.FrontEndTUI}, m.app.Location().Config.FrontEnd)
		}
		if m.fail(m.app.RememberFrontEnd(target)) {
			return
		}
		m.changed("Open with: %s", frontEndLabel(target))
	}
}

func (p *settingsPage) edit(m *Model, value settings.Value) {
	var item field
	help := value.Help
	if value.OverriddenBy != "" {
		help += " This value is overridden by the " + value.OverriddenBy + "; your change is saved and applies when the override is removed."
	}
	current := value.Stored
	if !value.HasStored {
		current = value.Default
	}
	switch value.Kind {
	case settings.KindBool:
		item = choiceField("value", value.Label, current, help, []string{"true", "false"}, []string{"Yes", "No"})
	case settings.KindEnum:
		var labels []string
		for _, option := range value.Options {
			if value.Key == settings.KeyTheme {
				labels = append(labels, theme.Label(option))
			} else {
				labels = append(labels, option)
			}
		}
		item = choiceField("value", value.Label, current, help, value.Options, labels)
	case settings.KindInt:
		item = textField("value", value.Label, current, fmt.Sprintf("%s Whole number from %d to %d.", help, value.Min, value.Max), 12)
	default:
		item = textField("value", value.Label, current, help, 200)
	}
	m.form = &form{title: value.Group + ", " + value.Label, fields: []field{item}, submit: func(values map[string]string) error {
		saved, err := m.app.Settings.Set(m.ctx, value.Key, values["value"])
		if err != nil {
			return err
		}
		m.changed("Saved %s", saved.Label)
		if saved.OverriddenBy != "" {
			m.say(noticeWarning, "Saved %s, but the %s still takes precedence", saved.Label, saved.OverriddenBy)
		}
		for _, warning := range m.tokens.Warnings {
			if saved.Key == settings.KeyAccent {
				m.say(noticeWarning, "%s", warning)
			}
		}
		return nil
	}}
}

func (p *settingsPage) moveForm(m *Model) {
	m.form = &form{title: "Database location", fields: []field{
		textField("path", "New location", "", "Full path of the database file, for example "+exampleDatabasePath()+". The current file stays where it is.", 1024),
		choiceField("mode", "What to do", "copy", "", []string{"copy", "switch"}, []string{"Copy my data there and use it", "Use the database that is there, or start an empty one"}),
	}, submit: func(values map[string]string) error {
		mode := app.MoveCopy
		if values["mode"] == "switch" {
			mode = app.MoveSwitch
		}
		plan, err := m.app.PlanMove(strings.TrimSpace(values["path"]), mode)
		if err != nil {
			return err
		}
		m.confirm = &confirmation{title: "Database location", statement: plan.Statement, accept: "continue", action: func() (string, error) {
			if err := m.app.Move(m.ctx, plan); err != nil {
				return "", err
			}
			return "Now using " + m.app.Location().Path, nil
		}}
		return nil
	}}
}

func exampleDatabasePath() string {
	if runtime.GOOS == "windows" {
		return `D:\Data\` + meta.Slug + ".db"
	}
	return "/home/you/data/" + meta.Slug + ".db"
}

func (p *settingsPage) toggleNotifier(m *Model) {
	if m.options.Autostart == nil {
		m.say(noticeWarning, "This build cannot manage the sign-in entry")
		return
	}
	entry := m.options.Autostart()
	installed, path, err := entry.Status()
	if m.fail(err) {
		return
	}
	if installed {
		m.ask("Notifier at sign-in", "This removes "+path+". The notifier no longer starts when you sign in. Nothing else changes.", "remove the entry", func() (string, error) {
			if _, _, err := entry.Uninstall(false); err != nil {
				return "", err
			}
			return "The notifier no longer starts at sign-in", nil
		})
		return
	}
	target, err := entry.Install(true)
	if m.fail(err) {
		return
	}
	m.confirm = &confirmation{title: "Notifier at sign-in", statement: "This writes " + target + " so that \"" + meta.Slug + " notifier run\" starts when you sign in. It needs no administrator rights and can be removed here at any time.", accept: "write the entry", action: func() (string, error) {
		if _, err := entry.Install(false); err != nil {
			return "", err
		}
		return "The notifier starts the next time you sign in", nil
	}}
}

func (p *settingsPage) view(m *Model, width, height int) []string {
	type line struct {
		text string
		row  int
	}
	var all []line
	group := ""
	labelWidth := 0
	for _, row := range p.rows {
		labelWidth = max(labelWidth, len(row.label))
	}
	labelWidth = min(labelWidth, max(18, width-36))
	valueWidth := max(12, width-labelWidth-18)
	cursorLine := 0
	for index, row := range p.rows {
		if row.group != group {
			if group != "" {
				all = append(all, line{"", -1})
			}
			group = row.group
			all = append(all, line{m.st.heading.Render(group), -1})
		}
		value := row.value
		if len([]rune(value)) > valueWidth {
			runes := []rune(value)
			value = "…" + string(runes[len(runes)-valueWidth+1:])
		}
		text := fit(row.label, labelWidth) + "  " + fit(value, valueWidth) + "  " + row.source
		if index == p.cursor {
			cursorLine = len(all)
			all = append(all, line{m.st.selected.Render(fit("▌ "+text, width)), index})
		} else {
			all = append(all, line{"  " + fit(text, width-2), index})
		}
	}
	var help []string
	if p.cursor < len(p.rows) {
		help = wrap(p.rows[p.cursor].help, width)
	}
	rows := max(3, height-len(help)-2)
	if cursorLine < p.offset+1 {
		p.offset = max(0, cursorLine-1)
	}
	if cursorLine >= p.offset+rows {
		p.offset = cursorLine - rows + 1
	}
	var lines []string
	for index := p.offset; index < len(all) && index < p.offset+rows; index++ {
		lines = append(lines, all[index].text)
	}
	lines = append(lines, m.st.border.Render(strings.Repeat("─", width)))
	for _, text := range help {
		lines = append(lines, m.st.muted.Render(text))
	}
	return lines
}

type aboutTab int

const (
	aboutOverview aboutTab = iota
	aboutGuide
	aboutShortcuts
	aboutDiagnostics
	aboutLicenses
)

var aboutTabs = []string{"Overview", "Usage guide", "Shortcuts", "Diagnostics", "Licenses"}

type aboutPage struct {
	tab         aboutTab
	cursor      int
	scroll      int
	usage       guide.Guide
	hits        []int
	section     int
	reading     bool
	searching   bool
	search      input
	diagnostics app.Diagnostics
}

func (p *aboutPage) load(m *Model) {
	if len(p.usage.Sections) == 0 {
		p.usage = app.UsageGuide()
		p.filter()
	}
	p.diagnostics = m.app.Diagnostics(m.ctx, m.options.HasDashboard)
}

func (p *aboutPage) filter() {
	p.hits = p.hits[:0]
	query := strings.TrimSpace(p.search.value())
	if query == "" {
		for index := range p.usage.Sections {
			p.hits = append(p.hits, index)
		}
	} else {
		for _, hit := range p.usage.Search(query) {
			p.hits = append(p.hits, hit.Section)
		}
	}
	p.cursor = max(0, min(p.cursor, len(p.hits)-1))
}

type aboutLink struct {
	label string
	link  app.Link
	issue bool
}

func (p *aboutPage) links() []aboutLink {
	issue := p.diagnostics.IssueReport()
	return []aboutLink{
		{"Source Code", app.SourceLink(), false},
		{"Found a bug? Please report it on GitHub.", app.ExternalLink("Report a bug", issue.URL), true},
		{"Report a vulnerability privately", app.AdvisoryLink(), false},
	}
}

func (p *aboutPage) openLink(m *Model, link app.Link) (string, error) {
	if !link.Allowed {
		return "", errors.New(link.Reason)
	}
	if m.options.OpenURL == nil {
		return "", errors.New("no browser can be started from here. Copy this link: " + link.URL)
	}
	if err := m.options.OpenURL(link.URL); err != nil {
		return "", fmt.Errorf("%w: %s", err, link.URL)
	}
	return "Opened " + link.Label + " in the browser", nil
}

func (p *aboutPage) key(m *Model, key string, typed []rune) {
	if p.searching {
		switch key {
		case "enter":
			p.searching = false
		case "esc":
			p.searching, p.search = false, newInput("", 100)
			p.filter()
		default:
			if p.search.handle(key, typed) {
				p.cursor = 0
				p.filter()
			}
		}
		return
	}
	switch key {
	case "right", "]":
		if !p.reading {
			p.tab, p.cursor, p.scroll = aboutTab((int(p.tab)+1)%len(aboutTabs)), 0, 0
		}
		return
	case "left", "[":
		if !p.reading {
			p.tab, p.cursor, p.scroll = aboutTab((int(p.tab)+len(aboutTabs)-1)%len(aboutTabs)), 0, 0
		}
		return
	}
	switch p.tab {
	case aboutOverview:
		links := p.links()
		switch key {
		case "up", "k":
			p.cursor = max(0, p.cursor-1)
		case "down", "j":
			p.cursor = min(len(links)-1, p.cursor+1)
		case "enter":
			chosen := links[min(p.cursor, len(links)-1)]
			if !chosen.issue {
				done, err := p.openLink(m, chosen.link)
				if !m.fail(err) {
					m.say(noticeSuccess, "%s", done)
				}
				return
			}
			issue := p.diagnostics.IssueReport()
			lines := []string{"The report form opens in your browser, prefilled with exactly this text and nothing else. You can edit it before you submit it.", ""}
			lines = append(lines, strings.Split(issue.Body, "\n")...)
			lines = append(lines, "", "Link: "+issue.URL)
			m.preview = &preview{title: "Report a bug", lines: lines, accept: "open the report form in the browser", action: func() (string, error) {
				return p.openLink(m, chosen.link)
			}}
		}
	case aboutGuide:
		switch {
		case p.reading:
			switch key {
			case "esc", "backspace", "enter":
				p.reading, p.scroll = false, 0
			case "up", "k":
				p.scroll = max(0, p.scroll-1)
			case "down", "j":
				p.scroll++
			case "pgup":
				p.scroll = max(0, p.scroll-10)
			case "pgdown", " ":
				p.scroll += 10
			}
		default:
			switch key {
			case "up", "k":
				p.cursor = max(0, p.cursor-1)
			case "down", "j":
				p.cursor = max(0, min(len(p.hits)-1, p.cursor+1))
			case "/":
				p.searching = true
			case "esc":
				p.search = newInput("", 100)
				p.filter()
			case "enter":
				if p.cursor < len(p.hits) {
					p.section, p.reading, p.scroll = p.hits[p.cursor], true, 0
				}
			}
		}
	default:
		switch key {
		case "up", "k":
			p.scroll = max(0, p.scroll-1)
		case "down", "j":
			p.scroll++
		case "pgup":
			p.scroll = max(0, p.scroll-10)
		case "pgdown", " ":
			p.scroll += 10
		}
	}
}

func (p *aboutPage) view(m *Model, width, height int) []string {
	var tabs []string
	for index, title := range aboutTabs {
		if aboutTab(index) == p.tab {
			tabs = append(tabs, m.st.selected.Render("▌"+title+" "))
		} else {
			tabs = append(tabs, " "+title+" ")
		}
	}
	lines := []string{strings.Join(tabs, " "), ""}
	room := max(3, height-3)
	scrolled := func(body []string) []string {
		shown, offset := window(body, p.scroll, room)
		p.scroll = offset
		return append(lines, shown...)
	}
	switch p.tab {
	case aboutOverview:
		lines = append(lines, m.st.heading.Render(meta.AppName+" "+meta.Version), "")
		lines = append(lines, wrap(meta.Tagline, width)...)
		lines = append(lines, "", "License: "+meta.License, "")
		for index, link := range p.links() {
			text := fit(link.label, 42) + m.st.muted.Render(shorten(link.link.URL, width-46))
			if index == p.cursor {
				lines = append(lines, m.st.selected.Render("▌ "+fit(link.label, 40))+" "+m.st.muted.Render(shorten(link.link.URL, width-46)))
			} else {
				lines = append(lines, "  "+text)
			}
		}
		lines = append(lines, "", m.st.muted.Render("Enter opens the selected link in your browser. Nothing is sent by the application."))
		return lines
	case aboutGuide:
		if p.reading {
			section := p.usage.Sections[p.section]
			body := []string{m.st.heading.Render(section.Title), ""}
			body = append(body, wrap(guide.Plain(section.Body), width)...)
			body = append(body, "", m.st.muted.Render("Esc back to the contents"))
			return scrolled(body)
		}
		search := p.search.value()
		if p.searching {
			search = p.search.view(30, true)
		} else if search == "" {
			search = "press / to search"
		}
		lines = append(lines, m.st.muted.Render("Search: "+strings.TrimRight(search, " ")), "")
		if len(p.hits) == 0 {
			return append(lines, "No section mentions these words. Press Esc to show the contents.")
		}
		shown, offset := window(make([]string, len(p.hits)), max(0, p.cursor-room+4), room-2)
		for index := offset; index < offset+len(shown); index++ {
			section := p.usage.Sections[p.hits[index]]
			title := strings.Repeat("  ", max(0, section.Level-1)) + section.Title
			if index == p.cursor {
				lines = append(lines, m.st.selected.Render(fit("▌ "+title, width)))
			} else {
				lines = append(lines, "  "+title)
			}
		}
		return lines
	case aboutShortcuts:
		widths := []int{22, width - 24}
		body := []string{m.st.muted.Render(renderRow([]string{"KEY", "ACTION"}, widths))}
		for _, shortcut := range app.Shortcuts() {
			body = append(body, renderRow([]string{shortcut.Terminal, shortcut.Action}, widths))
		}
		for _, extra := range [][2]string{
			{"Tab, Shift+Tab", "Next or previous category"}, {": or F10", "Open the menu"}, {"Left, Right", "Switch sections inside a category"},
			{"e", "Records: edit the selected record"}, {"x", "Records: enable or disable the selected record"},
			{"f, v", "Records: next status filter, next severity filter"}, {"s, S", "Records: next sort column, reverse the order"},
			{"c", "Records: choose the table columns"}, {"a", "Records: show or hide disabled records"},
			{"i, o", "Records: import, export"}, {"l", "Overview: load demo data"},
			{"m, M", "Notifications: mark one as read, mark all as read"}, {"u", "Notifications: unread only. Rules and Settings: reset"},
		} {
			body = append(body, renderRow([]string{extra[0], extra[1]}, widths))
		}
		return scrolled(body)
	case aboutDiagnostics:
		var body []string
		for _, item := range p.diagnostics.Lines() {
			body = append(body, fit(item[0], 30)+" "+item[1])
		}
		body = append(body, "", m.st.muted.Render("Diagnostics stay on this computer. A bug report includes only the lines shown in its preview."))
		return scrolled(body)
	default:
		body := wrap(app.LicenseText()+"\n\n"+app.NoticesText(), width)
		return scrolled(body)
	}
}

func shorten(text string, width int) string {
	if width < 8 {
		return ""
	}
	runes := []rune(text)
	if len(runes) <= width {
		return text
	}
	return string(runes[:width-1]) + "…"
}
