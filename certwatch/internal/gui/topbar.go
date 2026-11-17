package gui

import (
	"image/color"
	"strconv"
	"strings"
	"time"

	"fyne.io/fyne/v2"
	"fyne.io/fyne/v2/canvas"
	"fyne.io/fyne/v2/container"
	"fyne.io/fyne/v2/driver/desktop"
	"fyne.io/fyne/v2/layout"
	fynetheme "fyne.io/fyne/v2/theme"
	"fyne.io/fyne/v2/widget"

	"github.com/unalivin/certwatch/internal/app"
	"github.com/unalivin/certwatch/internal/theme"
	"github.com/unalivin/certwatch/internal/words"
)

const noticeFade = 180 * time.Millisecond

type hintButton struct {
	widget.Button
	hint  func() string
	board *dashboard
}

var _ desktop.Hoverable = (*hintButton)(nil)

func newHintButton(board *dashboard, label string, resource fyne.Resource, hint func() string, tapped func()) *hintButton {
	button := &hintButton{hint: hint, board: board}
	button.Text = label
	button.Icon = resource
	button.OnTapped = tapped
	button.ExtendBaseWidget(button)
	return button
}

func (b *hintButton) MouseIn(event *desktop.MouseEvent) {
	b.Button.MouseIn(event)
	b.board.showHint(b, b.hint())
}

func (b *hintButton) MouseOut() {
	b.Button.MouseOut()
	b.board.hideHint()
}

func (b *hintButton) FocusGained() {
	b.Button.FocusGained()
	b.board.showHint(b, b.hint())
}

func (b *hintButton) FocusLost() {
	b.Button.FocusLost()
	b.board.hideHint()
}

func (d *dashboard) buildHints() *fyne.Container {
	d.hintText = canvas.NewText("", nil)
	d.hintFill = canvas.NewRectangle(nil)
	d.hintBox = container.NewStack(d.hintFill, container.NewPadded(d.hintText))
	d.hintBox.Hide()
	return container.NewWithoutLayout(d.hintBox)
}

func (d *dashboard) showHint(anchor fyne.CanvasObject, text string) {
	if d.hintBox == nil || text == "" {
		return
	}
	d.hintFill.FillColor = rgb(d.tokens.Palette.Text)
	d.hintFill.CornerRadius = fynetheme.Size(fynetheme.SizeNameInputRadius)
	d.hintText.Color = rgb(d.tokens.Palette.Background)
	d.hintText.TextSize = fynetheme.CaptionTextSize()
	d.hintText.Text = text
	d.hintLabel = text
	origin := fyne.CurrentApp().Driver().AbsolutePositionForObject(anchor)
	size := d.hintBox.MinSize()
	d.hintBox.Resize(size)
	d.hintBox.Move(fyne.NewPos(origin.X+anchor.Size().Width+fynetheme.Padding(), origin.Y+(anchor.Size().Height-size.Height)/2))
	d.hintBox.Show()
	d.hintBox.Refresh()
}

func (d *dashboard) hideHint() {
	if d.hintBox == nil {
		return
	}
	d.hintLabel = ""
	d.hintBox.Hide()
}

func (d *dashboard) buildTopBar() fyne.CanvasObject {
	d.search = widget.NewEntry()
	d.search.SetPlaceHolder("Search endpoints, certificates and the usage guide")
	d.search.OnSubmitted = d.searchEverywhere
	d.chip = widget.NewButtonWithIcon("Demo data", icon("info", d.tokens.Semantic.Info), func() {
		d.show(app.NavSettings)
		d.say(theme.SeverityInfo, "Demo endpoints and certificates are loaded. They are never contacted. Remove demo data below deletes only them.")
	})
	d.chip.Importance = widget.LowImportance
	d.chip.Hide()
	d.bell = newHintButton(d, "", icon("bell", d.tokens.Palette.Text), d.bellHint, func() { d.show(app.NavNotifications) })
	d.bell.Importance = widget.LowImportance
	d.help = newHintButton(d, "Help", icon("info", d.tokens.Palette.Text), func() string { return "Usage guide (F1)" }, func() { d.showAbout(aboutGuide) })
	d.help.Importance = widget.LowImportance
	return container.NewBorder(nil, nil, d.title, container.NewHBox(d.chip, d.bell, d.help), container.NewVBox(layout.NewSpacer(), d.search, layout.NewSpacer()))
}

func (d *dashboard) bellHint() string {
	unread := d.overview.Counts.UnreadAlerts
	switch unread {
	case 0:
		return "Alerts: nothing unread"
	case 1:
		return "Alerts: 1 unread"
	}
	return "Alerts: " + strconv.Itoa(unread) + " unread"
}

func (d *dashboard) refreshTopBar() {
	if d.bell == nil {
		return
	}
	label := ""
	if d.overview.Counts.UnreadAlerts > 0 {
		label = strconv.Itoa(d.overview.Counts.UnreadAlerts) + " unread"
	}
	if d.bell.Text != label {
		d.bell.SetText(label)
	}
	if d.overview.Demo {
		d.chip.Show()
	} else {
		d.chip.Hide()
	}
	d.bell.SetIcon(icon("bell", d.tokens.Palette.Text))
	d.help.SetIcon(icon("info", d.tokens.Palette.Text))
	d.chip.SetIcon(icon("info", d.tokens.Semantic.Info))
	if d.header != nil {
		d.header.Refresh()
	}
}

func (d *dashboard) searchEverywhere(text string) {
	query := strings.TrimSpace(text)
	if query == "" {
		d.say(theme.SeverityInfo, "Type one or more words, then press Enter.")
		return
	}
	endpoints, err := d.app.Table(d.ctx, app.ViewEndpoints, app.ViewQuery{Search: query, Filter: "all"})
	if d.fail(err) {
		return
	}
	certificates, err := d.app.Table(d.ctx, app.ViewCertificates, app.ViewQuery{Search: query, Filter: "all"})
	if d.fail(err) {
		return
	}
	sections := d.aboutP.usage.Search(query)
	open := func(view string) {
		d.openList(view, "all")
		d.views[view].search.SetText(query)
		d.say(theme.SeverityInfo, "%s, %s and %s match \"%s\". The guide is under About, Usage guide.", words.Count(len(endpoints.Rows), "endpoint", "endpoints"), words.Count(len(certificates.Rows), "certificate", "certificates"), words.Count(len(sections), "usage guide section", "usage guide sections"), query)
	}
	switch {
	case len(endpoints.Rows) > 0:
		open(app.ViewEndpoints)
	case len(certificates.Rows) > 0:
		open(app.ViewCertificates)
	case len(sections) > 0:
		d.showAbout(aboutGuide)
		d.aboutP.search.SetText(query)
		d.say(theme.SeverityInfo, "No endpoint or certificate matches \"%s\". %d usage guide sections do.", query, len(sections))
	default:
		d.say(theme.SeverityInfo, "Nothing matches \"%s\". Search covers endpoints, certificates and the usage guide.", query)
	}
}

func (d *dashboard) flashNotice() {
	if d.noticeFill == nil {
		return
	}
	rest := color.NRGBA{}
	if !d.tokens.Motion {
		d.noticeFill.FillColor = rest
		d.noticeFill.Refresh()
		return
	}
	start := alpha(d.tokens.Palette.Accent, 0x50)
	d.fades++
	canvas.NewColorRGBAAnimation(start, rest, noticeFade, func(now color.Color) {
		d.noticeFill.FillColor = now
		d.noticeFill.Refresh()
	}).Start()
}
