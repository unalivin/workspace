package gui

import (
	"fyne.io/fyne/v2"
	"fyne.io/fyne/v2/container"
	fynetheme "fyne.io/fyne/v2/theme"
)

type shareLayout struct {
	shares []float32
	gap    func() float32
}

func (l *shareLayout) Layout(objects []fyne.CanvasObject, size fyne.Size) {
	gap := l.gap()
	total := float32(0)
	for index := range objects {
		total += l.shares[index%len(l.shares)]
	}
	free := size.Width - gap*float32(max(0, len(objects)-1))
	x := float32(0)
	for index, object := range objects {
		width := free * l.shares[index%len(l.shares)] / total
		object.Move(fyne.NewPos(x, 0))
		object.Resize(fyne.NewSize(width, size.Height))
		x += width + gap
	}
}

func (l *shareLayout) MinSize(objects []fyne.CanvasObject) fyne.Size {
	width, height := float32(0), float32(0)
	for _, object := range objects {
		minimum := object.MinSize()
		width += minimum.Width
		height = max(height, minimum.Height)
	}
	return fyne.NewSize(width+l.gap()*float32(max(0, len(objects)-1)), height)
}

type pileLayout struct {
	gap func() float32
}

func (l *pileLayout) Layout(objects []fyne.CanvasObject, size fyne.Size) {
	y := float32(0)
	for _, object := range objects {
		if !object.Visible() {
			continue
		}
		height := object.MinSize().Height
		object.Move(fyne.NewPos(0, y))
		object.Resize(fyne.NewSize(size.Width, height))
		y += height + l.gap()
	}
}

func (l *pileLayout) MinSize(objects []fyne.CanvasObject) fyne.Size {
	width, height, shown := float32(0), float32(0), 0
	for _, object := range objects {
		if !object.Visible() {
			continue
		}
		minimum := object.MinSize()
		width = max(width, minimum.Width)
		height += minimum.Height
		shown++
	}
	return fyne.NewSize(width, height+l.gap()*float32(max(0, shown-1)))
}

type marginLayout struct {
	side func() float32
	edge func() float32
}

func (l *marginLayout) Layout(objects []fyne.CanvasObject, size fyne.Size) {
	for _, object := range objects {
		object.Move(fyne.NewPos(l.side(), l.edge()))
		object.Resize(fyne.NewSize(size.Width-2*l.side(), size.Height-2*l.edge()))
	}
}

func (l *marginLayout) MinSize(objects []fyne.CanvasObject) fyne.Size {
	width, height := float32(0), float32(0)
	for _, object := range objects {
		minimum := object.MinSize()
		width, height = max(width, minimum.Width), max(height, minimum.Height)
	}
	return fyne.NewSize(width+2*l.side(), height+2*l.edge())
}

type gutterLayout struct{}

func (gutterLayout) width() float32 {
	return fynetheme.Size(fynetheme.SizeNameScrollBar) + fynetheme.Padding()
}

func (l gutterLayout) Layout(objects []fyne.CanvasObject, size fyne.Size) {
	for _, object := range objects {
		object.Move(fyne.NewPos(0, 0))
		object.Resize(fyne.NewSize(size.Width-l.width(), size.Height))
	}
}

func (l gutterLayout) MinSize(objects []fyne.CanvasObject) fyne.Size {
	width, height := float32(0), float32(0)
	for _, object := range objects {
		minimum := object.MinSize()
		width, height = max(width, minimum.Width), max(height, minimum.Height)
	}
	return fyne.NewSize(width+l.width(), height)
}

func scrolled(content fyne.CanvasObject) *container.Scroll {
	return container.NewVScroll(container.New(gutterLayout{}, content))
}

var ledgerUnits = []int{1, 2, 5, 10, 20, 50, 100, 200, 500, 1000, 2000, 5000, 10000}

func ledgerUnit(largest, capacity int) int {
	if capacity < 1 {
		capacity = 1
	}
	for _, unit := range ledgerUnits {
		if (largest+unit-1)/unit <= capacity {
			return unit
		}
	}
	return (largest + capacity - 1) / capacity
}
