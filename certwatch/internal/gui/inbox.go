package gui

import (
	"errors"
	"fmt"
	"strconv"
	"strings"

	"fyne.io/fyne/v2"
	"fyne.io/fyne/v2/container"
	"fyne.io/fyne/v2/widget"

	"github.com/unalivin/certwatch/internal/app"
	"github.com/unalivin/certwatch/internal/inventory"
	"github.com/unalivin/certwatch/internal/notify"
	"github.com/unalivin/certwatch/internal/theme"
	"github.com/unalivin/certwatch/internal/words"
)

func severityChoices() ([]string, []string) {
	var options, labels []string
	for _, level := range theme.Severities {
		options = append(options, string(level))
		labels = append(labels, level.Label())
	}
	return options, labels
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
	layout := &columns{weights: []float32{0, 0, 3, 0, 0}, fixed: []float32{140, 120, 160, 70, 120}}
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
	openRecord := widget.NewButtonWithIcon("Open source", glyph("arrow-right"), p.openRecord)
	inbox := container.NewBorder(
		container.NewVBox(container.NewHBox(p.unreadOnly, markRead, markAll, openRecord), headerRow(layout, "When", "Severity", "Alert", "State", "About")),
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
	p.summary.SetText(fmt.Sprintf("%d shown, %d unread", len(items), p.d.overview.Counts.UnreadAlerts))
	switch {
	case len(items) > 0:
		p.empty.Hide()
	case p.unreadOnly.Checked:
		p.empty.SetText("No unread alerts. Clear Unread only to see all of them.")
		p.empty.Show()
	default:
		p.empty.SetText("No alerts yet. They appear when a certificate nears its expiry, when validation fails or when an endpoint presents a different certificate.")
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
	p.d.say(theme.SeverityInfo, "Select an alert first")
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
	p.d.done("Marked the alert as read")
}

func (p *inboxPage) markAll() {
	changed, err := p.d.app.Notices.MarkAllRead(p.d.ctx)
	if p.d.fail(err) {
		return
	}
	p.d.done("Marked %s as read", words.Count(int(changed), "alert", "alerts"))
}

func (p *inboxPage) openRecord() {
	item, found := p.chosen()
	if !found {
		return
	}
	view := app.ViewEndpoints
	if item.RecordKind == inventory.KindCertificate {
		view = app.ViewCertificates
	}
	if item.RecordID == 0 {
		p.d.say(theme.SeverityInfo, "The certificate or endpoint of this alert was removed")
		return
	}
	if item.Unread() {
		if p.d.fail(p.d.app.Notices.MarkRead(p.d.ctx, item.ID)) {
			return
		}
	}
	p.d.openRow(view, item.RecordID)
	if p.d.views[view].selected != item.RecordID {
		p.d.say(theme.SeverityInfo, "The certificate or endpoint of this alert was removed")
	}
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
		{key: "enabled", label: "Enabled", value: strconv.FormatBool(rule.Enabled), options: []string{"true", "false"}, labels: []string{"Yes", "No"}, hint: "Off stops alerts of this event type."},
		{key: "severity", label: "Minimum severity", value: string(rule.MinSeverity), options: severities, labels: labels, hint: "Events below this severity raise no alert."},
		{key: "start", label: "Quiet hours start", value: rule.QuietStart, hint: "Time of day as HH:MM. Leave both times empty for no quiet hours."},
		{key: "end", label: "Quiet hours end", value: rule.QuietEnd, hint: "Time of day as HH:MM. May be earlier than the start to span midnight."},
		{key: "dedup", label: "De-duplication", value: strconv.Itoa(rule.DedupMinutes), hint: "Minutes in which the same event for the same certificate or endpoint is not raised again. 0 turns it off."},
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
