package theme

import (
	"fmt"
	"math"
	"strconv"
	"strings"
)

const (
	MinTextContrast      = 4.5
	MinComponentContrast = 3.0
)

type Color struct {
	R, G, B uint8
}

func Hex(value string) Color {
	color, err := ParseHex(value)
	if err != nil {
		panic(err)
	}
	return color
}

func ParseHex(value string) (Color, error) {
	trimmed := strings.TrimPrefix(strings.TrimSpace(value), "#")
	if len(trimmed) != 6 {
		return Color{}, fmt.Errorf("%q is not a color: use the form #RRGGBB", value)
	}
	number, err := strconv.ParseUint(trimmed, 16, 32)
	if err != nil {
		return Color{}, fmt.Errorf("%q is not a color: use the form #RRGGBB", value)
	}
	return Color{R: uint8(number >> 16), G: uint8(number >> 8), B: uint8(number)}, nil
}

func (c Color) Hex() string {
	return fmt.Sprintf("#%02X%02X%02X", c.R, c.G, c.B)
}

func channel(value uint8) float64 {
	scaled := float64(value) / 255
	if scaled <= 0.04045 {
		return scaled / 12.92
	}
	return math.Pow((scaled+0.055)/1.055, 2.4)
}

func Luminance(c Color) float64 {
	return 0.2126*channel(c.R) + 0.7152*channel(c.G) + 0.0722*channel(c.B)
}

func Contrast(a, b Color) float64 {
	first, second := Luminance(a), Luminance(b)
	if first < second {
		first, second = second, first
	}
	return (first + 0.05) / (second + 0.05)
}

func Mix(a, b Color, share float64) Color {
	blend := func(x, y uint8) uint8 {
		return uint8(math.Round(float64(x)*(1-share) + float64(y)*share))
	}
	return Color{R: blend(a.R, b.R), G: blend(a.G, b.G), B: blend(a.B, b.B)}
}

type Severity string

const (
	SeverityInfo     Severity = "info"
	SeverityLow      Severity = "low"
	SeverityMedium   Severity = "medium"
	SeverityHigh     Severity = "high"
	SeverityCritical Severity = "critical"
)

var Severities = []Severity{SeverityInfo, SeverityLow, SeverityMedium, SeverityHigh, SeverityCritical}

func (s Severity) Rank() int {
	for index, candidate := range Severities {
		if candidate == s {
			return index
		}
	}
	return -1
}

func (s Severity) Label() string {
	if s.Rank() < 0 {
		return "Unknown"
	}
	return strings.ToUpper(string(s)[:1]) + string(s)[1:]
}

func (s Severity) Icon() string {
	switch s {
	case SeverityCritical:
		return "octagon-alert"
	case SeverityHigh:
		return "triangle-alert"
	case SeverityMedium:
		return "circle-alert"
	case SeverityLow:
		return "circle-minus"
	default:
		return "info"
	}
}

func (s Severity) Glyph() string {
	switch s {
	case SeverityCritical:
		return "!!"
	case SeverityHigh:
		return "! "
	case SeverityMedium:
		return "* "
	case SeverityLow:
		return "- "
	default:
		return "i "
	}
}

type Palette struct {
	Background Color
	Surface    Color
	SurfaceAlt Color
	Border     Color
	Divider    Color
	Text       Color
	TextMuted  Color
	Accent     Color
}

type Semantic struct {
	Critical Color
	High     Color
	Medium   Color
	Low      Color
	Info     Color
	Success  Color
	Warning  Color
	Error    Color
}

func (s Semantic) Severity(level Severity) Color {
	switch level {
	case SeverityCritical:
		return s.Critical
	case SeverityHigh:
		return s.High
	case SeverityMedium:
		return s.Medium
	case SeverityLow:
		return s.Low
	default:
		return s.Info
	}
}

type Spacing struct {
	XS, S, M, L, XL, XXL float32
	RowHeight            float32
	RowGap               float32
}

type TypeScale struct {
	Caption, Body, Subheading, Heading, Figure float32
}

type Radii struct {
	Chip, Control, Card float32
}

type Tokens struct {
	Name      string
	Label     string
	Dark      bool
	Palette   Palette
	OnAccent  Color
	Focus     Color
	Selection Color
	Semantic  Semantic
	Spacing   Spacing
	Type      TypeScale
	Radius    Radii
	Elevation float32
	Motion    bool
	Warnings  []string
}

type definition struct {
	name  string
	label string
	light Palette
	dark  Palette
}

func palette(background, surface, alt, border, divider, text, muted, accent string) Palette {
	return Palette{Background: Hex(background), Surface: Hex(surface), SurfaceAlt: Hex(alt), Border: Hex(border), Divider: Hex(divider), Text: Hex(text), TextMuted: Hex(muted), Accent: Hex(accent)}
}

var themes = []definition{
	{"slate", "Neutral slate",
		palette("#F6F7F8", "#FFFFFF", "#ECEEF1", "#6E7681", "#DDE0E4", "#16191D", "#535B66", "#2459C9"),
		palette("#121416", "#191C1F", "#23272B", "#7F8894", "#2D3237", "#ECEEF0", "#A3ABB5", "#84ADFF")},
	{"blue", "Blue",
		palette("#F5F7F9", "#FFFFFF", "#EBEEF2", "#6D7682", "#DCE0E5", "#16191D", "#535B66", "#1257C4"),
		palette("#121416", "#191C1F", "#23262B", "#7F8894", "#2D3137", "#ECEEF0", "#A5ABB3", "#6FA8FF")},
	{"green", "Green",
		palette("#F5F9F7", "#FFFFFF", "#EBF2EE", "#6D8276", "#DCE5E0", "#161D19", "#53665B", "#136B38"),
		palette("#121614", "#191F1C", "#232B26", "#7F9488", "#2D3731", "#ECF0EE", "#A5B3AB", "#5FD08A")},
	{"crimson", "Crimson",
		palette("#F9F5F6", "#FFFFFF", "#F2EBEC", "#826D70", "#E5DCDD", "#1D1617", "#665356", "#A8112F"),
		palette("#161213", "#1F191A", "#2B2324", "#947F82", "#372D2F", "#F0ECED", "#B3A5A8", "#FF7A90")},
	{"amber", "Amber",
		palette("#F9F7F5", "#FFFFFF", "#F2EFEB", "#827A6D", "#E5E2DC", "#1D1A16", "#665F53", "#8A5200"),
		palette("#161512", "#1F1D19", "#2B2823", "#948C7F", "#37332D", "#F0EEEC", "#B3AEA5", "#F2B544")},
	{"purple", "Purple",
		palette("#F7F5F9", "#FFFFFF", "#EEEBF2", "#756D82", "#E0DCE5", "#19161D", "#5B5366", "#6230C2"),
		palette("#141216", "#1B191F", "#26232B", "#877F94", "#312D37", "#EEECF0", "#ABA5B3", "#B79BFF")},
	{"contrast", "High contrast",
		palette("#FFFFFF", "#FFFFFF", "#E6E6E6", "#000000", "#000000", "#000000", "#1A1A1A", "#0000C8"),
		palette("#000000", "#000000", "#1F1F1F", "#FFFFFF", "#FFFFFF", "#FFFFFF", "#E6E6E6", "#FFD60A")},
}

var (
	lightSemantic = Semantic{Critical: Hex("#C01439"), High: Hex("#8E2E0B"), Medium: Hex("#7F600A"), Low: Hex("#1E5FC4"), Info: Hex("#59626E"), Success: Hex("#16713D"), Warning: Hex("#7F600A"), Error: Hex("#C01439")}
	darkSemantic  = Semantic{Critical: Hex("#FF7B88"), High: Hex("#FFA057"), Medium: Hex("#F2CC60"), Low: Hex("#7AB7FF"), Info: Hex("#B6BEC9"), Success: Hex("#56D364"), Warning: Hex("#E3B341"), Error: Hex("#FF7B88")}
)

type Mode string

const (
	ModeSystem Mode = "system"
	ModeLight  Mode = "light"
	ModeDark   Mode = "dark"
)

type Options struct {
	Name          string
	Mode          Mode
	SystemDark    bool
	Accent        string
	Density       string
	TextScale     int
	ReducedMotion bool
}

func Names() []string {
	names := make([]string, len(themes))
	for index, theme := range themes {
		names[index] = theme.name
	}
	return names
}

func Label(name string) string {
	for _, theme := range themes {
		if theme.name == name {
			return theme.label
		}
	}
	return ""
}

func Resolve(options Options) Tokens {
	selected := themes[0]
	var warnings []string
	found := false
	for _, theme := range themes {
		if theme.name == options.Name {
			selected, found = theme, true
		}
	}
	if !found && options.Name != "" {
		warnings = append(warnings, fmt.Sprintf("unknown theme %q, using %s", options.Name, selected.name))
	}
	dark := options.Mode == ModeDark || (options.Mode != ModeLight && options.SystemDark)
	tokens := Tokens{Name: selected.name, Label: selected.label, Dark: dark, Palette: selected.light, Semantic: lightSemantic}
	if dark {
		tokens.Palette, tokens.Semantic = selected.dark, darkSemantic
	}
	if options.Accent != "" {
		custom, err := ParseHex(options.Accent)
		if err != nil {
			warnings = append(warnings, "custom accent ignored: "+err.Error())
		} else {
			adjusted, changed := AdjustForContrast(custom, []Color{tokens.Palette.Background, tokens.Palette.Surface, tokens.Palette.SurfaceAlt}, MinTextContrast)
			if changed {
				warnings = append(warnings, fmt.Sprintf("custom accent %s was adjusted to %s to keep a %.1f:1 contrast", custom.Hex(), adjusted.Hex(), MinTextContrast))
			}
			tokens.Palette.Accent = adjusted
		}
	}
	tokens.OnAccent = BestOn(tokens.Palette.Accent)
	tokens.Focus = tokens.Palette.Accent
	tokens.Selection = Mix(tokens.Palette.Surface, tokens.Palette.Accent, 0.12)
	scale := float32(options.TextScale) / 100
	if options.TextScale < 80 || options.TextScale > 200 {
		scale = 1
	}
	tokens.Type = TypeScale{Caption: 12 * scale, Body: 14 * scale, Subheading: 16 * scale, Heading: 24 * scale, Figure: 28 * scale}
	if options.Density == "compact" {
		tokens.Spacing = Spacing{XS: 4, S: 4, M: 8, L: 12, XL: 16, XXL: 24, RowHeight: 24 * scale, RowGap: 4}
	} else {
		tokens.Spacing = Spacing{XS: 4, S: 8, M: 12, L: 16, XL: 24, XXL: 32, RowHeight: 32 * scale, RowGap: 4}
	}
	tokens.Radius = Radii{Chip: 4, Control: 8, Card: 12}
	tokens.Elevation = 1
	if selected.name == "contrast" {
		tokens.Radius = Radii{}
		tokens.Elevation = 0
	}
	tokens.Motion = !options.ReducedMotion
	tokens.Warnings = warnings
	return tokens
}

func BestOn(background Color) Color {
	white, black := Color{255, 255, 255}, Color{0, 0, 0}
	if Contrast(white, background) >= Contrast(black, background) {
		return white
	}
	return black
}

func minContrast(color Color, against []Color) float64 {
	lowest := math.Inf(1)
	for _, other := range against {
		lowest = math.Min(lowest, Contrast(color, other))
	}
	return lowest
}

func AdjustForContrast(color Color, against []Color, minimum float64) (Color, bool) {
	if minContrast(color, against) >= minimum {
		return color, false
	}
	darkTarget, lightTarget := Color{0, 0, 0}, Color{255, 255, 255}
	target := lightTarget
	if minContrast(darkTarget, against) > minContrast(lightTarget, against) {
		target = darkTarget
	}
	for step := 1; step <= 100; step++ {
		candidate := Mix(color, target, float64(step)/100)
		if minContrast(candidate, against) >= minimum {
			return candidate, true
		}
	}
	return target, true
}
