package gui

import (
	"strings"

	"fyne.io/fyne/v2"
	"fyne.io/fyne/v2/canvas"
	"fyne.io/fyne/v2/container"
	"fyne.io/fyne/v2/dialog"
	"fyne.io/fyne/v2/layout"
	fynetheme "fyne.io/fyne/v2/theme"
	"fyne.io/fyne/v2/widget"
)

type columns struct {
	weights []float32
	fixed   []float32
	hidden  []bool
}

func (c *columns) shows(index int) bool {
	return index >= len(c.hidden) || !c.hidden[index]
}

func (c *columns) visible() int {
	count := 0
	for index := range c.weights {
		if c.shows(index) {
			count++
		}
	}
	return count
}

func (c *columns) widths(total float32) []float32 {
	gap := fynetheme.Padding()
	available := total - gap*float32(max(0, c.visible()-1))
	var fixed, weight float32
	for index := range c.weights {
		if c.shows(index) {
			fixed += c.fixed[index]
			weight += c.weights[index]
		}
	}
	spare := available - fixed
	if spare < 0 {
		spare = 0
	}
	widths := make([]float32, len(c.weights))
	for index := range c.weights {
		if !c.shows(index) {
			continue
		}
		widths[index] = c.fixed[index]
		if weight > 0 {
			widths[index] += spare * c.weights[index] / weight
		}
	}
	return widths
}

func (c *columns) Layout(objects []fyne.CanvasObject, size fyne.Size) {
	widths := c.widths(size.Width)
	x := float32(0)
	for index, object := range objects {
		if index >= len(widths) {
			break
		}
		if !c.shows(index) {
			if object.Visible() {
				object.Hide()
			}
			continue
		}
		if !object.Visible() {
			object.Show()
		}
		object.Move(fyne.NewPos(x, 0))
		object.Resize(fyne.NewSize(widths[index], size.Height))
		x += widths[index] + fynetheme.Padding()
	}
}

func (c *columns) MinSize(objects []fyne.CanvasObject) fyne.Size {
	var width, height float32
	for index := range c.weights {
		if c.shows(index) {
			width += c.fixed[index]
		}
	}
	for _, object := range objects {
		if minimum := object.MinSize().Height; minimum > height {
			height = minimum
		}
	}
	return fyne.NewSize(width+fynetheme.Padding()*float32(max(0, c.visible()-1)), height)
}

func cell(text string) *widget.Label {
	label := widget.NewLabel(text)
	label.Truncation = fyne.TextTruncateEllipsis
	return label
}

func headerRow(layout *columns, titles ...string) fyne.CanvasObject {
	row := container.New(layout)
	for _, title := range titles {
		label := cell(title)
		label.TextStyle = fyne.TextStyle{Bold: true}
		label.Importance = widget.LowImportance
		row.Add(label)
	}
	return row
}

func iconLabel(resource fyne.Resource, text string) (*fyne.Container, *widget.Icon, *widget.Label) {
	image := widget.NewIcon(resource)
	label := cell(text)
	return container.NewBorder(nil, nil, image, nil, label), image, label
}

func heading(text string) *widget.Label {
	label := widget.NewLabel(text)
	label.TextStyle = fyne.TextStyle{Bold: true}
	label.SizeName = fynetheme.SizeNameSubHeadingText
	return label
}

func muted(text string) *widget.Label {
	label := widget.NewLabel(text)
	label.Importance = widget.LowImportance
	label.Wrapping = fyne.TextWrapWord
	return label
}

func wrapped(text string) *widget.Label {
	label := widget.NewLabel(text)
	label.Wrapping = fyne.TextWrapWord
	return label
}

type surface struct {
	widget.BaseWidget
	content fyne.CanvasObject
}

type surfaceRenderer struct {
	surface    *surface
	background *canvas.Rectangle
}

func (r *surfaceRenderer) paint() {
	r.background.FillColor = fynetheme.Color(fynetheme.ColorNameInputBackground)
	r.background.StrokeColor = fynetheme.Color(fynetheme.ColorNameSeparator)
	r.background.StrokeWidth = 1
	r.background.CornerRadius = fynetheme.Size(fynetheme.SizeNameInputRadius)
}

func (r *surfaceRenderer) inset() float32 { return fynetheme.InnerPadding() }

func (r *surfaceRenderer) Layout(size fyne.Size) {
	r.background.Resize(size)
	inset := r.inset()
	r.surface.content.Move(fyne.NewPos(inset, inset))
	r.surface.content.Resize(fyne.NewSize(size.Width-2*inset, size.Height-2*inset))
}

func (r *surfaceRenderer) MinSize() fyne.Size {
	inset := r.inset()
	return r.surface.content.MinSize().Add(fyne.NewSize(2*inset, 2*inset))
}

func (r *surfaceRenderer) Refresh() {
	r.paint()
	r.background.Refresh()
	r.surface.content.Refresh()
}

func (r *surfaceRenderer) Objects() []fyne.CanvasObject {
	return []fyne.CanvasObject{r.background, r.surface.content}
}

func (r *surfaceRenderer) Destroy() {}

func (s *surface) CreateRenderer() fyne.WidgetRenderer {
	renderer := &surfaceRenderer{surface: s, background: canvas.NewRectangle(nil)}
	renderer.paint()
	return renderer
}

func panel(content fyne.CanvasObject) fyne.CanvasObject {
	s := &surface{content: content}
	s.ExtendBaseWidget(s)
	return s
}

type formField struct {
	key       string
	label     string
	hint      string
	value     string
	options   []string
	labels    []string
	multiline bool
	browse    string
}

type formDialog struct {
	title   string
	fields  []formField
	entries map[string]*widget.Entry
	selects map[string]*widget.Select
	problem *widget.Label
	popup   *dialog.CustomDialog
	submit  func(values map[string]string) error
	closed  func()
}

func (f *formDialog) value(field formField) string {
	if selector, found := f.selects[field.key]; found {
		for index, label := range field.labels {
			if label == selector.Selected {
				return field.options[index]
			}
		}
		return field.value
	}
	return f.entries[field.key].Text
}

func (f *formDialog) values() map[string]string {
	values := map[string]string{}
	for _, field := range f.fields {
		values[field.key] = f.value(field)
	}
	return values
}

func (f *formDialog) set(key, value string) {
	if entry, found := f.entries[key]; found {
		entry.SetText(value)
		return
	}
	for _, field := range f.fields {
		if field.key != key {
			continue
		}
		for index, option := range field.options {
			if option == value {
				f.selects[key].SetSelected(field.labels[index])
			}
		}
	}
}

func (f *formDialog) save() {
	if err := f.submit(f.values()); err != nil {
		f.problem.SetText("Not saved: " + err.Error())
		f.problem.Show()
		return
	}
	f.close()
}

func (f *formDialog) close() {
	f.popup.Hide()
	if f.closed != nil {
		f.closed()
	}
}

func newFormDialog(window fyne.Window, title, saveLabel string, fields []formField, submit func(values map[string]string) error) *formDialog {
	f := &formDialog{title: title, fields: fields, entries: map[string]*widget.Entry{}, selects: map[string]*widget.Select{}, submit: submit}
	form := widget.NewForm()
	for index := range fields {
		field := &fields[index]
		var control fyne.CanvasObject
		switch {
		case field.options != nil:
			if field.labels == nil {
				field.labels = field.options
			}
			selector := widget.NewSelect(field.labels, nil)
			for position, option := range field.options {
				if option == field.value {
					selector.SetSelected(field.labels[position])
				}
			}
			if selector.Selected == "" && len(field.labels) > 0 {
				selector.SetSelected(field.labels[0])
			}
			f.selects[field.key] = selector
			control = selector
		default:
			entry := widget.NewEntry()
			if field.multiline {
				entry = widget.NewMultiLineEntry()
				entry.Wrapping = fyne.TextWrapWord
				entry.SetMinRowsVisible(3)
			} else {
				entry.OnSubmitted = func(string) { f.save() }
			}
			entry.SetText(field.value)
			f.entries[field.key] = entry
			control = entry
			if field.browse != "" {
				mode := field.browse
				browse := widget.NewButton("Browse…", func() {
					if mode == "save" {
						dialog.ShowFileSave(func(target fyne.URIWriteCloser, err error) {
							if err == nil && target != nil {
								path := target.URI().Path()
								_ = target.Close()
								entry.SetText(path)
							}
						}, window)
						return
					}
					dialog.ShowFileOpen(func(source fyne.URIReadCloser, err error) {
						if err == nil && source != nil {
							entry.SetText(source.URI().Path())
							_ = source.Close()
						}
					}, window)
				})
				control = container.NewBorder(nil, nil, nil, browse, entry)
			}
		}
		item := widget.NewFormItem(field.label, control)
		item.HintText = field.hint
		form.AppendItem(item)
	}
	f.problem = widget.NewLabel("")
	f.problem.Importance = widget.DangerImportance
	f.problem.Wrapping = fyne.TextWrapWord
	f.problem.Hide()
	cancel := widget.NewButton("Cancel", func() { f.close() })
	save := widget.NewButton(saveLabel, func() { f.save() })
	save.Importance = widget.HighImportance
	content := container.NewVBox(form, f.problem)
	f.popup = dialog.NewCustomWithoutButtons(title, content, window)
	f.popup.SetButtons([]fyne.CanvasObject{cancel, layout.NewSpacer(), save})
	f.popup.Resize(fyne.NewSize(560, 0))
	f.popup.Show()
	return f
}

type confirmDialog struct {
	title     string
	statement string
	popup     *dialog.CustomDialog
	action    func()
	closed    func()
}

func (c *confirmDialog) accept() {
	c.popup.Hide()
	c.closed()
	c.action()
}

func (c *confirmDialog) cancel() {
	c.popup.Hide()
	c.closed()
}

func newConfirmDialog(window fyne.Window, title, statement, accept string, danger bool, action func(), closed func()) *confirmDialog {
	c := &confirmDialog{title: title, statement: statement, action: action, closed: closed}
	text := widget.NewLabel(statement)
	text.Wrapping = fyne.TextWrapWord
	body := container.NewVScroll(text)
	lines := 1
	for _, line := range strings.Split(statement, "\n") {
		lines += 1 + len(line)/64
	}
	height := float32(lines) * fynetheme.TextSize() * 1.5
	body.SetMinSize(fyne.NewSize(540, min(max(height, 70), 380)))
	cancel := widget.NewButton("Cancel", func() { c.cancel() })
	confirm := widget.NewButton(accept, func() { c.accept() })
	confirm.Importance = widget.HighImportance
	if danger {
		confirm.Importance = widget.DangerImportance
	}
	c.popup = dialog.NewCustomWithoutButtons(title, body, window)
	c.popup.SetButtons([]fyne.CanvasObject{cancel, layout.NewSpacer(), confirm})
	c.popup.Show()
	if danger {
		window.Canvas().Focus(cancel)
	} else {
		window.Canvas().Focus(confirm)
	}
	return c
}
