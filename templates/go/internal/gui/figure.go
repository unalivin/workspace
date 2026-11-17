package gui

import (
	"image/color"

	"fyne.io/fyne/v2"
	"fyne.io/fyne/v2/canvas"
	"fyne.io/fyne/v2/widget"
)

type tone int

const (
	toneText tone = iota
	toneMuted
	toneAccent
)

type scaleStep int

const (
	stepCaption scaleStep = iota
	stepBody
	stepSubheading
	stepFigure
)

type inked struct {
	widget.BaseWidget
	Text  string
	step  scaleStep
	tone  tone
	look  fyne.TextStyle
	board *dashboard
}

func newInked(board *dashboard, text string, step scaleStep, tint tone, look fyne.TextStyle) *inked {
	label := &inked{Text: text, step: step, tone: tint, look: look, board: board}
	label.ExtendBaseWidget(label)
	return label
}

func (i *inked) SetText(text string) {
	if i.Text != text {
		i.Text = text
		i.Refresh()
	}
}

func (i *inked) size() float32 {
	scale := i.board.tokens.Type
	switch i.step {
	case stepCaption:
		return scale.Caption
	case stepSubheading:
		return scale.Subheading
	case stepFigure:
		return scale.Figure
	}
	return scale.Body
}

func (i *inked) ink() color.Color {
	palette := i.board.tokens.Palette
	switch i.tone {
	case toneMuted:
		return rgb(palette.TextMuted)
	case toneAccent:
		return rgb(palette.Accent)
	}
	return rgb(palette.Text)
}

func (i *inked) CreateRenderer() fyne.WidgetRenderer {
	renderer := &inkedRenderer{owner: i, text: canvas.NewText(i.Text, nil)}
	renderer.paint()
	return renderer
}

type inkedRenderer struct {
	owner *inked
	text  *canvas.Text
}

func (r *inkedRenderer) paint() {
	r.text.Text = r.owner.Text
	r.text.TextSize = r.owner.size()
	r.text.TextStyle = r.owner.look
	r.text.Color = r.owner.ink()
}

func (r *inkedRenderer) Layout(size fyne.Size) {
	r.text.Move(fyne.NewPos(0, 0))
	r.text.Resize(size)
}

func (r *inkedRenderer) MinSize() fyne.Size {
	r.paint()
	return r.text.MinSize()
}

func (r *inkedRenderer) Refresh() {
	r.paint()
	r.text.Refresh()
}

func (r *inkedRenderer) Objects() []fyne.CanvasObject { return []fyne.CanvasObject{r.text} }

func (r *inkedRenderer) Destroy() {}
