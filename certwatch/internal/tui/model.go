package tui

import (
	"context"
	"fmt"
	"io"
	"strings"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"

	"github.com/NexzaDev/certwatch/internal/app"
	"github.com/NexzaDev/certwatch/internal/launcher"
	"github.com/NexzaDev/certwatch/internal/meta"
	"github.com/NexzaDev/certwatch/internal/notify"
	"github.com/NexzaDev/certwatch/internal/settings"
	"github.com/NexzaDev/certwatch/internal/store"
	"github.com/NexzaDev/certwatch/internal/theme"
)

const (
	minWidth       = 60
	minHeight      = 16
	maxNoticeLines = 3
)

type Options struct {
	App          *app.App
	Getenv       func(string) string
	Dark         func() bool
	Autostart    func() notify.Autostart
	HasDashboard bool
	OpenURL      func(url string) error
	Input        io.Reader
	Output       io.Writer
}

type noticeKind int

const (
	noticeInfo noticeKind = iota
	noticeSuccess
	noticeWarning
	noticeFailure
)

type confirmation struct {
	title     string
	statement string
	accept    string
	action    func() (string, error)
}

type preview struct {
	title   string
	lines   []string
	offset  int
	accept  string
	action  func() (string, error)
	command func() tea.Cmd
}

type menuItem struct {
	label  string
	hint   string
	action func(m *Model) tea.Cmd
}

type tickMsg time.Time

type Model struct {
	ctx      context.Context
	options  Options
	app      *app.App
	st       styles
	tokens   theme.Tokens
	values   map[string]settings.Value
	width    int
	height   int
	nav      []app.NavItem
	page     int
	sidebar  bool
	notice   string
	kind     noticeKind
	form     *form
	confirm  *confirmation
	preview  *preview
	menu     []menuItem
	menuAt   int
	welcome  bool
	switchTo string
	quitting bool
	overview app.OverviewData
	pages    map[string]*viewPage
	tile     int
	busy     string
	after    tea.Cmd
	inbox    inboxPage
	set      settingsPage
	about    aboutPage
}

func New(ctx context.Context, options Options) *Model {
	if options.Getenv == nil {
		options.Getenv = func(string) string { return "" }
	}
	m := &Model{ctx: ctx, options: options, app: options.App, nav: app.Navigation(), width: 100, height: 30, welcome: options.App.CreatedDatabase()}
	m.pages = map[string]*viewPage{}
	for _, view := range app.Views() {
		m.pages[view.ID] = newViewPage(view)
	}
	m.reload()
	if value, found := m.values[settings.KeySidebarCollapsed]; found {
		m.sidebar = value.Bool()
	}
	return m
}

func Run(ctx context.Context, options Options) (string, error) {
	model := New(ctx, options)
	programOptions := []tea.ProgramOption{tea.WithAltScreen(), tea.WithContext(ctx)}
	if options.Input != nil {
		programOptions = append(programOptions, tea.WithInput(options.Input))
	}
	if options.Output != nil {
		programOptions = append(programOptions, tea.WithOutput(options.Output))
	}
	program := tea.NewProgram(model, programOptions...)
	stop := options.App.StartScheduler(ctx, time.Minute, func(done app.Housekeeping, err error) {
		program.Send(housekeepMsg{done: done, err: err})
	})
	final, err := program.Run()
	stop()
	if err != nil {
		return "", fmt.Errorf("the terminal UI stopped: %w", err)
	}
	if finished, ok := final.(*Model); ok {
		return finished.switchTo, nil
	}
	return "", nil
}

func (m *Model) SwitchTo() string { return m.switchTo }

func (m *Model) interval() time.Duration {
	seconds := m.values[settings.KeyRefreshSeconds].Int()
	if seconds < 1 {
		seconds = 2
	}
	return time.Duration(seconds) * time.Second
}

func (m *Model) tick() tea.Cmd {
	return tea.Tick(m.interval(), func(moment time.Time) tea.Msg { return tickMsg(moment) })
}

func (m *Model) Init() tea.Cmd { return m.tick() }

func (m *Model) say(kind noticeKind, format string, arguments ...any) {
	m.kind, m.notice = kind, fmt.Sprintf(format, arguments...)
}

func (m *Model) fail(err error) bool {
	if err == nil {
		return false
	}
	if store.IsBusy(err) {
		m.say(noticeWarning, "The database is busy in another window. Nothing was lost; press r to try again.")
		return true
	}
	m.say(noticeFailure, "%s", err.Error())
	return true
}

func (m *Model) restyle() {
	values, err := m.app.Settings.Effective(m.ctx)
	if m.fail(err) {
		return
	}
	m.values = values
	dark := false
	if m.options.Dark != nil {
		dark = m.options.Dark()
	}
	tokens, err := m.app.Appearance(m.ctx, dark)
	if m.fail(err) {
		return
	}
	m.tokens = tokens
	m.st = newStyles(theme.ForTerminal(tokens, theme.DetectProfile(m.options.Getenv, values[settings.KeyMonochrome].Bool())))
}

func (m *Model) reload() {
	m.restyle()
	overview, err := m.app.Overview(m.ctx)
	if m.fail(err) {
		return
	}
	m.overview = overview
	m.tile = max(0, min(m.tile, len(overview.Tiles)-1))
	for _, page := range m.pages {
		page.load(m)
	}
	m.inbox.load(m)
	m.set.load(m)
	m.about.load(m)
	m.fail(m.app.Acknowledge(m.ctx))
}

func (m *Model) changed(format string, arguments ...any) {
	m.reload()
	m.say(noticeSuccess, format, arguments...)
}

func (m *Model) pageID() string { return m.nav[m.page].ID }

func (m *Model) open(id string) {
	for index, item := range m.nav {
		if item.ID == id {
			m.page = index
		}
	}
}

func (m *Model) ask(title, statement, accept string, action func() (string, error)) {
	if !m.values[settings.KeyConfirm].Bool() {
		m.run(action)
		return
	}
	m.confirm = &confirmation{title: title, statement: statement, accept: accept, action: action}
}

func (m *Model) run(action func() (string, error)) {
	done, err := action()
	if m.fail(err) {
		return
	}
	m.changed("%s", done)
}

func (m *Model) openMenu() {
	m.menu = nil
	if m.options.HasDashboard {
		m.menu = append(m.menu, menuItem{"Open dashboard", "Close the terminal UI and open the dashboard window", func(m *Model) tea.Cmd {
			m.switchTo = launcher.ModeGUI
			return tea.Quit
		}})
	} else {
		m.menu = append(m.menu, menuItem{"Open dashboard", "Not available: this build has no dashboard", func(m *Model) tea.Cmd {
			m.say(noticeWarning, "This build has no dashboard. Download the standard build to use the dashboard window.")
			return nil
		}})
	}
	m.menu = append(m.menu,
		menuItem{"Usage guide", "Read and search the guide offline", func(m *Model) tea.Cmd { m.openGuide(); return nil }},
		menuItem{"Keyboard shortcuts", "List every key", func(m *Model) tea.Cmd { m.open(app.NavAbout); m.about.tab = aboutShortcuts; return nil }},
		menuItem{"Settings", "Appearance, data and notifier", func(m *Model) tea.Cmd { m.open(app.NavSettings); return nil }},
		menuItem{"Refresh now", "Read the database again", func(m *Model) tea.Cmd { m.changed("Refreshed"); return nil }},
		menuItem{"Collapse or expand the sidebar", "", func(m *Model) tea.Cmd { m.toggleSidebar(); return nil }},
		menuItem{"Quit", "", func(m *Model) tea.Cmd { return tea.Quit }},
	)
	m.menuAt = 0
}

func (m *Model) openGuide() {
	m.open(app.NavAbout)
	m.about.tab = aboutGuide
}

func (m *Model) toggleSidebar() {
	m.sidebar = !m.sidebar
	value := "false"
	if m.sidebar {
		value = "true"
	}
	if _, err := m.app.Settings.Set(m.ctx, settings.KeySidebarCollapsed, value); m.fail(err) {
		return
	}
	m.fail(m.app.Acknowledge(m.ctx))
}

func (m *Model) Update(message tea.Msg) (tea.Model, tea.Cmd) {
	switch message := message.(type) {
	case tea.WindowSizeMsg:
		m.width, m.height = message.Width, message.Height
		return m, nil
	case tickMsg:
		change, err := m.app.Poll(m.ctx)
		if err == nil && change.Changed {
			m.reload()
			if m.form == nil {
				m.say(noticeInfo, "Updated by another window")
			}
		} else if err != nil && !store.IsBusy(err) {
			m.fail(err)
		}
		return m, m.tick()
	case actionDoneMsg:
		m.done(message.view, message.outcome, message.err)
		return m, nil
	case housekeepMsg:
		if message.err != nil {
			if !store.IsBusy(message.err) {
				m.say(noticeWarning, "The automatic check stopped early: %s", message.err.Error())
			}
			return m, nil
		}
		m.reload()
		switch {
		case message.done.Report.Checked > 0:
			m.say(noticeInfo, "Automatic check: %s", message.done.Report.Summary())
		case message.done.Alerts > 0:
			m.say(noticeInfo, "%d new expiry alerts", message.done.Alerts)
		}
		return m, nil
	case tea.KeyMsg:
		return m, m.key(message)
	}
	return m, nil
}

func (m *Model) key(message tea.KeyMsg) tea.Cmd {
	key := message.String()
	var typed []rune
	if message.Type == tea.KeyRunes || message.Type == tea.KeySpace {
		typed = message.Runes
	}
	if key == "ctrl+c" {
		m.quitting = true
		return tea.Quit
	}
	if m.busy != "" {
		return nil
	}
	m.notice = ""
	switch {
	case m.welcome:
		m.welcome = false
		if key == "l" {
			m.loadDemo()
		}
		return nil
	case m.form != nil:
		switch m.form.handle(key, typed) {
		case formSaved:
			m.form = nil
		case formCancelled:
			m.form = nil
			m.say(noticeInfo, "Cancelled, nothing was changed")
		}
		return m.takeAfter()
	case m.confirm != nil:
		switch key {
		case "y", "Y":
			action := m.confirm.action
			m.confirm = nil
			m.run(action)
		case "n", "N", "esc", "q":
			m.confirm = nil
			m.say(noticeInfo, "Cancelled, nothing was changed")
		}
		return nil
	case m.preview != nil:
		switch key {
		case "up", "k":
			m.preview.offset--
		case "down", "j":
			m.preview.offset++
		case "pgup":
			m.preview.offset -= 10
		case "pgdown", " ":
			m.preview.offset += 10
		case "esc", "q", "n":
			cancelled := m.preview.action != nil || m.preview.command != nil
			m.preview = nil
			if cancelled {
				m.say(noticeInfo, "Cancelled, nothing was changed")
			}
		case "enter", "y":
			if command := m.preview.command; command != nil {
				m.preview = nil
				return command()
			}
			if action := m.preview.action; action != nil {
				m.preview = nil
				return m.finish(action)
			}
			m.preview = nil
		}
		if m.preview != nil && m.preview.offset < 0 {
			m.preview.offset = 0
		}
		return nil
	case m.menu != nil:
		switch key {
		case "up", "k":
			m.menuAt = (m.menuAt + len(m.menu) - 1) % len(m.menu)
		case "down", "j":
			m.menuAt = (m.menuAt + 1) % len(m.menu)
		case "esc", "q", ":", "f10":
			m.menu = nil
		case "enter":
			action := m.menu[m.menuAt].action
			m.menu = nil
			return action(m)
		}
		return nil
	}
	if m.capturesText() {
		return m.pageKey(key, typed)
	}
	switch key {
	case "q":
		m.quitting = true
		return tea.Quit
	case "tab":
		m.page = (m.page + 1) % len(m.nav)
	case "shift+tab":
		m.page = (m.page + len(m.nav) - 1) % len(m.nav)
	case ",":
		m.open(app.NavSettings)
	case "?", "f1":
		m.openGuide()
	case "r":
		m.changed("Refreshed")
	case "b":
		m.toggleSidebar()
	case ":", "f10":
		m.openMenu()
	default:
		if len(key) == 1 && key[0] >= '0' && key[0] <= '9' {
			if index := (int(key[0]-'0') + 9) % 10; index < len(m.nav) {
				m.page = index
			}
			return nil
		}
		return m.pageKey(key, typed)
	}
	return nil
}

func (m *Model) finish(action func() (string, error)) tea.Cmd {
	m.run(action)
	return nil
}

func (m *Model) capturesText() bool {
	if page := m.pages[m.pageID()]; page != nil {
		return page.searching
	}
	return m.pageID() == app.NavAbout && m.about.searching
}

func (m *Model) pageKey(key string, typed []rune) tea.Cmd {
	if page := m.pages[m.pageID()]; page != nil {
		return page.key(m, key, typed)
	}
	switch m.pageID() {
	case app.NavOverview:
		m.overviewKey(key)
	case app.NavNotifications:
		m.inbox.key(m, key)
	case app.NavSettings:
		m.set.key(m, key)
	case app.NavAbout:
		m.about.key(m, key, typed)
	}
	return nil
}

func (m *Model) loadDemo() {
	added, err := m.app.LoadDemo(m.ctx)
	if m.fail(err) {
		return
	}
	if added == 0 {
		m.changed("The demo data is already loaded")
		return
	}
	m.changed("Added %d demo endpoints with generated certificates. They are never contacted. Remove them in Settings, Remove demo data.", added)
}

func (m *Model) hints() string {
	common := ": menu · ? guide · q quit"
	if page := m.pages[m.pageID()]; page != nil {
		if page.searching {
			return page.hints()
		}
		return page.hints() + " · " + common
	}
	switch m.pageID() {
	case app.NavOverview:
		return "Up/Down choose a count · Enter open its list · l load demo data · 1-9 and 0 pages · " + common
	case app.NavNotifications:
		if m.inbox.rules {
			return "e edit rule · u reset rule · Left inbox · " + common
		}
		return "m mark read · M mark all · u unread only · Enter open the endpoint or certificate · Right rules · " + common
	case app.NavSettings:
		return "Enter change · u reset · " + common
	case app.NavAbout:
		if m.about.searching {
			return "Type to search the guide · Enter keep · Esc clear"
		}
		return "Left/Right section · / search guide · Enter open · " + common
	}
	return common
}

func (m *Model) View() string {
	if m.quitting || m.switchTo != "" {
		return ""
	}
	if m.width < minWidth || m.height < minHeight {
		return fmt.Sprintf("%s needs a terminal of at least %d columns and %d rows.\nThis one is %d by %d. Enlarge the window, or use the commands: %s --help\n", meta.AppName, minWidth, minHeight, m.width, m.height, meta.Slug)
	}
	status := m.statusLines()
	bodyHeight := m.height - 2 - len(status)
	sidebarWidth := 20
	if m.sidebar {
		sidebarWidth = 4
	}
	contentWidth := m.width - sidebarWidth - 3
	var content []string
	switch {
	case m.welcome:
		content = m.welcomeView(contentWidth)
	case m.form != nil:
		content = m.form.view(m.st, contentWidth)
	case m.confirm != nil:
		content = m.confirmView(contentWidth)
	case m.preview != nil:
		content = m.previewView(contentWidth, bodyHeight)
	case m.menu != nil:
		content = m.menuView(contentWidth)
	default:
		content = m.pageView(contentWidth, bodyHeight)
	}
	sidebar := m.sidebarView(sidebarWidth, bodyHeight)
	var builder strings.Builder
	title := meta.AppName + " · " + m.nav[m.page].Label
	location := m.app.Location().Path
	if room := m.width - lipgloss.Width(title) - 4; lipgloss.Width(location) > room {
		location = "…" + location[len(location)-max(0, min(len(location), room-1)):]
		if room < 8 {
			location = ""
		}
	}
	builder.WriteString(m.st.heading.Render(title))
	builder.WriteString(strings.Repeat(" ", max(1, m.width-lipgloss.Width(title)-lipgloss.Width(location))))
	builder.WriteString(m.st.muted.Render(location))
	builder.WriteString("\n")
	builder.WriteString(m.st.border.Render(strings.Repeat("─", m.width)))
	builder.WriteString("\n")
	for row := 0; row < bodyHeight-1; row++ {
		left, right := "", ""
		if row < len(sidebar) {
			left = sidebar[row]
		}
		if row < len(content) {
			right = content[row]
		}
		builder.WriteString(fit(left, sidebarWidth))
		builder.WriteString(m.st.border.Render(" │ "))
		builder.WriteString(fit(right, contentWidth))
		builder.WriteString("\n")
	}
	builder.WriteString(strings.Join(status, "\n"))
	return builder.String()
}

func shortHints(hints string, width int) string {
	if lipgloss.Width(hints) <= width {
		return hints
	}
	parts := strings.Split(hints, " · ")
	tail := "… · ? guide · q quit"
	kept := ""
	for _, part := range parts {
		candidate := part
		if kept != "" {
			candidate = kept + " · " + part
		}
		if lipgloss.Width(candidate)+lipgloss.Width(tail)+3 > width {
			break
		}
		kept = candidate
	}
	if kept == "" {
		return tail
	}
	return kept + " · " + tail
}

func (m *Model) statusLines() []string {
	if m.busy != "" {
		return []string{m.st.status.Render(fit(" ◌ "+m.busy, m.width))}
	}
	if m.notice == "" {
		return []string{m.st.status.Render(fit(" "+shortHints(m.hints(), m.width-2), m.width))}
	}
	glyph := "ℹ"
	switch m.kind {
	case noticeSuccess:
		glyph = "✔"
	case noticeWarning:
		glyph = "▲"
	case noticeFailure:
		glyph = "✖"
	}
	wrapped := wrap(m.notice, m.width-4)
	if len(wrapped) > maxNoticeLines {
		wrapped = wrapped[:maxNoticeLines]
		wrapped[maxNoticeLines-1] = fit(wrapped[maxNoticeLines-1]+"…", m.width-4)
	}
	lines := make([]string, 0, len(wrapped))
	for index, line := range wrapped {
		lead := "   "
		if index == 0 {
			lead = " " + glyph + " "
		}
		lines = append(lines, m.st.status.Render(fit(lead+line, m.width)))
	}
	return lines
}

func (m *Model) sidebarView(width, height int) []string {
	lines := make([]string, 0, height)
	bottomStart := -1
	for index, item := range m.nav {
		if item.Bottom && bottomStart < 0 {
			bottomStart = index
		}
	}
	entry := func(index int) string {
		item := m.nav[index]
		label := fmt.Sprintf("%d %s", (index+1)%10, item.Label)
		if m.sidebar {
			label = fmt.Sprintf("%d", (index+1)%10)
		} else if item.ID == app.NavNotifications && m.overview.Counts.UnreadAlerts > 0 {
			label += fmt.Sprintf(" (%d)", m.overview.Counts.UnreadAlerts)
		}
		if index == m.page {
			return m.st.selected.Render(fit("▌"+label, width))
		}
		return fit(" "+label, width)
	}
	for index := 0; index < len(m.nav) && (bottomStart < 0 || index < bottomStart); index++ {
		lines = append(lines, entry(index))
	}
	if bottomStart >= 0 {
		for len(lines) < height-1-(len(m.nav)-bottomStart) {
			lines = append(lines, "")
		}
		for index := bottomStart; index < len(m.nav); index++ {
			lines = append(lines, entry(index))
		}
	}
	return lines
}

func (m *Model) welcomeView(width int) []string {
	lines := []string{m.st.heading.Render("Welcome to " + meta.AppName), ""}
	lines = append(lines, wrap("A new database was created at "+m.app.Location().Path+". Everything stays in this one file on your computer. You can move it later in Settings, Database location.", width)...)
	lines = append(lines, "")
	lines = append(lines, wrap(meta.AppName+" connects only to hosts you add and confirm that you are authorized to test, and only when you start a check or turn on automatic checks. Nothing else is sent anywhere.", width)...)
	lines = append(lines, "", "Press l to load labeled demo data and look around.", "Press any other key to start with an empty database.")
	return lines
}

func (m *Model) confirmView(width int) []string {
	lines := []string{m.st.heading.Render(m.confirm.title), ""}
	lines = append(lines, wrap(m.confirm.statement, width)...)
	lines = append(lines, "", m.st.warning.Render("▲ Press y to "+m.confirm.accept+", or n or Esc to cancel."))
	return lines
}

func (m *Model) previewView(width, height int) []string {
	lines := []string{m.st.heading.Render(m.preview.title), ""}
	var body []string
	for _, line := range m.preview.lines {
		body = append(body, wrap(line, width)...)
	}
	shown, offset := window(body, m.preview.offset, height-6)
	m.preview.offset = offset
	lines = append(lines, shown...)
	lines = append(lines, "")
	if m.preview.action != nil || m.preview.command != nil {
		lines = append(lines, m.st.warning.Render("▲ Press Enter to "+m.preview.accept+", or Esc to cancel. Up and Down scroll."))
	} else {
		lines = append(lines, m.st.muted.Render("Up and Down scroll · Esc close"))
	}
	return lines
}

func (m *Model) menuView(width int) []string {
	lines := []string{m.st.heading.Render("Menu"), ""}
	for index, item := range m.menu {
		line := fit(item.label, 34) + m.st.muted.Render(item.hint)
		if index == m.menuAt {
			line = m.st.selected.Render(fit("▌ "+item.label, 36)) + m.st.muted.Render(item.hint)
		} else {
			line = "  " + line
		}
		lines = append(lines, fit(line, width))
	}
	lines = append(lines, "", m.st.muted.Render("Up and Down move · Enter choose · Esc close"))
	return lines
}

func (m *Model) pageView(width, height int) []string {
	if page := m.pages[m.pageID()]; page != nil {
		return page.render(m, width, height-1)
	}
	switch m.pageID() {
	case app.NavOverview:
		return m.overviewView(width)
	case app.NavNotifications:
		return m.inbox.view(m, width, height-1)
	case app.NavSettings:
		return m.set.view(m, width, height-1)
	case app.NavAbout:
		return m.about.view(m, width, height-1)
	}
	return nil
}
