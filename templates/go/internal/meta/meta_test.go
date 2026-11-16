package meta

import (
	"net/url"
	"strings"
	"testing"
)

func TestIsRepositoryURL(t *testing.T) {
	cases := []struct {
		name string
		raw  string
		want bool
	}{
		{"repository root", RepositoryURL, true},
		{"issues page", RepositoryURL + "/issues/new?title=a", true},
		{"advisory form", AdvisoryURL(), true},
		{"other repository", "https://github.com/unalivin/other", false},
		{"prefix trick", RepositoryURL + "-evil/issues", false},
		{"plain http", strings.Replace(RepositoryURL, "https://", "http://", 1), false},
		{"other host", "https://example.test/unalivin/workspace", false},
		{"user info", strings.Replace(RepositoryURL, "https://", "https://user:secret"+string(rune(64)), 1), false},
		{"path traversal", RepositoryURL + "/../../other/repo", false},
		{"encoded traversal", RepositoryURL + "/%2e%2e/other", false},
		{"encoded slash", RepositoryURL + "/..%2f..%2fother", false},
		{"backslash", RepositoryURL + "/\\..\\other", false},
		{"double slash", RepositoryURL + "//issues", false},
		{"javascript scheme", "javascript:alert(1)", false},
		{"file scheme", "file:///etc/passwd", false},
		{"empty", "", false},
		{"not a url", "://", false},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			if got := IsRepositoryURL(tc.raw); got != tc.want {
				t.Fatalf("IsRepositoryURL(%q) = %v, want %v", tc.raw, got, tc.want)
			}
		})
	}
}

func TestNewIssueURLEncodesFields(t *testing.T) {
	raw := NewIssueURL("Crash & burn", "line one\nline two #3")
	if !IsRepositoryURL(raw) {
		t.Fatalf("issue URL is outside the repository: %s", raw)
	}
	parsed, err := url.Parse(raw)
	if err != nil {
		t.Fatal(err)
	}
	if parsed.Query().Get("title") != "Crash & burn" || parsed.Query().Get("body") != "line one\nline two #3" {
		t.Fatalf("fields were not preserved: %v", parsed.Query())
	}
}

func TestEnvName(t *testing.T) {
	if got := EnvName("ui"); got != EnvPrefix+"_UI" {
		t.Fatalf("got %s", got)
	}
	if got := EnvName("appearance.text-scale"); got != EnvPrefix+"_APPEARANCE_TEXT_SCALE" {
		t.Fatalf("got %s", got)
	}
}

func TestIdentity(t *testing.T) {
	if Version != "0.1.0-dev" {
		t.Fatalf("unexpected version %s", Version)
	}
	if strings.ContainsAny(EnvPrefix, "-. ") || EnvPrefix != strings.ToUpper(EnvPrefix) {
		t.Fatalf("environment prefix %q is not a valid variable prefix", EnvPrefix)
	}
	build := Build(true)
	if build.OS == "" || build.Arch == "" || build.GoVersion == "" || !build.GUI {
		t.Fatalf("incomplete build info: %+v", build)
	}
}
