package tui

import (
	"errors"
	"fmt"
	"strconv"
	"strings"

	"github.com/unalivin/certwatch/internal/app"
	"github.com/unalivin/certwatch/internal/inventory"
	"github.com/unalivin/certwatch/internal/notify"
	"github.com/unalivin/certwatch/internal/theme"
	"github.com/unalivin/certwatch/internal/words"
)

const listLimit = 500

func severityOptions() ([]string, []string) {
	var options, labels []string
	for _, level := range theme.Severities {
		options = append(options, string(level))
		labels = append(labels, level.Glyph()+" "+level.Label())
	}
	return options, labels
}

func next(options []string, current string) string {
	for index, option := range options {
		if option == current {
			return options[(index+1)%len(options)]
		}
	}
	return options[0]
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
		m.changed("Marked %s as read", words.Count(int(changed), "alert", "alerts"))
	case "m", "enter":
		if p.cursor >= len(p.list) {
			return
		}
		item := p.list[p.cursor]
		if item.Unread() {
			if m.fail(m.app.Notices.MarkRead(m.ctx, item.ID)) {
				return
			}
			m.changed("Marked alert %d as read", item.ID)
		}
		if key != "enter" {
			return
		}
		target := app.ViewEndpoints
		if item.RecordKind == inventory.KindCertificate {
			target = app.ViewCertificates
		}
		page := m.pages[target]
		page.query = app.ViewQuery{Filter: "all"}
		page.load(m)
		if item.RecordID == 0 || !page.selectID(item.RecordID) {
			m.say(noticeInfo, "The endpoint or certificate of this alert is no longer stored")
			return
		}
		m.open(target)
		page.openDetail(m)
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
			choiceField("enabled", "Enabled", enabled, "Off stops alerts of this event type.", []string{"true", "false"}, []string{"Yes", "No"}),
			choiceField("severity", "Minimum severity", string(rule.MinSeverity), "Events below this severity raise no alert.", severities, labels),
			textField("start", "Quiet hours start", rule.QuietStart, "Time of day as HH:MM. Leave both times empty for no quiet hours.", 5),
			textField("end", "Quiet hours end", rule.QuietEnd, "Time of day as HH:MM. May be earlier than the start to span midnight.", 5),
			textField("dedup", "De-duplication", strconv.Itoa(rule.DedupMinutes), "Minutes in which the same event for the same endpoint or certificate is not raised again. 0 turns it off.", 5),
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
	filter := "all alerts"
	if p.unreadOnly {
		filter = "unread only"
	}
	lines = append(lines, m.st.muted.Render(fmt.Sprintf("Showing %s · %d unread", filter, m.overview.Counts.UnreadAlerts)), "")
	if len(p.list) == 0 {
		message := "No alerts yet. They appear when a certificate nears its expiry, when validation fails or when an endpoint presents a different certificate."
		if p.unreadOnly {
			message = "No unread alerts. Press u to show all of them."
		}
		return append(lines, wrap(message, width)...)
	}
	widths := layoutColumns([]column{{"", "", 4, false}, {"", "", 16, false}, {"", "", 12, false}, {"", "", 6, false}, {"", "", 20, true}, {"", "", 10, false}}, width-2)
	lines = append(lines, m.st.muted.Render("  "+renderRow([]string{"ID", "WHEN", "SEVERITY", "STATE", "ALERT", "ABOUT"}, widths)))
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
