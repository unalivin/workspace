package tui

import (
	"strings"
	"unicode"

	"github.com/charmbracelet/lipgloss"
	"github.com/charmbracelet/x/ansi"

	"github.com/unalivin/templateapp/internal/theme"
)

type styles struct {
	terminal theme.Terminal
	text     lipgloss.Style
	muted    lipgloss.Style
	accent   lipgloss.Style
	heading  lipgloss.Style
	selected lipgloss.Style
	border   lipgloss.Style
	status   lipgloss.Style
	success  lipgloss.Style
	warning  lipgloss.Style
	failure  lipgloss.Style
}

func convert(style theme.Style) lipgloss.Style {
	out := lipgloss.NewStyle()
	if style.Foreground != "" {
		out = out.Foreground(lipgloss.Color(style.Foreground))
	}
	if style.Background != "" {
		out = out.Background(lipgloss.Color(style.Background))
	}
	return out.Bold(style.Bold).Underline(style.Underline).Reverse(style.Reverse)
}

func newStyles(terminal theme.Terminal) styles {
	return styles{
		terminal: terminal,
		text:     convert(terminal.Text),
		muted:    convert(terminal.Muted),
		accent:   convert(terminal.Accent),
		heading:  convert(terminal.Heading),
		selected: convert(terminal.Selected),
		border:   convert(terminal.Border),
		status:   convert(terminal.StatusBar),
		success:  convert(terminal.Success),
		warning:  convert(terminal.Warning),
		failure:  convert(terminal.Error),
	}
}

func (s styles) severity(level theme.Severity) string {
	return convert(s.terminal.Severity(level)).Render(level.Glyph() + " " + level.Label())
}

func fit(text string, width int) string {
	if width <= 0 {
		return ""
	}
	text = strings.ReplaceAll(text, "\n", " ")
	if lipgloss.Width(text) > width {
		text = ansi.Truncate(text, width, "…")
	}
	return text + strings.Repeat(" ", width-lipgloss.Width(text))
}

func wrap(text string, width int) []string {
	if width < 8 {
		width = 8
	}
	var lines []string
	for _, paragraph := range strings.Split(text, "\n") {
		if strings.TrimSpace(paragraph) == "" {
			lines = append(lines, "")
			continue
		}
		indent := paragraph[:len(paragraph)-len(strings.TrimLeft(paragraph, " "))]
		current := indent
		for _, word := range strings.Fields(paragraph) {
			for lipgloss.Width(word) > width-len(indent) {
				cut := ansi.Truncate(word, width-len(indent), "")
				if current != indent {
					lines = append(lines, current)
				}
				lines = append(lines, indent+cut)
				word, current = word[len(cut):], indent
			}
			switch {
			case current == indent:
				current += word
			case lipgloss.Width(current)+1+lipgloss.Width(word) <= width:
				current += " " + word
			default:
				lines = append(lines, current)
				current = indent + word
			}
		}
		lines = append(lines, current)
	}
	return lines
}

func window(lines []string, offset, height int) ([]string, int) {
	if height < 1 {
		height = 1
	}
	if offset > len(lines)-height {
		offset = len(lines) - height
	}
	if offset < 0 {
		offset = 0
	}
	end := offset + height
	if end > len(lines) {
		end = len(lines)
	}
	return lines[offset:end], offset
}

type input struct {
	runes  []rune
	cursor int
	limit  int
}

func newInput(value string, limit int) input {
	runes := []rune(value)
	return input{runes: runes, cursor: len(runes), limit: limit}
}

func (i input) value() string { return string(i.runes) }

func (i *input) handle(key string, typed []rune) bool {
	switch key {
	case "left":
		if i.cursor > 0 {
			i.cursor--
		}
	case "right":
		if i.cursor < len(i.runes) {
			i.cursor++
		}
	case "home", "ctrl+a":
		i.cursor = 0
	case "end", "ctrl+e":
		i.cursor = len(i.runes)
	case "backspace":
		if i.cursor > 0 {
			i.runes = append(i.runes[:i.cursor-1], i.runes[i.cursor:]...)
			i.cursor--
		}
	case "delete":
		if i.cursor < len(i.runes) {
			i.runes = append(i.runes[:i.cursor], i.runes[i.cursor+1:]...)
		}
	case "ctrl+u":
		i.runes, i.cursor = i.runes[i.cursor:], 0
	default:
		if len(typed) == 0 {
			return false
		}
		for _, r := range typed {
			if unicode.IsControl(r) || (i.limit > 0 && len(i.runes) >= i.limit) {
				continue
			}
			i.runes = append(i.runes[:i.cursor], append([]rune{r}, i.runes[i.cursor:]...)...)
			i.cursor++
		}
	}
	return true
}

func (i input) view(width int, focused bool) string {
	text := i.value()
	if !focused {
		return fit(text, width)
	}
	before, after := string(i.runes[:i.cursor]), string(i.runes[i.cursor:])
	shown := before + "▏" + after
	for lipgloss.Width(shown) > width && len(before) > 0 {
		_, size := []rune(before)[0], len(string([]rune(before)[0]))
		before = before[size:]
		shown = before + "▏" + after
	}
	return fit(shown, width)
}

type fieldKind int

const (
	fieldText fieldKind = iota
	fieldChoice
)

type field struct {
	key     string
	label   string
	help    string
	kind    fieldKind
	input   input
	options []string
	labels  []string
	choice  int
}

func textField(key, label, value, help string, limit int) field {
	return field{key: key, label: label, help: help, kind: fieldText, input: newInput(value, limit)}
}

func choiceField(key, label, value, help string, options, labels []string) field {
	selected := 0
	for index, option := range options {
		if option == value {
			selected = index
		}
	}
	if labels == nil {
		labels = options
	}
	return field{key: key, label: label, help: help, kind: fieldChoice, options: options, labels: labels, choice: selected}
}

func (f field) value() string {
	if f.kind == fieldChoice {
		return f.options[f.choice]
	}
	return f.input.value()
}

type form struct {
	title   string
	fields  []field
	focus   int
	problem string
	submit  func(values map[string]string) error
}

func (f *form) values() map[string]string {
	values := map[string]string{}
	for _, item := range f.fields {
		values[item.key] = item.value()
	}
	return values
}

type formResult int

const (
	formOpen formResult = iota
	formSaved
	formCancelled
)

func (f *form) handle(key string, typed []rune) formResult {
	current := &f.fields[f.focus]
	switch key {
	case "esc":
		return formCancelled
	case "tab", "down":
		f.focus = (f.focus + 1) % len(f.fields)
	case "shift+tab", "up":
		f.focus = (f.focus + len(f.fields) - 1) % len(f.fields)
	case "enter", "ctrl+s":
		if key == "enter" && f.focus < len(f.fields)-1 {
			f.focus++
			return formOpen
		}
		if err := f.submit(f.values()); err != nil {
			f.problem = err.Error()
			return formOpen
		}
		return formSaved
	default:
		if current.kind == fieldChoice {
			switch key {
			case "left":
				current.choice = (current.choice + len(current.options) - 1) % len(current.options)
			case "right", " ":
				current.choice = (current.choice + 1) % len(current.options)
			}
			return formOpen
		}
		current.input.handle(key, typed)
	}
	return formOpen
}

func (f *form) view(st styles, width int) []string {
	lines := []string{st.heading.Render(f.title), ""}
	labelWidth := 0
	for _, item := range f.fields {
		if len(item.label) > labelWidth {
			labelWidth = len(item.label)
		}
	}
	valueWidth := width - labelWidth - 6
	if valueWidth < 10 {
		valueWidth = 10
	}
	for index, item := range f.fields {
		focused := index == f.focus
		marker := "  "
		if focused {
			marker = st.accent.Render("▌ ")
		}
		var value string
		if item.kind == fieldChoice {
			value = "‹ " + item.labels[item.choice] + " ›"
		} else {
			value = item.input.view(valueWidth, focused)
		}
		label := fit(item.label, labelWidth)
		if focused {
			label = st.accent.Render(label)
		}
		lines = append(lines, marker+label+"  "+value)
		if focused && item.help != "" {
			for _, helpLine := range wrap(item.help, width-4) {
				lines = append(lines, "    "+st.muted.Render(helpLine))
			}
		}
	}
	lines = append(lines, "")
	if f.problem != "" {
		for index, line := range wrap("Not saved: "+f.problem, width-2) {
			lead := "  "
			if index == 0 {
				lead = "✖ "
			}
			lines = append(lines, st.failure.Render(lead+line))
		}
		lines = append(lines, "")
	}
	lines = append(lines, st.muted.Render("Tab next field · Left/Right change a choice · Enter or Ctrl+S save · Esc cancel"))
	return lines
}

type column struct {
	key   string
	title string
	width int
	flex  bool
}

func layoutColumns(columns []column, width int) []int {
	widths := make([]int, len(columns))
	used, flexible := 0, 0
	for index, item := range columns {
		widths[index] = item.width
		used += item.width + 2
		if item.flex {
			flexible++
		}
	}
	if spare := width - used; spare > 0 && flexible > 0 {
		for index, item := range columns {
			if item.flex {
				widths[index] += spare / flexible
			}
		}
	}
	return widths
}

func renderRow(cells []string, widths []int) string {
	var builder strings.Builder
	for index, cell := range cells {
		if index >= len(widths) {
			break
		}
		builder.WriteString(fit(cell, widths[index]))
		builder.WriteString("  ")
	}
	return builder.String()
}
