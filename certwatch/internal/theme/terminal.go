package theme

import "strings"

type Profile int

const (
	ProfileMono Profile = iota
	ProfileANSI16
	ProfileANSI256
	ProfileTrueColor
)

func (p Profile) String() string {
	switch p {
	case ProfileTrueColor:
		return "truecolor"
	case ProfileANSI256:
		return "256 colors"
	case ProfileANSI16:
		return "16 colors"
	default:
		return "monochrome"
	}
}

func DetectProfile(getenv func(string) string, monochrome bool) Profile {
	if monochrome || getenv("NO_COLOR") != "" {
		return ProfileMono
	}
	term := strings.ToLower(getenv("TERM"))
	if term == "dumb" {
		return ProfileMono
	}
	colorTerm := strings.ToLower(getenv("COLORTERM"))
	if colorTerm == "truecolor" || colorTerm == "24bit" || getenv("WT_SESSION") != "" {
		return ProfileTrueColor
	}
	if strings.Contains(term, "256color") {
		return ProfileANSI256
	}
	return ProfileANSI16
}

type Style struct {
	Foreground string
	Background string
	Bold       bool
	Underline  bool
	Reverse    bool
}

type Terminal struct {
	Profile   Profile
	Text      Style
	Muted     Style
	Accent    Style
	Heading   Style
	Selected  Style
	Border    Style
	StatusBar Style
	Success   Style
	Warning   Style
	Error     Style
	severity  map[Severity]Style
}

func (t Terminal) Severity(level Severity) Style {
	if style, ok := t.severity[level]; ok {
		return style
	}
	return t.severity[SeverityInfo]
}

var ansiAccent = map[string]string{"slate": "7", "blue": "12", "green": "10", "crimson": "9", "amber": "11", "purple": "13", "contrast": "15"}

var ansiAccentLight = map[string]string{"slate": "8", "blue": "4", "green": "2", "crimson": "1", "amber": "3", "purple": "5", "contrast": "0"}

func ForTerminal(tokens Tokens, profile Profile) Terminal {
	switch profile {
	case ProfileMono:
		return Terminal{
			Profile:   profile,
			Accent:    Style{Bold: true},
			Heading:   Style{Bold: true, Underline: true},
			Selected:  Style{Reverse: true},
			StatusBar: Style{Reverse: true},
			Success:   Style{},
			Warning:   Style{Bold: true},
			Error:     Style{Bold: true, Underline: true},
			severity: map[Severity]Style{
				SeverityInfo:     {},
				SeverityLow:      {},
				SeverityMedium:   {Bold: true},
				SeverityHigh:     {Bold: true},
				SeverityCritical: {Bold: true, Underline: true},
			},
		}
	case ProfileANSI16:
		accent := ansiAccent[tokens.Name]
		critical, high, medium, low, muted := "9", "11", "3", "12", "8"
		success, selectedForeground := "10", "0"
		if !tokens.Dark {
			accent = ansiAccentLight[tokens.Name]
			critical, high, medium, low = "1", "5", "3", "4"
			success, selectedForeground = "2", "15"
		}
		if accent == "" {
			accent = "7"
		}
		return Terminal{
			Profile:   profile,
			Muted:     Style{Foreground: muted},
			Accent:    Style{Foreground: accent, Bold: true},
			Heading:   Style{Foreground: accent, Bold: true},
			Selected:  Style{Foreground: selectedForeground, Background: accent, Bold: true},
			Border:    Style{Foreground: muted},
			StatusBar: Style{Reverse: true},
			Success:   Style{Foreground: success},
			Warning:   Style{Foreground: medium, Bold: true},
			Error:     Style{Foreground: critical, Bold: true},
			severity: map[Severity]Style{
				SeverityInfo:     {},
				SeverityLow:      {Foreground: low},
				SeverityMedium:   {Foreground: medium},
				SeverityHigh:     {Foreground: high, Bold: true},
				SeverityCritical: {Foreground: critical, Bold: true},
			},
		}
	}
	p := tokens.Palette
	return Terminal{
		Profile:   profile,
		Text:      Style{Foreground: p.Text.Hex()},
		Muted:     Style{Foreground: p.TextMuted.Hex()},
		Accent:    Style{Foreground: p.Accent.Hex(), Bold: true},
		Heading:   Style{Foreground: p.Accent.Hex(), Bold: true},
		Selected:  Style{Foreground: tokens.OnAccent.Hex(), Background: p.Accent.Hex(), Bold: true},
		Border:    Style{Foreground: p.Border.Hex()},
		StatusBar: Style{Foreground: p.Text.Hex(), Background: p.SurfaceAlt.Hex()},
		Success:   Style{Foreground: tokens.Semantic.Success.Hex()},
		Warning:   Style{Foreground: tokens.Semantic.Warning.Hex(), Bold: true},
		Error:     Style{Foreground: tokens.Semantic.Error.Hex(), Bold: true},
		severity: map[Severity]Style{
			SeverityInfo:     {Foreground: tokens.Semantic.Info.Hex()},
			SeverityLow:      {Foreground: tokens.Semantic.Low.Hex()},
			SeverityMedium:   {Foreground: tokens.Semantic.Medium.Hex()},
			SeverityHigh:     {Foreground: tokens.Semantic.High.Hex(), Bold: true},
			SeverityCritical: {Foreground: tokens.Semantic.Critical.Hex(), Bold: true},
		},
	}
}
