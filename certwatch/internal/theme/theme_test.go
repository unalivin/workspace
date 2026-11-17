package theme

import (
	"fmt"
	"math"
	"strings"
	"testing"
)

func allVariants() []Tokens {
	var variants []Tokens
	for _, name := range Names() {
		for _, mode := range []Mode{ModeLight, ModeDark} {
			variants = append(variants, Resolve(Options{Name: name, Mode: mode, TextScale: 100}))
		}
	}
	return variants
}

func TestThereAreSevenNamedThemesIncludingHighContrast(t *testing.T) {
	want := []string{"slate", "blue", "green", "crimson", "amber", "purple", "contrast"}
	if fmt.Sprint(Names()) != fmt.Sprint(want) {
		t.Fatalf("themes = %v, want %v", Names(), want)
	}
	accents, surfaces := map[Color]bool{}, map[Color]bool{}
	for _, name := range Names() {
		if Label(name) == "" {
			t.Fatalf("theme %s has no label", name)
		}
		tokens := Resolve(Options{Name: name, Mode: ModeLight})
		accents[tokens.Palette.Accent] = true
		surfaces[tokens.Palette.Background] = true
	}
	if len(accents) != 7 || len(surfaces) != 7 {
		t.Fatalf("themes must differ in accent and surface palette: %d accents, %d backgrounds", len(accents), len(surfaces))
	}
}

func TestEveryThemeMeetsContrastRules(t *testing.T) {
	for _, tokens := range allVariants() {
		name := fmt.Sprintf("%s dark=%v", tokens.Name, tokens.Dark)
		t.Run(name, func(t *testing.T) {
			p := tokens.Palette
			surfaces := map[string]Color{"background": p.Background, "surface": p.Surface, "alternate surface": p.SurfaceAlt, "selection": tokens.Selection}
			check := func(label string, foreground Color, minimum float64) {
				for surfaceName, surface := range surfaces {
					if ratio := Contrast(foreground, surface); ratio < minimum {
						t.Errorf("%s %s on %s %s has contrast %.2f, need %.1f", label, foreground.Hex(), surfaceName, surface.Hex(), ratio, minimum)
					}
				}
			}
			check("text", p.Text, MinTextContrast)
			check("muted text", p.TextMuted, MinTextContrast)
			check("accent", p.Accent, MinTextContrast)
			check("border", p.Border, MinComponentContrast)
			check("focus ring", tokens.Focus, MinComponentContrast)
			for _, level := range Severities {
				check("severity "+string(level), tokens.Semantic.Severity(level), MinTextContrast)
			}
			check("success", tokens.Semantic.Success, MinTextContrast)
			check("warning", tokens.Semantic.Warning, MinTextContrast)
			check("error", tokens.Semantic.Error, MinTextContrast)
			if ratio := Contrast(tokens.OnAccent, p.Accent); ratio < MinTextContrast {
				t.Errorf("text on accent has contrast %.2f", ratio)
			}
			if tokens.Name == "contrast" {
				if ratio := Contrast(p.Text, p.Background); ratio < 15 {
					t.Errorf("high contrast theme text contrast is only %.2f", ratio)
				}
			}
		})
	}
}

func TestSeverityColorsIgnoreTheAccentAndStayDistinct(t *testing.T) {
	base := Resolve(Options{Name: "blue", Mode: ModeDark})
	custom := Resolve(Options{Name: "crimson", Mode: ModeDark, Accent: "#00FF88"})
	if base.Semantic != custom.Semantic {
		t.Fatal("severity and status colors changed with the theme or accent")
	}
	for _, tokens := range []Tokens{Resolve(Options{Mode: ModeLight}), Resolve(Options{Mode: ModeDark})} {
		seenIcon, seenGlyph, seenLabel := map[string]bool{}, map[string]bool{}, map[string]bool{}
		for index, level := range Severities {
			seenIcon[level.Icon()], seenGlyph[level.Glyph()], seenLabel[level.Label()] = true, true, true
			for _, other := range Severities[index+1:] {
				for _, deficiency := range []string{"none", "protanopia", "deuteranopia", "tritanopia"} {
					first := simulate(tokens.Semantic.Severity(level), deficiency)
					second := simulate(tokens.Semantic.Severity(other), deficiency)
					if distance := colorDistance(first, second); distance < 24 {
						t.Errorf("dark=%v: severities %s and %s are only %.1f apart under %s", tokens.Dark, level, other, distance, deficiency)
					}
				}
			}
		}
		if len(seenIcon) != 5 || len(seenGlyph) != 5 || len(seenLabel) != 5 {
			t.Fatalf("each severity needs its own icon, glyph and label: %d %d %d", len(seenIcon), len(seenGlyph), len(seenLabel))
		}
	}
	if SeverityCritical.Rank() <= SeverityHigh.Rank() || Severity("bogus").Rank() != -1 || Severity("bogus").Label() != "Unknown" {
		t.Fatal("severity ranking is wrong")
	}
}

func simulate(c Color, deficiency string) [3]float64 {
	linear := [3]float64{channel(c.R), channel(c.G), channel(c.B)}
	matrices := map[string][3][3]float64{
		"none":         {{1, 0, 0}, {0, 1, 0}, {0, 0, 1}},
		"protanopia":   {{0.152286, 1.052583, -0.204868}, {0.114503, 0.786281, 0.099216}, {-0.003882, -0.048116, 1.051998}},
		"deuteranopia": {{0.367322, 0.860646, -0.227968}, {0.280085, 0.672501, 0.047413}, {-0.011820, 0.042940, 0.968881}},
		"tritanopia":   {{1.255528, -0.076749, -0.178779}, {-0.078411, 0.930809, 0.147602}, {0.004733, 0.691367, 0.303900}},
	}
	matrix := matrices[deficiency]
	var out [3]float64
	for row := 0; row < 3; row++ {
		value := matrix[row][0]*linear[0] + matrix[row][1]*linear[1] + matrix[row][2]*linear[2]
		value = math.Max(0, math.Min(1, value))
		if value <= 0.0031308 {
			out[row] = 255 * 12.92 * value
		} else {
			out[row] = 255 * (1.055*math.Pow(value, 1/2.4) - 0.055)
		}
	}
	return out
}

func colorDistance(a, b [3]float64) float64 {
	return math.Sqrt(math.Pow(a[0]-b[0], 2) + math.Pow(a[1]-b[1], 2) + math.Pow(a[2]-b[2], 2))
}

func TestSwitchingThemeChangesTokens(t *testing.T) {
	light := Resolve(Options{Name: "green", Mode: ModeLight})
	dark := Resolve(Options{Name: "green", Mode: ModeDark})
	other := Resolve(Options{Name: "purple", Mode: ModeLight})
	if light.Palette == dark.Palette || light.Palette == other.Palette || light.Dark || !dark.Dark {
		t.Fatal("switching mode or theme did not change the tokens")
	}
	if !Resolve(Options{Name: "green", Mode: ModeSystem, SystemDark: true}).Dark || Resolve(Options{Name: "green", Mode: ModeSystem}).Dark {
		t.Fatal("system mode does not follow the system")
	}
	if Resolve(Options{Name: "green", Mode: ModeLight, SystemDark: true}).Dark {
		t.Fatal("light mode must not follow a dark system")
	}
	fallback := Resolve(Options{Name: "neon", Mode: ModeLight})
	if fallback.Name != "slate" || len(fallback.Warnings) != 1 {
		t.Fatalf("unknown theme must fall back with a warning: %+v", fallback.Warnings)
	}
}

func TestDensityTextScaleAndMotionTokens(t *testing.T) {
	comfortable := Resolve(Options{Density: "comfortable", TextScale: 100})
	compact := Resolve(Options{Density: "compact", TextScale: 100})
	if compact.Spacing.RowHeight >= comfortable.Spacing.RowHeight || compact.Spacing.M >= comfortable.Spacing.M {
		t.Fatal("compact density is not denser")
	}
	large := Resolve(Options{TextScale: 150})
	if large.Type.Body != comfortable.Type.Body*1.5 || large.Type.Heading <= large.Type.Subheading {
		t.Fatalf("text scale not applied: %+v", large.Type)
	}
	if Resolve(Options{TextScale: 5000}).Type.Body != comfortable.Type.Body {
		t.Fatal("out-of-range text scale must fall back to 100 percent")
	}
	if Resolve(Options{ReducedMotion: true}).Motion || !Resolve(Options{}).Motion {
		t.Fatal("reduced motion not reflected")
	}
	contrast := Resolve(Options{Name: "contrast"})
	if contrast.Radius != (Radii{}) || contrast.Elevation != 0 {
		t.Fatal("high contrast theme must use flat, square components")
	}
}

func TestSpacingStaysOnTheFourPixelGridWithOneRowHeight(t *testing.T) {
	for _, density := range []string{"comfortable", "compact"} {
		tokens := Resolve(Options{Density: density, TextScale: 100})
		s := tokens.Spacing
		for name, value := range map[string]float32{"XS": s.XS, "S": s.S, "M": s.M, "L": s.L, "XL": s.XL, "XXL": s.XXL, "RowHeight": s.RowHeight, "RowGap": s.RowGap} {
			if value <= 0 || float32(int(value)) != value || int(value)%4 != 0 {
				t.Errorf("%s %s = %v is not a positive multiple of 4", density, name, value)
			}
		}
		if s.XS > s.S || s.S > s.M || s.M > s.L || s.L > s.XL || s.XL > s.XXL {
			t.Errorf("%s spacing scale is not ascending: %+v", density, s)
		}
		scale := tokens.Type
		if scale.Caption >= scale.Body || scale.Body >= scale.Subheading || scale.Subheading >= scale.Heading || scale.Heading >= scale.Figure {
			t.Errorf("%s type scale is not ascending: %+v", density, scale)
		}
		if tokens.Radius.Chip >= tokens.Radius.Control || tokens.Radius.Control >= tokens.Radius.Card {
			t.Errorf("%s radii are not ascending: %+v", density, tokens.Radius)
		}
	}
}

func TestCustomAccentIsKeptAdjustedOrRejected(t *testing.T) {
	kept := Resolve(Options{Name: "slate", Mode: ModeLight, Accent: "#0B3D91"})
	if kept.Palette.Accent != Hex("#0B3D91") || len(kept.Warnings) != 0 {
		t.Fatalf("a compliant accent must be kept: %s %v", kept.Palette.Accent.Hex(), kept.Warnings)
	}
	for _, mode := range []Mode{ModeLight, ModeDark} {
		for _, accent := range []string{"#FFFF00", "#101010", "#FFFFFF", "#000000", "#808080", "#FF00FF"} {
			tokens := Resolve(Options{Name: "slate", Mode: mode, Accent: accent})
			for _, surface := range []Color{tokens.Palette.Background, tokens.Palette.Surface, tokens.Palette.SurfaceAlt} {
				if ratio := Contrast(tokens.Palette.Accent, surface); ratio < MinTextContrast {
					t.Errorf("accent %s in %s mode ended at %s with contrast %.2f", accent, mode, tokens.Palette.Accent.Hex(), ratio)
				}
			}
			if ratio := Contrast(tokens.OnAccent, tokens.Palette.Accent); ratio < MinTextContrast {
				t.Errorf("text on adjusted accent %s has contrast %.2f", tokens.Palette.Accent.Hex(), ratio)
			}
			if tokens.Palette.Accent != Hex(accent) && (len(tokens.Warnings) != 1 || !strings.Contains(tokens.Warnings[0], "adjusted")) {
				t.Errorf("adjusting %s must produce a warning, got %v", accent, tokens.Warnings)
			}
		}
	}
	rejected := Resolve(Options{Name: "blue", Mode: ModeLight, Accent: "not-a-color"})
	if rejected.Palette.Accent != Resolve(Options{Name: "blue", Mode: ModeLight}).Palette.Accent || len(rejected.Warnings) != 1 {
		t.Fatalf("invalid accent must be ignored with a warning: %v", rejected.Warnings)
	}
}

func TestContrastMath(t *testing.T) {
	white, black := Color{255, 255, 255}, Color{0, 0, 0}
	if ratio := Contrast(white, black); math.Abs(ratio-21) > 0.001 {
		t.Fatalf("white on black = %.3f, want 21", ratio)
	}
	if Contrast(white, black) != Contrast(black, white) || Contrast(white, white) != 1 {
		t.Fatal("contrast must be symmetric and 1 for equal colors")
	}
	if ratio := Contrast(Hex("#767676"), white); ratio < 4.5 || ratio > 4.6 {
		t.Fatalf("known reference #767676 on white = %.3f, want about 4.54", ratio)
	}
	if BestOn(black) != white || BestOn(white) != black {
		t.Fatal("BestOn picked the weaker color")
	}
	if Mix(black, white, 0.5) != (Color{128, 128, 128}) || Mix(black, white, 0) != black || Mix(black, white, 1) != white {
		t.Fatal("Mix is wrong")
	}
	for _, bad := range []string{"", "#12", "#GGGGGG", "1234567", "#12345"} {
		if _, err := ParseHex(bad); err == nil {
			t.Fatalf("ParseHex(%q) should fail", bad)
		}
	}
	if color, err := ParseHex(" a1b2c3 "); err != nil || color.Hex() != "#A1B2C3" {
		t.Fatalf("ParseHex round trip failed: %v %v", color, err)
	}
}

func FuzzAdjustForContrast(f *testing.F) {
	f.Add(uint8(255), uint8(255), uint8(0), true)
	f.Add(uint8(0), uint8(0), uint8(0), false)
	f.Fuzz(func(t *testing.T, r, g, b uint8, dark bool) {
		mode := ModeLight
		if dark {
			mode = ModeDark
		}
		for _, name := range Names() {
			tokens := Resolve(Options{Name: name, Mode: mode, Accent: Color{r, g, b}.Hex()})
			for _, surface := range []Color{tokens.Palette.Background, tokens.Palette.Surface, tokens.Palette.SurfaceAlt} {
				if Contrast(tokens.Palette.Accent, surface) < MinTextContrast {
					t.Fatalf("theme %s: accent %s fails contrast on %s", name, tokens.Palette.Accent.Hex(), surface.Hex())
				}
			}
			if Contrast(tokens.OnAccent, tokens.Palette.Accent) < MinTextContrast {
				t.Fatalf("theme %s: text on accent %s fails contrast", name, tokens.Palette.Accent.Hex())
			}
		}
	})
}

func TestTerminalProfiles(t *testing.T) {
	env := func(values map[string]string) func(string) string {
		return func(key string) string { return values[key] }
	}
	cases := []struct {
		name string
		env  map[string]string
		mono bool
		want Profile
	}{
		{"no color variable", map[string]string{"NO_COLOR": "1", "COLORTERM": "truecolor"}, false, ProfileMono},
		{"monochrome setting", map[string]string{"COLORTERM": "truecolor"}, true, ProfileMono},
		{"dumb terminal", map[string]string{"TERM": "dumb"}, false, ProfileMono},
		{"truecolor", map[string]string{"TERM": "xterm-256color", "COLORTERM": "truecolor"}, false, ProfileTrueColor},
		{"windows terminal", map[string]string{"WT_SESSION": "abc"}, false, ProfileTrueColor},
		{"256 colors", map[string]string{"TERM": "xterm-256color"}, false, ProfileANSI256},
		{"basic terminal", map[string]string{"TERM": "xterm"}, false, ProfileANSI16},
		{"no information", nil, false, ProfileANSI16},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			if got := DetectProfile(env(tc.env), tc.mono); got != tc.want {
				t.Fatalf("profile = %v, want %v", got, tc.want)
			}
		})
	}
}

func TestTerminalStyles(t *testing.T) {
	allowed := map[string]bool{"": true}
	for index := 0; index < 16; index++ {
		allowed[fmt.Sprint(index)] = true
	}
	for _, tokens := range allVariants() {
		mono := ForTerminal(tokens, ProfileMono)
		sixteen := ForTerminal(tokens, ProfileANSI16)
		full := ForTerminal(tokens, ProfileTrueColor)
		styles := func(terminal Terminal) []Style {
			list := []Style{terminal.Text, terminal.Muted, terminal.Accent, terminal.Heading, terminal.Selected, terminal.Border, terminal.StatusBar, terminal.Success, terminal.Warning, terminal.Error}
			for _, level := range Severities {
				list = append(list, terminal.Severity(level))
			}
			return list
		}
		for _, style := range styles(mono) {
			if style.Foreground != "" || style.Background != "" {
				t.Fatalf("%s: monochrome style uses a color: %+v", tokens.Name, style)
			}
		}
		if !mono.Selected.Reverse || !mono.Severity(SeverityCritical).Bold {
			t.Fatal("monochrome mode must mark selection and critical severity with attributes")
		}
		for _, style := range styles(sixteen) {
			if !allowed[style.Foreground] || !allowed[style.Background] {
				t.Fatalf("%s: 16-color style uses a color outside 0-15: %+v", tokens.Name, style)
			}
		}
		if sixteen.Accent.Foreground == "" || sixteen.Selected.Background == "" {
			t.Fatalf("%s: 16-color accent missing", tokens.Name)
		}
		if full.Accent.Foreground != tokens.Palette.Accent.Hex() || full.Severity(SeverityHigh).Foreground != tokens.Semantic.High.Hex() {
			t.Fatalf("%s: truecolor styles do not use the theme tokens", tokens.Name)
		}
		if full.Severity(Severity("bogus")) != full.Severity(SeverityInfo) {
			t.Fatal("unknown severity must fall back to info")
		}
	}
	blue := ForTerminal(Resolve(Options{Name: "blue", Mode: ModeDark}), ProfileANSI16)
	green := ForTerminal(Resolve(Options{Name: "green", Mode: ModeDark}), ProfileANSI16)
	if blue.Accent.Foreground == green.Accent.Foreground {
		t.Fatal("themes must map to different ANSI accents")
	}
	if ProfileMono.String() == "" || ProfileANSI16.String() == ProfileTrueColor.String() || ProfileANSI256.String() == "" {
		t.Fatal("profile names missing")
	}
}
