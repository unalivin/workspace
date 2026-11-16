package guide

import (
	"strings"
	"unicode"
)

type Section struct {
	Level  int
	Title  string
	Anchor string
	Body   string
}

type Guide struct {
	Sections []Section
}

type Hit struct {
	Section int
	Title   string
	Excerpt string
}

func anchor(title string, taken map[string]int) string {
	var builder strings.Builder
	for _, r := range strings.ToLower(title) {
		switch {
		case unicode.IsLetter(r) || unicode.IsDigit(r):
			builder.WriteRune(r)
		case r == ' ' || r == '-' || r == '_':
			builder.WriteRune('-')
		}
	}
	base := strings.Trim(builder.String(), "-")
	if base == "" {
		base = "section"
	}
	candidate := base
	count := taken[base]
	for {
		if _, used := taken[candidate]; !used {
			break
		}
		count++
		candidate = base + "-" + strings.Repeat("i", count)
	}
	taken[base] = count
	if _, known := taken[candidate]; !known {
		taken[candidate] = 0
	}
	return candidate
}

func heading(line string) (int, string, bool) {
	level := 0
	for level < len(line) && line[level] == '#' {
		level++
	}
	if level == 0 || level > 6 || level >= len(line) || line[level] != ' ' {
		return 0, "", false
	}
	title := strings.TrimSpace(strings.TrimRight(strings.TrimSpace(line[level:]), "#"))
	if title == "" {
		return 0, "", false
	}
	return level, title, true
}

func Parse(markdown string) Guide {
	var guide Guide
	taken := map[string]int{}
	var body []string
	current := Section{Level: 1, Title: "", Anchor: ""}
	started, fenced := false, false
	flush := func() {
		current.Body = strings.TrimSpace(strings.Join(body, "\n"))
		if started || current.Body != "" {
			if !started {
				current.Title, current.Anchor = "Introduction", anchor("Introduction", taken)
			}
			guide.Sections = append(guide.Sections, current)
		}
		body = nil
	}
	for _, line := range strings.Split(strings.ReplaceAll(markdown, "\r\n", "\n"), "\n") {
		if strings.HasPrefix(strings.TrimSpace(line), "```") {
			fenced = !fenced
		}
		if level, title, ok := heading(line); ok && !fenced {
			flush()
			current = Section{Level: level, Title: title, Anchor: anchor(title, taken)}
			started = true
			continue
		}
		body = append(body, line)
	}
	flush()
	return guide
}

func (g Guide) Contents() []Section {
	contents := make([]Section, 0, len(g.Sections))
	for _, section := range g.Sections {
		contents = append(contents, Section{Level: section.Level, Title: section.Title, Anchor: section.Anchor})
	}
	return contents
}

func (g Guide) Find(anchor string) (int, bool) {
	for index, section := range g.Sections {
		if section.Anchor == anchor {
			return index, true
		}
	}
	return 0, false
}

func excerpt(text string, position, length int) string {
	runes := []rune(text)
	start := len([]rune(text[:position])) - 40
	if start < 0 {
		start = 0
	}
	end := start + 80 + length
	if end > len(runes) {
		end = len(runes)
	}
	out := strings.Join(strings.Fields(string(runes[start:end])), " ")
	if start > 0 {
		out = "…" + out
	}
	if end < len(runes) {
		out += "…"
	}
	return out
}

func (g Guide) Search(query string) []Hit {
	terms := strings.Fields(strings.ToLower(query))
	if len(terms) == 0 {
		return nil
	}
	var hits []Hit
	for index, section := range g.Sections {
		haystack := strings.ToLower(section.Title + "\n" + section.Body)
		matched := true
		for _, term := range terms {
			matched = matched && strings.Contains(haystack, term)
		}
		if !matched {
			continue
		}
		hit := Hit{Section: index, Title: section.Title, Excerpt: section.Title}
		lowered := strings.ToLower(section.Body)
		if position := strings.Index(lowered, terms[0]); position >= 0 && len(lowered) == len(section.Body) {
			hit.Excerpt = excerpt(section.Body, position, len(terms[0]))
		}
		hits = append(hits, hit)
	}
	return hits
}

func Plain(markdown string) string {
	var lines []string
	fenced := false
	for _, line := range strings.Split(markdown, "\n") {
		trimmed := strings.TrimSpace(line)
		if strings.HasPrefix(trimmed, "```") {
			fenced = !fenced
			continue
		}
		if fenced {
			lines = append(lines, "    "+line)
			continue
		}
		if strings.HasPrefix(trimmed, "|") && strings.Trim(trimmed, "|-: ") == "" {
			continue
		}
		if strings.HasPrefix(trimmed, "|") {
			cells := strings.Split(strings.Trim(trimmed, "|"), "|")
			for index := range cells {
				cells[index] = strings.TrimSpace(cells[index])
			}
			line = strings.Join(cells, "  ·  ")
		}
		line = strings.NewReplacer("**", "", "`", "").Replace(line)
		lines = append(lines, line)
	}
	return strings.TrimSpace(strings.Join(lines, "\n"))
}
