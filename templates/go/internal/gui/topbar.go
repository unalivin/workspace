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

	"github.com/NexzaDev/templateapp/internal/app"
	"github.com/NexzaDev/templateapp/internal/records"
	"github.com/NexzaDev/templateapp/internal/theme"
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
	d.search.SetPlaceHolder("Search records and the usage guide, then press Enter")
	d.search.OnSubmitted = d.searchEverywhere
	d.chip = widget.NewButtonWithIcon("Demo data", icon("info", d.tokens.Semantic.Info), func() {
		d.show(app.NavSettings)
		d.say(theme.SeverityInfo, "Sample records are loaded. Remove demo data below deletes only the records whose names start with \"Demo:\".")
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
	unread := d.overview.UnreadNotifications
	switch unread {
	case 0:
		return "Notifications: nothing unread"
	case 1:
		return "Notifications: 1 unread"
	}
	return "Notifications: " + strconv.Itoa(unread) + " unread"
}

func (d *dashboard) refreshTopBar() {
	if d.bell == nil {
		return
	}
	label := ""
	if d.overview.UnreadNotifications > 0 {
		label = strconv.Itoa(d.overview.UnreadNotifications) + " unread"
	}
	if d.bell.Text != label {
		d.bell.SetText(label)
	}
	if d.overview.Demo > 0 {
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
	found, err := d.app.Records.List(d.ctx, records.Query{Search: query, IncludeDisabled: true})
	if d.fail(err) {
		return
	}
	sections := d.aboutP.usage.Search(query)
	switch {
	case len(found) > 0:
		d.show(app.NavRecords)
		d.recordsP.disabled.SetChecked(true)
		d.recordsP.search.SetText(query)
		d.say(theme.SeverityInfo, "%d records and %d usage guide sections match \"%s\". The guide is under About, Usage guide.", len(found), len(sections), query)
	case len(sections) > 0:
		d.showAbout(aboutGuide)
		d.aboutP.search.SetText(query)
		d.say(theme.SeverityInfo, "No record matches \"%s\". %d usage guide sections do.", query, len(sections))
	default:
		d.say(theme.SeverityInfo, "Nothing matches \"%s\". Search covers record names, record notes and the usage guide.", query)
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
