package gui

import (
	_ "embed"
	"fmt"
	"image/color"
	"sync"

	"fyne.io/fyne/v2"
	fynetheme "fyne.io/fyne/v2/theme"

	"github.com/unalivin/certwatch/internal/theme"
)

//go:embed font/NotoSans-Medium.ttf
var mediumFont []byte

var mediumResource = fyne.NewStaticResource("NotoSans-Medium.ttf", mediumFont)

type skin struct {
	tokens theme.Tokens
}

func rgb(c theme.Color) color.Color {
	return color.NRGBA{R: c.R, G: c.G, B: c.B, A: 0xFF}
}

func alpha(c theme.Color, a uint8) color.Color {
	return color.NRGBA{R: c.R, G: c.G, B: c.B, A: a}
}

func (s skin) variant() fyne.ThemeVariant {
	if s.tokens.Dark {
		return fynetheme.VariantDark
	}
	return fynetheme.VariantLight
}

func (s skin) Color(name fyne.ThemeColorName, _ fyne.ThemeVariant) color.Color {
	t := s.tokens
	p := t.Palette
	switch name {
	case fynetheme.ColorNameBackground:
		return rgb(p.Background)
	case fynetheme.ColorNameButton, fynetheme.ColorNameHeaderBackground:
		return rgb(p.SurfaceAlt)
	case fynetheme.ColorNameDisabledButton:
		return rgb(p.Divider)
	case fynetheme.ColorNameDisabled:
		return rgb(theme.Mix(p.TextMuted, p.Surface, 0.35))
	case fynetheme.ColorNameError:
		return rgb(t.Semantic.Error)
	case fynetheme.ColorNameSuccess:
		return rgb(t.Semantic.Success)
	case fynetheme.ColorNameWarning:
		return rgb(t.Semantic.Warning)
	case fynetheme.ColorNameFocus:
		return rgb(t.Focus)
	case fynetheme.ColorNameForeground:
		return rgb(p.Text)
	case fynetheme.ColorNameForegroundOnPrimary:
		return rgb(t.OnAccent)
	case fynetheme.ColorNameForegroundOnError:
		return rgb(theme.BestOn(t.Semantic.Error))
	case fynetheme.ColorNameForegroundOnSuccess:
		return rgb(theme.BestOn(t.Semantic.Success))
	case fynetheme.ColorNameForegroundOnWarning:
		return rgb(theme.BestOn(t.Semantic.Warning))
	case fynetheme.ColorNameHover:
		return alpha(p.Text, 0x14)
	case fynetheme.ColorNamePressed:
		return alpha(p.Text, 0x29)
	case fynetheme.ColorNameHyperlink, fynetheme.ColorNamePrimary:
		return rgb(p.Accent)
	case fynetheme.ColorNameInputBackground, fynetheme.ColorNameMenuBackground, fynetheme.ColorNameOverlayBackground:
		return rgb(p.Surface)
	case fynetheme.ColorNameInputBorder:
		return rgb(p.Border)
	case fynetheme.ColorNamePlaceHolder:
		return rgb(p.TextMuted)
	case fynetheme.ColorNameScrollBar:
		return alpha(p.TextMuted, 0xA0)
	case fynetheme.ColorNameScrollBarBackground:
		return alpha(p.Divider, 0x40)
	case fynetheme.ColorNameSelection:
		return rgb(t.Selection)
	case fynetheme.ColorNameSeparator:
		return rgb(p.Divider)
	case fynetheme.ColorNameShadow:
		return color.NRGBA{A: uint8(0x38 * t.Elevation)}
	}
	return fynetheme.DefaultTheme().Color(name, s.variant())
}

func (s skin) Font(style fyne.TextStyle) fyne.Resource {
	if style.Bold && !style.Italic && !style.Monospace && !style.Symbol {
		return mediumResource
	}
	return fynetheme.DefaultTheme().Font(style)
}

func (s skin) Icon(name fyne.ThemeIconName) fyne.Resource {
	return fynetheme.DefaultTheme().Icon(name)
}

func (s skin) Size(name fyne.ThemeSizeName) float32 {
	t := s.tokens
	switch name {
	case fynetheme.SizeNamePadding:
		return t.Spacing.XS
	case fynetheme.SizeNameInnerPadding:
		return t.Spacing.S
	case fynetheme.SizeNameText:
		return t.Type.Body
	case fynetheme.SizeNameHeadingText:
		return t.Type.Heading
	case fynetheme.SizeNameSubHeadingText:
		return t.Type.Subheading
	case fynetheme.SizeNameCaptionText:
		return t.Type.Caption
	case fynetheme.SizeNameInlineIcon:
		return iconSize(t)
	case fynetheme.SizeNameInputRadius, fynetheme.SizeNameSelectionRadius:
		return t.Radius.Control
	case fynetheme.SizeNameScrollBarRadius:
		return t.Radius.Chip
	case fynetheme.SizeNameInputBorder, fynetheme.SizeNameSeparatorThickness:
		return 1
	}
	return fynetheme.DefaultTheme().Size(name)
}

func iconSize(t theme.Tokens) float32 {
	return t.Spacing.L * t.Type.Body / 14
}

var iconPaths = map[string]string{
	"layout-dashboard":   "M3 3h8v10H3z M13 3h8v6h-8z M13 11h8v10h-8z M3 15h8v6H3z",
	"bell":               "M12 2a2 2 0 0 1 2 2v.3A7 7 0 0 1 19 11v4l2 3v1H3v-1l2-3v-4a7 7 0 0 1 5-6.7V4a2 2 0 0 1 2-2z M9.5 20h5a2.5 2.5 0 0 1-5 0z",
	"settings":           "M3 5h9v2H3z M16 5h5v2h-5z M12 3h2v6h-2z M3 11h3v2H3z M10 11h11v2H10z M6 9h2v6H6z M3 17h11v2H3z M18 17h3v2h-3z M14 15h2v6h-2z",
	"info":               "M12 2a10 10 0 1 0 0 20a10 10 0 1 0 0-20z M12 4a8 8 0 1 1 0 16a8 8 0 1 1 0-16z M11 7h2v2h-2z M11 11h2v6h-2z",
	"octagon-alert":      "M8 2h8l6 6v8l-6 6H8l-6-6V8z M11 6v8h2V6z M11 16v2h2v-2z",
	"triangle-alert":     "M12 2L23 21H1z M11 9v6h2V9z M11 17v2h2v-2z",
	"circle-alert":       "M12 2a10 10 0 1 1 0 20a10 10 0 1 1 0-20z M11 6v8h2V6z M11 16v2h2v-2z",
	"circle-minus":       "M12 2a10 10 0 1 1 0 20a10 10 0 1 1 0-20z M7 11v2h10v-2z",
	"status-open":        "M12 3a9 9 0 1 0 0 18a9 9 0 1 0 0-18z M12 5a7 7 0 1 1 0 14a7 7 0 1 1 0-14z",
	"status-done":        "M12 3a9 9 0 1 0 0 18a9 9 0 1 0 0-18z M8.5 11L10.5 13L16 7.5L17.5 9L10.5 16L7 12.5z",
	"sidebar":            "M3 4h18v16H3z M5 6v12h4V6z",
	"x":                  "M6.4 5L12 10.6L17.6 5L19 6.4L13.4 12L19 17.6L17.6 19L12 13.4L6.4 19L5 17.6L10.6 12L5 6.4z",
	"check":              "M9 16.2L4.8 12L3.4 13.4L9 19L21 7L19.6 5.6z",
	"plus":               "M11 5h2v6h6v2h-6v6h-2v-6H5v-2h6z",
	"copy":               "M8 3h12v14H8z M10 5v10h8V5z M4 7h2v12h10v2H4z",
	"trash":              "M9 3h6v2h5v2H4V5h5z M6 9h12v12H6z M8 11v8h8v-8z",
	"pencil":             "M16.5 3L21 7.5L8.5 20H4v-4.5z M6 16.3V18h1.7L18.2 7.5L16.5 5.8z",
	"download":           "M11 3h2v9.2l3.6-3.6L18 10l-6 6l-6-6l1.4-1.4L11 12.2z M4 19h16v2H4z",
	"upload":             "M11 16h2V6.8l3.6 3.6L18 9l-6-6l-6 6l1.4 1.4L11 6.8z M4 19h16v2H4z",
	"arrow-right":        "M4 11h12.2l-4.6-4.6L13 5l7 7l-7 7l-1.4-1.4l4.6-4.6H4z",
	"palette":            "M3 3h8v8H3z M13 3h8v8h-8z M3 13h8v8H3z M13 13h8v8h-8z M15 15v4h4v-4z",
	"monitor":            "M2 4h20v13H2z M4 6v9h16V6z M8 19h8v2H8z",
	"message":            "M2 4h20v14H9l-5 4v-4H2z M4 6v10h2v2l2.5-2H20V6z",
	"badge-check":        "M12 1l2.6 2.1l3.3-.4l1.2 3.1l3.1 1.2l-.4 3.3L23.9 13l-2.1 2.6l.4 3.3l-3.1 1.2l-1.2 3.1l-3.3-.4L12 24.9l-2.6-2.1l-3.3.4l-1.2-3.1l-3.1-1.2l.4-3.3L.1 13l2.1-2.6l-.4-3.3l3.1-1.2l1.2-3.1l3.3.4z M10.6 17.2l7-7l-1.5-1.4l-5.5 5.5l-2.7-2.7l-1.5 1.4z",
	"globe":              "M12 2a10 10 0 1 0 0 20a10 10 0 1 0 0-20z M4.1 11a8 8 0 0 1 4.6-6.3A15 15 0 0 0 7 11z M4.1 13H7a15 15 0 0 0 1.7 6.3A8 8 0 0 1 4.1 13z M9 11a13 13 0 0 1 3-6.6A13 13 0 0 1 15 11z M9 13h6a13 13 0 0 1-3 6.6A13 13 0 0 1 9 13z M17 11a15 15 0 0 0-1.7-6.3A8 8 0 0 1 19.9 11z M17 13h2.9a8 8 0 0 1-4.6 6.3A15 15 0 0 0 17 13z",
	"calendar-clock":     "M7 2h2v2h6V2h2v2h4v8h-2v-2H5v10h7v2H3V4h4z M5 6v2h14V6z M18 13a5 5 0 1 0 0 10a5 5 0 1 0 0-10z M17.2 15h1.6v2.6l1.9 1.3l-.9 1.3l-2.6-1.8z",
	"link-2":             "M2 12a5 5 0 0 1 5-5h4v2H7a3 3 0 0 0 0 6h4v2H7a5 5 0 0 1-5-5z M13 7h4a5 5 0 0 1 0 10h-4v-2h4a3 3 0 0 0 0-6h-4z M8 11h8v2H8z",
	"sliders-horizontal": "M3 5h9v2H3z M16 5h5v2h-5z M12 3h2v6h-2z M3 11h3v2H3z M10 11h11v2H10z M6 9h2v6H6z M3 17h11v2H3z M18 17h3v2h-3z M14 15h2v6h-2z",
	"file-text":          "M5 2h10l5 5v15H5z M7 4v16h11V8h-4V4z M9 11h7v2H9z M9 15h7v2H9z",
	"settings-2":         "M10.3 2h3.4l.5 2.6l1.8.8l2.2-1.5l2.4 2.4l-1.5 2.2l.8 1.8l2.6.5v3.4l-2.6.5l-.8 1.8l1.5 2.2l-2.4 2.4l-2.2-1.5l-1.8.8l-.5 2.6h-3.4l-.5-2.6l-1.8-.8l-2.2 1.5l-2.4-2.4l1.5-2.2l-.8-1.8L1.5 13.7v-3.4l2.6-.5l.8-1.8l-1.5-2.2l2.4-2.4l2.2 1.5l1.8-.8z M12 8a4 4 0 1 0 0 8a4 4 0 1 0 0-8z",
	"refresh":            "M12 4a8 8 0 0 1 7.4 5H22l-4 4l-4-4h3.2A6 6 0 1 0 18 12h2a8 8 0 1 1-8-8z",
}

var (
	iconCache = map[string]fyne.Resource{}
	iconLock  sync.Mutex
)

func icon(name string, tint theme.Color) fyne.Resource {
	key := name + tint.Hex()
	iconLock.Lock()
	defer iconLock.Unlock()
	if cached, found := iconCache[key]; found {
		return cached
	}
	path, found := iconPaths[name]
	if !found {
		path = iconPaths["info"]
	}
	content := fmt.Sprintf(`<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24"><path fill="%s" d="%s"/></svg>`, tint.Hex(), path)
	resource := fyne.NewStaticResource(key+".svg", []byte(content))
	iconCache[key] = resource
	return resource
}

var (
	glyphCache = map[string]fyne.Resource{}
	glyphLock  sync.Mutex
)

func glyph(name string) fyne.Resource {
	glyphLock.Lock()
	defer glyphLock.Unlock()
	if cached, found := glyphCache[name]; found {
		return cached
	}
	path, found := iconPaths[name]
	if !found {
		path = iconPaths["info"]
	}
	content := fmt.Sprintf(`<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24"><path fill="#000000" d="%s"/></svg>`, path)
	resource := fynetheme.NewThemedResource(fyne.NewStaticResource("glyph-"+name+".svg", []byte(content)))
	glyphCache[name] = resource
	return resource
}
