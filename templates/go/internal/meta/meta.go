package meta

import (
	"net/url"
	"runtime"
	"runtime/debug"
	"strings"
)

const (
	AppName       = "Template App"
	Slug          = "templateapp"
	AppID         = "io.github.unalivin.templateapp"
	EnvPrefix     = "TEMPLATEAPP"
	Version       = "0.1.0-dev"
	Tagline       = "TEMPLATE_TAGLINE"
	Description   = "TEMPLATE_DESCRIPTION"
	IconConcept   = "TEMPLATE_ICON_CONCEPT"
	RepositoryURL = "https://github.com/unalivin/workspace"
	License       = "MIT"
)

type BuildInfo struct {
	Version   string
	GoVersion string
	OS        string
	Arch      string
	Revision  string
	Modified  bool
	GUI       bool
}

func Build(gui bool) BuildInfo {
	info := BuildInfo{Version: Version, GoVersion: runtime.Version(), OS: runtime.GOOS, Arch: runtime.GOARCH, GUI: gui}
	if embedded, ok := debug.ReadBuildInfo(); ok {
		for _, setting := range embedded.Settings {
			switch setting.Key {
			case "vcs.revision":
				info.Revision = setting.Value
			case "vcs.modified":
				info.Modified = setting.Value == "true"
			}
		}
	}
	return info
}

func EnvName(suffix string) string {
	return EnvPrefix + "_" + strings.ToUpper(strings.NewReplacer(".", "_", "-", "_").Replace(suffix))
}

func SourceURL() string {
	return RepositoryURL
}

func AdvisoryURL() string {
	return RepositoryURL + "/security/advisories/new"
}

func NewIssueURL(title, body string) string {
	query := url.Values{}
	query.Set("title", title)
	query.Set("body", body)
	return RepositoryURL + "/issues/new?" + query.Encode()
}

func IsRepositoryURL(raw string) bool {
	candidate, err := url.Parse(raw)
	if err != nil {
		return false
	}
	base, err := url.Parse(RepositoryURL)
	if err != nil {
		return false
	}
	if candidate.Scheme != "https" || candidate.User != nil || !strings.EqualFold(candidate.Host, base.Host) {
		return false
	}
	decoded := candidate.Path
	if strings.Contains(decoded, "..") || strings.Contains(decoded, "//") || strings.Contains(decoded, "\\") {
		return false
	}
	if strings.Contains(strings.ToLower(candidate.EscapedPath()), "%2f") {
		return false
	}
	return decoded == base.Path || strings.HasPrefix(decoded, base.Path+"/")
}
