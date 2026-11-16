package guide

import (
	"strings"
	"testing"
	"unicode/utf8"
)

const sample = "Lead paragraph before any heading.\n\n# Usage guide\n\nWelcome text.\n\n## First steps\n\nOpen the Records view and add a record.\n\n```\n# not a heading\ncommand --flag\n```\n\n## Themes\n\nChoose **High contrast** in `Settings`.\n\n| Key | Action |\n|-----|--------|\n| F1 | Open this guide |\n\n### Themes\n\nRepeated title.\n\n#NoSpace is text\n####### seven is text\n"

func TestParseBuildsSectionsAndUniqueAnchors(t *testing.T) {
	parsed := Parse(sample)
	var titles, anchors []string
	for _, section := range parsed.Sections {
		titles = append(titles, section.Title)
		anchors = append(anchors, section.Anchor)
	}
	if got := strings.Join(titles, "|"); got != "Introduction|Usage guide|First steps|Themes|Themes" {
		t.Fatalf("titles: %s", got)
	}
	if got := strings.Join(anchors, "|"); got != "introduction|usage-guide|first-steps|themes|themes-i" {
		t.Fatalf("anchors: %s", got)
	}
	if !strings.Contains(parsed.Sections[2].Body, "# not a heading") {
		t.Fatal("a fenced line was treated as a heading")
	}
	if !strings.Contains(parsed.Sections[4].Body, "#NoSpace is text") || !strings.Contains(parsed.Sections[4].Body, "####### seven") {
		t.Fatal("malformed headings must stay in the body")
	}
	if parsed.Sections[3].Level != 2 || parsed.Sections[4].Level != 3 {
		t.Fatal("levels are wrong")
	}
	contents := parsed.Contents()
	if len(contents) != 5 || contents[1].Body != "" {
		t.Fatalf("contents: %+v", contents)
	}
	if index, ok := parsed.Find("themes-i"); !ok || index != 4 {
		t.Fatalf("find: %d, %v", index, ok)
	}
	if _, ok := parsed.Find("missing"); ok {
		t.Fatal("a missing anchor was found")
	}
}

func TestParseEdgeCases(t *testing.T) {
	if got := Parse(""); len(got.Sections) != 0 {
		t.Fatalf("empty input: %+v", got)
	}
	if got := Parse("just text"); len(got.Sections) != 1 || got.Sections[0].Title != "Introduction" {
		t.Fatalf("text only: %+v", got)
	}
	if got := Parse("# Title\r\n\r\nBody\r\n"); len(got.Sections) != 1 || got.Sections[0].Body != "Body" {
		t.Fatalf("CRLF: %+v", got)
	}
	if got := Parse("# !!!\n# ???\n"); got.Sections[0].Anchor != "section" || got.Sections[1].Anchor != "section-i" {
		t.Fatalf("symbol-only titles: %+v", got)
	}
	var clashing []string
	for _, section := range Parse("# Step\n# Step\n# Step i\n# Step\n# Step-i\n").Sections {
		clashing = append(clashing, section.Anchor)
	}
	if got := strings.Join(clashing, "|"); got != "step|step-i|step-i-i|step-ii|step-i-ii" {
		t.Fatalf("anchors of clashing titles: %s", got)
	}
	if got := Parse("## Trailing ##\n"); got.Sections[0].Title != "Trailing" {
		t.Fatalf("closing hashes: %+v", got)
	}
}

func TestSearch(t *testing.T) {
	parsed := Parse(sample)
	if hits := parsed.Search(""); hits != nil {
		t.Fatal("an empty query returned hits")
	}
	hits := parsed.Search("RECORD")
	if len(hits) != 1 || hits[0].Title != "First steps" || !strings.Contains(hits[0].Excerpt, "add a record") {
		t.Fatalf("hits: %+v", hits)
	}
	if hits := parsed.Search("high settings"); len(hits) != 1 || hits[0].Section != 3 {
		t.Fatalf("every term must match: %+v", hits)
	}
	if hits := parsed.Search("themes"); len(hits) != 2 {
		t.Fatalf("title matches: %+v", hits)
	}
	if hits := parsed.Search("absent"); len(hits) != 0 {
		t.Fatalf("no match expected: %+v", hits)
	}
	long := Parse("# Long\n\n" + strings.Repeat("word ", 100) + "needle " + strings.Repeat("tail ", 100))
	hit := long.Search("needle")
	if len(hit) != 1 || !strings.HasPrefix(hit[0].Excerpt, "…") || !strings.HasSuffix(hit[0].Excerpt, "…") || len(hit[0].Excerpt) > 200 {
		t.Fatalf("excerpt: %+v", hit)
	}
}

func TestPlain(t *testing.T) {
	plain := Plain("Choose **High contrast** in `Settings`.\n\n| Key | Action |\n|-----|--------|\n| F1 | Open this guide |\n\n```\ncommand --flag\n```\n")
	want := "Choose High contrast in Settings.\n\nKey  ·  Action\nF1  ·  Open this guide\n\n    command --flag"
	if plain != want {
		t.Fatalf("got:\n%s\nwant:\n%s", plain, want)
	}
}

func FuzzParseAndSearch(f *testing.F) {
	for _, seed := range []string{sample, "", "#", "# \n", "```\n# x", "\xff# bad\n## é\n", strings.Repeat("#", 10) + " x"} {
		f.Add(seed, "the")
	}
	f.Fuzz(func(t *testing.T, markdown, query string) {
		parsed := Parse(markdown)
		seen := map[string]bool{}
		for _, section := range parsed.Sections {
			if section.Anchor == "" || seen[section.Anchor] {
				t.Fatalf("anchor %q is empty or repeated", section.Anchor)
			}
			seen[section.Anchor] = true
			if section.Level < 1 || section.Level > 6 || section.Title == "" {
				t.Fatalf("bad section %+v", section)
			}
		}
		for _, hit := range parsed.Search(query) {
			if hit.Section < 0 || hit.Section >= len(parsed.Sections) {
				t.Fatalf("hit outside the guide: %+v", hit)
			}
			if utf8.ValidString(markdown) && !utf8.ValidString(hit.Excerpt) {
				t.Fatalf("excerpt broke a character: %q", hit.Excerpt)
			}
		}
		_ = Plain(markdown)
	})
}
