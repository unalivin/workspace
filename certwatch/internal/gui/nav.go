package gui

import (
	"image/color"

	"fyne.io/fyne/v2"
	"fyne.io/fyne/v2/canvas"
	"fyne.io/fyne/v2/driver/desktop"
	fynetheme "fyne.io/fyne/v2/theme"
	"fyne.io/fyne/v2/widget"

	"github.com/unalivin/certwatch/internal/theme"
)

const (
	sidebarWidth = 224
	markerWidth  = 4
)

type navRow struct {
	widget.BaseWidget
	Text     string
	OnTapped func()
	glyph    string
	count    string
	selected bool
	quiet    bool
	hovered  bool
	focused  bool
	hint     func() string
	board    *dashboard
}

var (
	_ fyne.Tappable     = (*navRow)(nil)
	_ fyne.Focusable    = (*navRow)(nil)
	_ desktop.Hoverable = (*navRow)(nil)
)

func newNavRow(board *dashboard, label, glyph string, hint func() string, tapped func()) *navRow {
	row := &navRow{Text: label, glyph: glyph, hint: hint, board: board, OnTapped: tapped}
	row.ExtendBaseWidget(row)
	return row
}

func (r *navRow) Tapped(*fyne.PointEvent) {
	if r.OnTapped != nil {
		r.OnTapped()
	}
}

func (r *navRow) MouseIn(*desktop.MouseEvent) {
	r.hovered = true
	r.Refresh()
	r.board.showHint(r, r.hint())
}

func (r *navRow) MouseMoved(*desktop.MouseEvent) {}

func (r *navRow) MouseOut() {
	r.hovered = false
	r.Refresh()
	r.board.hideHint()
}

func (r *navRow) FocusGained() {
	r.focused = true
	r.Refresh()
	r.board.showHint(r, r.hint())
}

func (r *navRow) FocusLost() {
	r.focused = false
	r.Refresh()
	r.board.hideHint()
}

func (r *navRow) TypedRune(letter rune) {
	if letter == ' ' {
		r.Tapped(nil)
	}
}

func (r *navRow) TypedKey(event *fyne.KeyEvent) {
	if event.Name == fyne.KeyReturn || event.Name == fyne.KeyEnter || event.Name == fyne.KeySpace {
		r.Tapped(nil)
	}
}

func (r *navRow) setText(label, count string) bool {
	if r.Text == label && r.count == count {
		return false
	}
	r.Text, r.count = label, count
	r.Refresh()
	return true
}

func (r *navRow) setSelected(selected bool) {
	if r.selected != selected {
		r.selected = selected
		r.Refresh()
	}
}

func (r *navRow) CreateRenderer() fyne.WidgetRenderer {
	renderer := &navRowRenderer{row: r, fill: canvas.NewRectangle(color.Transparent), marker: canvas.NewRectangle(color.Transparent), image: canvas.NewImageFromResource(nil), label: canvas.NewText("", nil), count: canvas.NewText("", nil)}
	renderer.image.FillMode = canvas.ImageFillContain
	renderer.count.TextStyle = fyne.TextStyle{Monospace: true}
	renderer.paint()
	return renderer
}

type navRowRenderer struct {
	row    *navRow
	fill   *canvas.Rectangle
	marker *canvas.Rectangle
	image  *canvas.Image
	label  *canvas.Text
	count  *canvas.Text
}

func (r *navRowRenderer) tokens() theme.Tokens { return r.row.board.tokens }

func (r *navRowRenderer) paint() {
	t := r.tokens()
	row := r.row
	r.fill.CornerRadius = t.Radius.Control
	r.fill.StrokeWidth = 0
	r.fill.FillColor = color.Transparent
	switch {
	case row.selected:
		r.fill.FillColor = rgb(t.Palette.SurfaceAlt)
	case row.hovered:
		r.fill.FillColor = fynetheme.Color(fynetheme.ColorNameHover)
	}
	if row.focused {
		r.fill.StrokeColor = rgb(t.Focus)
		r.fill.StrokeWidth = 2
	}
	r.marker.CornerRadius = markerWidth / 2
	r.marker.FillColor = color.Transparent
	if row.selected {
		r.marker.FillColor = rgb(t.Palette.Accent)
	}
	tint := t.Palette.TextMuted
	if row.selected && !row.quiet {
		tint = t.Palette.Text
	}
	r.image.Resource = icon(row.glyph, tint)
	r.label.Text = row.Text
	r.label.TextSize = t.Type.Body
	r.label.TextStyle = fyne.TextStyle{Bold: row.selected}
	r.label.Color = rgb(t.Palette.Text)
	r.count.Text = row.count
	r.count.TextSize = t.Type.Caption
	r.count.Color = rgb(t.Palette.TextMuted)
}

func (r *navRowRenderer) Layout(size fyne.Size) {
	t := r.tokens()
	glyph := iconSize(t)
	r.fill.Resize(size)
	r.fill.Move(fyne.NewPos(0, 0))
	r.marker.Resize(fyne.NewSize(markerWidth, t.Spacing.L))
	r.marker.Move(fyne.NewPos(0, (size.Height-t.Spacing.L)/2))
	r.image.Resize(fyne.NewSize(glyph, glyph))
	if r.row.Text == "" {
		r.image.Move(fyne.NewPos((size.Width-glyph)/2, (size.Height-glyph)/2))
		r.label.Hide()
		r.count.Hide()
		return
	}
	r.image.Move(fyne.NewPos(t.Spacing.M, (size.Height-glyph)/2))
	r.label.Show()
	label := r.label.MinSize()
	r.label.Resize(label)
	r.label.Move(fyne.NewPos(t.Spacing.M+glyph+t.Spacing.S, (size.Height-label.Height)/2))
	if r.row.count == "" {
		r.count.Hide()
		return
	}
	r.count.Show()
	count := r.count.MinSize()
	r.count.Resize(count)
	r.count.Move(fyne.NewPos(size.Width-t.Spacing.M-count.Width, (size.Height-count.Height)/2))
}

func (r *navRowRenderer) MinSize() fyne.Size {
	t := r.tokens()
	glyph := iconSize(t)
	if r.row.Text == "" {
		return fyne.NewSize(t.Spacing.RowHeight+t.Spacing.S, t.Spacing.RowHeight)
	}
	width := t.Spacing.M + glyph + t.Spacing.S + fyne.MeasureText(r.row.Text, t.Type.Body, fyne.TextStyle{Bold: true}).Width + t.Spacing.M
	if r.row.count != "" {
		width += t.Spacing.S + fyne.MeasureText(r.row.count, t.Type.Caption, fyne.TextStyle{Monospace: true}).Width
	}
	return fyne.NewSize(width, t.Spacing.RowHeight)
}

func (r *navRowRenderer) Refresh() {
	r.paint()
	r.Layout(r.row.Size())
	for _, object := range r.Objects() {
		object.Refresh()
	}
}

func (r *navRowRenderer) Objects() []fyne.CanvasObject {
	return []fyne.CanvasObject{r.fill, r.marker, r.image, r.label, r.count}
}

func (r *navRowRenderer) Destroy() {}

type railLayout struct {
	board *dashboard
}

func (l *railLayout) width() float32 {
	t := l.board.tokens
	if l.board.collapse {
		return t.Spacing.RowHeight + t.Spacing.S + 2*t.Spacing.S
	}
	return sidebarWidth * t.Type.Body / 14
}

func (l *railLayout) Layout(objects []fyne.CanvasObject, size fyne.Size) {
	inset := l.board.tokens.Spacing.S
	for _, object := range objects {
		object.Move(fyne.NewPos(inset, inset))
		object.Resize(fyne.NewSize(size.Width-2*inset, size.Height-2*inset))
	}
}

func (l *railLayout) MinSize(objects []fyne.CanvasObject) fyne.Size {
	inset := l.board.tokens.Spacing.S
	height := float32(0)
	for _, object := range objects {
		height = max(height, object.MinSize().Height)
	}
	return fyne.NewSize(l.width(), height+2*inset)
}
