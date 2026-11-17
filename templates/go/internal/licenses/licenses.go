package licenses

import (
	"os"
	"path/filepath"
	"regexp"
	"sort"
	"strings"
)

type Module struct {
	Path    string
	Version string
	Dir     string
}

type Notice struct {
	Module  Module
	License string
	Files   []File
}

type File struct {
	Name string
	Text string
}

var Allowed = map[string]bool{
	"MIT":          true,
	"BSD-2-Clause": true,
	"BSD-3-Clause": true,
	"Apache-2.0":   true,
	"ISC":          true,
	"Zlib":         true,
	"OFL-1.1":      true,
	"CC0-1.0":      true,
	"Unlicense":    true,
	"MPL-2.0":      true,
	"blessing":     true,
}

func normalize(text string) string {
	return strings.Join(strings.Fields(strings.ToLower(text)), " ")
}

func Detect(text string) string {
	flat := normalize(text)
	has := func(parts ...string) bool {
		for _, part := range parts {
			if !strings.Contains(flat, part) {
				return false
			}
		}
		return true
	}
	switch {
	case has("apache license", "version 2.0"):
		return "Apache-2.0"
	case has("mozilla public license", "2.0"):
		return "MPL-2.0"
	case has("sil open font license", "1.1"):
		return "OFL-1.1"
	case has("gnu general public license") || has("gnu lesser general public license") || has("gnu affero general public license"):
		return "GPL-family"
	case has("cc0 1.0 universal"):
		return "CC0-1.0"
	case has("this is free and unencumbered software released into the public domain"):
		return "Unlicense"
	case has("permission is hereby granted, free of charge", "the software is provided \"as is\""):
		return "MIT"
	case has("permission to use, copy, modify, and/or distribute this software for any purpose with or without fee") || has("permission to use, copy, modify, and distribute this software for any purpose with or without fee"):
		return "ISC"
	case has("this software is provided 'as-is'", "altered source versions must be plainly marked"):
		return "Zlib"
	case has("redistribution and use in source and binary forms", "neither the name") || has("redistribution and use in source and binary forms", "may not be used to endorse or promote"):
		return "BSD-3-Clause"
	case has("redistribution and use in source and binary forms"):
		return "BSD-2-Clause"
	case has("the author disclaims copyright to this source code", "in place of a legal notice, here is a blessing"):
		return "blessing"
	}
	return "unknown"
}

func isLicenseFile(name string) bool {
	upper := strings.ToUpper(name)
	for _, prefix := range []string{"LICENSE", "LICENCE", "COPYING", "NOTICE", "UNLICENSE"} {
		if strings.HasPrefix(upper, prefix) {
			return true
		}
	}
	return false
}

func Collect(module Module, nested ...string) (Notice, error) {
	notice := Notice{Module: module}
	directories := append([]string{""}, nested...)
	for _, directory := range directories {
		entries, err := os.ReadDir(filepath.Join(module.Dir, filepath.FromSlash(directory)))
		if err != nil {
			if directory == "" {
				return notice, err
			}
			continue
		}
		for _, entry := range entries {
			if entry.IsDir() || !isLicenseFile(entry.Name()) {
				continue
			}
			content, err := os.ReadFile(filepath.Join(module.Dir, filepath.FromSlash(directory), entry.Name()))
			if err != nil {
				return notice, err
			}
			name := entry.Name()
			if directory != "" {
				name = directory + "/" + name
			}
			notice.Files = append(notice.Files, File{Name: name, Text: strings.ReplaceAll(string(content), "\r\n", "\n")})
		}
	}
	sort.Slice(notice.Files, func(i, j int) bool { return notice.Files[i].Name < notice.Files[j].Name })
	found := map[string]bool{}
	var names []string
	for _, file := range notice.Files {
		if strings.HasPrefix(strings.ToUpper(filepath.Base(file.Name)), "NOTICE") {
			continue
		}
		if license := Detect(file.Text); !found[license] {
			found[license] = true
			names = append(names, license)
		}
	}
	if len(names) > 1 {
		kept := names[:0]
		for _, name := range names {
			if name != "unknown" {
				kept = append(kept, name)
			}
		}
		names = kept
	}
	if len(names) == 0 {
		names = []string{"unknown"}
	}
	notice.License = strings.Join(names, " AND ")
	return notice, nil
}

func (n Notice) Problems() []string {
	var problems []string
	if len(n.Files) == 0 {
		return []string{n.Module.Path + " has no license file"}
	}
	for _, name := range strings.Split(n.License, " AND ") {
		if !Allowed[name] {
			problems = append(problems, n.Module.Path+" uses "+name+", which is not on the allowed list")
		}
	}
	return problems
}

var address = regexp.MustCompile(`<?[A-Za-z0-9._%+-]+@[A-Za-z0-9-]+(\.[A-Za-z0-9-]+)*\.[A-Za-z]{2,}>?`)

const AddressOmitted = "(address omitted)"

func Redact(text string) string {
	return address.ReplaceAllString(text, AddressOmitted)
}

func fence(text string) string {
	fence := "```"
	for strings.Contains(text, fence) {
		fence += "`"
	}
	return fence + "text\n" + strings.TrimRight(text, "\n") + "\n" + fence + "\n"
}

func Render(appName, appLicense, holder string, notices []Notice, extra string) string {
	sort.Slice(notices, func(i, j int) bool { return notices[i].Module.Path < notices[j].Module.Path })
	var builder strings.Builder
	builder.WriteString("# Third-party notices\n\n")
	builder.WriteString(appName + " is published by " + holder + " under the " + appLicense + " license (see `LICENSE`). The release binaries also contain the third-party components listed here. Each component keeps its own license; none of them is relicensed.\n\n")
	builder.WriteString("This file is generated by `make notices` from the module versions in `go.sum` and the license files those modules ship. Do not edit it by hand. E-mail addresses in upstream copyright lines are replaced by \"" + AddressOmitted + "\" because this repository contains no e-mail addresses; the unmodified files ship with each module at the listed version.\n\n")
	builder.WriteString(extra)
	builder.WriteString("## Go modules\n\n| Module | Version | License |\n|---|---|---|\n")
	for _, notice := range notices {
		builder.WriteString("| `" + notice.Module.Path + "` | " + notice.Module.Version + " | " + notice.License + " |\n")
	}
	builder.WriteString("\n## License texts\n")
	for _, notice := range notices {
		builder.WriteString("\n### " + notice.Module.Path + " " + notice.Module.Version + "\n")
		for _, file := range notice.Files {
			builder.WriteString("\n`" + file.Name + "`\n\n" + fence(Redact(file.Text)))
		}
	}
	return builder.String()
}
