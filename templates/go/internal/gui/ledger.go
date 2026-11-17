package gui

import (
	"strconv"

	"fyne.io/fyne/v2"
	"fyne.io/fyne/v2/canvas"
	"fyne.io/fyne/v2/widget"

	"github.com/unalivin/templateapp/internal/records"
	"github.com/unalivin/templateapp/internal/theme"
)

func ledgerCaption(unit int) string {
	if unit <= 1 {
		return "One square is one enabled record."
	}
	return "One square is " + strconv.Itoa(unit) + " enabled records, rounded up."
}

type ledger struct {
	widget.BaseWidget
	board   *dashboard
	rows    []records.Breakdown
	unit    int
	changed func()
}

func newLedger(board *dashboard) *ledger {
	chart := &ledger{board: board, unit: 1}
	chart.ExtendBaseWidget(chart)
	return chart
}

func (l *ledger) setRows(rows []records.Breakdown) {
	l.rows = rows
	l.Refresh()
}

func (l *ledger) count(status string, level theme.Severity) int {
	total := 0
	for _, row := range l.rows {
		if row.Status == status && row.Severity == level {
			total += row.Total
		}
	}
	return total
}

func (l *ledger) largest() int {
	largest := 0
	for _, status := range records.Statuses {
		for _, level := range theme.Severities {
			largest = max(largest, l.count(status, level))
		}
	}
	return largest
}

func (l *ledger) CreateRenderer() fyne.WidgetRenderer {
	return &ledgerRenderer{chart: l}
}

type ledgerRenderer struct {
	chart   *ledger
	objects []fyne.CanvasObject
}

func (r *ledgerRenderer) labelWidth() float32 {
	return 8 * r.chart.board.tokens.Type.Body
}

func (r *ledgerRenderer) MinSize() fyne.Size {
	t := r.chart.board.tokens
	rows := float32(len(theme.Severities) + 1)
	return fyne.NewSize(r.labelWidth()+3*5*t.Spacing.L, rows*t.Spacing.RowHeight+(rows-1)*t.Spacing.RowGap)
}

func (r *ledgerRenderer) text(content string, size float32, tint theme.Color, look fyne.TextStyle, x, top, height float32) float32 {
	text := canvas.NewText(content, rgb(tint))
	text.TextSize = size
	text.TextStyle = look
	minimum := text.MinSize()
	text.Resize(minimum)
	text.Move(fyne.NewPos(x, top+(height-minimum.Height)/2))
	r.objects = append(r.objects, text)
	return minimum.Width
}

func (r *ledgerRenderer) glyph(name string, tint theme.Color, x, top, height float32) float32 {
	side := iconSize(r.chart.board.tokens)
	image := canvas.NewImageFromResource(icon(name, tint))
	image.FillMode = canvas.ImageFillContain
	image.Resize(fyne.NewSize(side, side))
	image.Move(fyne.NewPos(x, top+(height-side)/2))
	r.objects = append(r.objects, image)
	return side
}

func (r *ledgerRenderer) Layout(size fyne.Size) {
	t := r.chart.board.tokens
	r.objects = r.objects[:0]
	if size.Width <= 0 {
		return
	}
	label := r.labelWidth()
	column := (size.Width - label) / float32(len(records.Statuses))
	step := t.Spacing.RowHeight + t.Spacing.RowGap
	square := t.Spacing.M
	pitch := square + t.Spacing.XS
	figure := fyne.MeasureText("0000", t.Type.Caption, fyne.TextStyle{Monospace: true}).Width
	capacity := int((column - t.Spacing.L - figure - t.Spacing.S) / pitch)
	if unit := ledgerUnit(r.chart.largest(), capacity); unit != r.chart.unit {
		r.chart.unit = unit
		if r.chart.changed != nil {
			r.chart.changed()
		}
	}
	for index, status := range records.Statuses {
		x := label + float32(index)*column
		x += r.glyph(statusIcon(status), t.Palette.TextMuted, x, 0, t.Spacing.RowHeight) + t.Spacing.S
		x += r.text(records.StatusLabel(status), t.Type.Body, t.Palette.Text, fyne.TextStyle{Bold: true}, x, 0, t.Spacing.RowHeight) + t.Spacing.S
		total := 0
		for _, level := range theme.Severities {
			total += r.chart.count(status, level)
		}
		r.text(strconv.Itoa(total), t.Type.Caption, t.Palette.TextMuted, fyne.TextStyle{Monospace: true}, x, 0, t.Spacing.RowHeight)
	}
	for line := range theme.Severities {
		level := theme.Severities[len(theme.Severities)-1-line]
		top := float32(line+1) * step
		rule := canvas.NewRectangle(rgb(t.Palette.Divider))
		rule.Resize(fyne.NewSize(size.Width, 1))
		rule.Move(fyne.NewPos(0, top-t.Spacing.RowGap/2))
		r.objects = append(r.objects, rule)
		tint := t.Semantic.Severity(level)
		x := r.glyph(level.Icon(), tint, 0, top, t.Spacing.RowHeight) + t.Spacing.S
		r.text(level.Label(), t.Type.Body, t.Palette.Text, fyne.TextStyle{}, x, top, t.Spacing.RowHeight)
		for index, status := range records.Statuses {
			count := r.chart.count(status, level)
			if count == 0 {
				continue
			}
			fill := tint
			if status == records.StatusDone {
				fill = theme.Mix(tint, t.Palette.Surface, 0.6)
			}
			left := label + float32(index)*column
			squares := (count + r.chart.unit - 1) / r.chart.unit
			for position := range squares {
				mark := canvas.NewRectangle(rgb(fill))
				mark.CornerRadius = min(t.Radius.Chip, 3)
				mark.Resize(fyne.NewSize(square, square))
				mark.Move(fyne.NewPos(left+float32(position)*pitch, top+(t.Spacing.RowHeight-square)/2))
				r.objects = append(r.objects, mark)
			}
			r.text(strconv.Itoa(count), t.Type.Caption, t.Palette.TextMuted, fyne.TextStyle{Monospace: true}, left+float32(squares)*pitch+t.Spacing.XS, top, t.Spacing.RowHeight)
		}
	}
}

func (r *ledgerRenderer) Refresh() {
	r.Layout(r.chart.Size())
	canvas.Refresh(r.chart)
}

func (r *ledgerRenderer) Objects() []fyne.CanvasObject { return r.objects }

func (r *ledgerRenderer) Destroy() {}
