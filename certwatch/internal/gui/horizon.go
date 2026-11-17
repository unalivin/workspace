package gui

import (
	"strconv"

	"fyne.io/fyne/v2"
	"fyne.io/fyne/v2/canvas"
	"fyne.io/fyne/v2/widget"

	"github.com/unalivin/certwatch/internal/inventory"
	"github.com/unalivin/certwatch/internal/theme"
)

const horizonNote = " A longer period includes the shorter ones, each in its own color."

func horizonCaption(unit int) string {
	if unit <= 1 {
		return "One square is one tracked certificate." + horizonNote
	}
	return "One square is " + strconv.Itoa(unit) + " tracked certificates, rounded up." + horizonNote
}

type tallyPart struct {
	count int
	level theme.Severity
}

func horizonParts(id string, counts inventory.Overview) []tallyPart {
	week := tallyPart{counts.Within7, theme.SeverityHigh}
	month := tallyPart{counts.Within30 - counts.Within7, theme.SeverityMedium}
	quarter := tallyPart{counts.Within90 - counts.Within30, theme.SeverityLow}
	switch id {
	case "expired":
		return []tallyPart{{counts.Expired, theme.SeverityCritical}}
	case "within7":
		return []tallyPart{week}
	case "within30":
		return []tallyPart{week, month}
	case "within90":
		return []tallyPart{week, month, quarter}
	}
	return nil
}

type tallyScale struct {
	largest int
	unit    int
	changed func()
}

type tally struct {
	widget.BaseWidget
	board *dashboard
	scale *tallyScale
	parts []tallyPart
}

func newTally(board *dashboard, scale *tallyScale) *tally {
	row := &tally{board: board, scale: scale}
	row.ExtendBaseWidget(row)
	return row
}

func (t *tally) setParts(parts []tallyPart) {
	t.parts = parts
	t.Refresh()
}

func (t *tally) CreateRenderer() fyne.WidgetRenderer {
	return &tallyRenderer{row: t}
}

type tallyRenderer struct {
	row     *tally
	objects []fyne.CanvasObject
}

func (r *tallyRenderer) pitch() (float32, float32) {
	spacing := r.row.board.tokens.Spacing
	return spacing.M, spacing.M + spacing.XS
}

func (r *tallyRenderer) MinSize() fyne.Size {
	square, pitch := r.pitch()
	return fyne.NewSize(6*pitch, square)
}

func (r *tallyRenderer) Layout(size fyne.Size) {
	r.objects = r.objects[:0]
	if size.Width <= 0 {
		return
	}
	t := r.row.board.tokens
	square, pitch := r.pitch()
	scale := r.row.scale
	if unit := ledgerUnit(scale.largest, int(size.Width/pitch)-2); unit != scale.unit {
		scale.unit = unit
		if scale.changed != nil {
			scale.changed()
		}
	}
	position := 0
	for _, part := range r.row.parts {
		if part.count <= 0 {
			continue
		}
		for range (part.count + scale.unit - 1) / scale.unit {
			mark := canvas.NewRectangle(rgb(t.Semantic.Severity(part.level)))
			mark.CornerRadius = min(t.Radius.Chip, 3)
			mark.Resize(fyne.NewSize(square, square))
			mark.Move(fyne.NewPos(float32(position)*pitch, (size.Height-square)/2))
			r.objects = append(r.objects, mark)
			position++
		}
	}
}

func (r *tallyRenderer) Refresh() {
	r.Layout(r.row.Size())
	canvas.Refresh(r.row)
}

func (r *tallyRenderer) Objects() []fyne.CanvasObject { return r.objects }

func (r *tallyRenderer) Destroy() {}
