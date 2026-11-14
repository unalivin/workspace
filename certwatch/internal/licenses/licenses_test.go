package licenses

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestDetect(t *testing.T) {
	cases := map[string]string{
		"Permission is hereby granted, free of charge, to any person obtaining a copy.\nTHE SOFTWARE IS PROVIDED \"AS IS\", WITHOUT WARRANTY": "MIT",
		"Apache License\nVersion 2.0, January 2004": "Apache-2.0",
		"Redistribution and use in source and binary forms, with or without modification.\nNeither the name of the copyright holder nor the names of its contributors": "BSD-3-Clause",
		"Redistribution and use in source and binary forms, with or without modification, are permitted":                                                               "BSD-2-Clause",
		"Permission to use, copy, modify, and/or distribute this software for any purpose with or without fee is hereby granted":                                       "ISC",
		"This software is provided 'as-is', without any express or implied warranty.\nAltered source versions must be plainly marked as such":                          "Zlib",
		"SIL OPEN FONT LICENSE Version 1.1":    "OFL-1.1",
		"GNU GENERAL PUBLIC LICENSE Version 3": "GPL-family",
		"Mozilla Public License Version 2.0":   "MPL-2.0",
		"":                                     "unknown",
		"all rights reserved, do not copy":     "unknown",
	}
	for text, want := range cases {
		if got := Detect(text); got != want {
			t.Errorf("%q: got %s, want %s", text, got, want)
		}
	}
}

func TestCollectAndProblems(t *testing.T) {
	directory := t.TempDir()
	write := func(name, text string) {
		t.Helper()
		if err := os.MkdirAll(filepath.Dir(filepath.Join(directory, name)), 0o755); err != nil {
			t.Fatal(err)
		}
		if err := os.WriteFile(filepath.Join(directory, name), []byte(text), 0o644); err != nil {
			t.Fatal(err)
		}
	}
	module := Module{Path: "example.test/module", Version: "v1.2.3", Dir: directory}
	empty, err := Collect(module)
	if err != nil {
		t.Fatal(err)
	}
	if problems := empty.Problems(); len(problems) != 1 || !strings.Contains(problems[0], "no license file") {
		t.Fatalf("problems %v", problems)
	}
	write("LICENSE", "Permission is hereby granted, free of charge.\r\nTHE SOFTWARE IS PROVIDED \"AS IS\"")
	write("fonts/LICENSE_Font.txt", "SIL Open Font License, Version 1.1")
	write("NOTICE", "Portions copyright someone")
	write("main.go", "package main")
	notice, err := Collect(module, "fonts", "missing")
	if err != nil {
		t.Fatal(err)
	}
	if notice.License != "MIT AND OFL-1.1" || len(notice.Files) != 3 || len(notice.Problems()) != 0 {
		t.Fatalf("notice %+v", notice)
	}
	if strings.Contains(notice.Files[0].Text, "\r") {
		t.Fatal("line endings were not normalized")
	}
	write("COPYING", "GNU GENERAL PUBLIC LICENSE")
	copyleft, err := Collect(module)
	if err != nil {
		t.Fatal(err)
	}
	if problems := copyleft.Problems(); len(problems) != 1 || !strings.Contains(problems[0], "GPL-family") {
		t.Fatalf("problems %v", problems)
	}
	if _, err := Collect(Module{Path: "gone", Dir: filepath.Join(directory, "absent")}); err == nil {
		t.Fatal("a missing module directory was accepted")
	}
}

func TestRenderListsEveryModuleOnceAndKeepsFencesIntact(t *testing.T) {
	notices := []Notice{
		{Module: Module{Path: "b.test/two", Version: "v2.0.0"}, License: "MIT", Files: []File{{Name: "LICENSE", Text: "text with ``` inside\n"}}},
		{Module: Module{Path: "a.test/one", Version: "v1.0.0"}, License: "ISC", Files: []File{{Name: "LICENSE", Text: "plain"}}},
	}
	rendered := Render("App", "MIT", notices, "## Extra\n\nnote\n\n")
	first, second := strings.Index(rendered, "| `a.test/one` | v1.0.0 | ISC |"), strings.Index(rendered, "| `b.test/two` | v2.0.0 | MIT |")
	if first < 0 || second < first {
		t.Fatalf("table order:\n%s", rendered)
	}
	if !strings.Contains(rendered, "````text\ntext with ``` inside\n````\n") || !strings.Contains(rendered, "## Extra") {
		t.Fatalf("rendering:\n%s", rendered)
	}
	redacted := Render("App", "MIT", []Notice{{Module: Module{Path: "c.test/three", Version: "v3.0.0"}, License: "MIT", Files: []File{{Name: "LICENSE", Text: "Copyright (c) Some Person <" + "person" + "@" + "example.test" + "> and other" + "@" + "example.test"}}}}, "")
	if strings.Contains(redacted, "@"+"example") || strings.Count(redacted, AddressOmitted) != 3 || !strings.Contains(redacted, "Copyright (c) Some Person "+AddressOmitted+" and "+AddressOmitted) {
		t.Fatalf("addresses were not replaced:\n%s", redacted)
	}
	if strings.Count(rendered, "### a.test/one v1.0.0") != 1 {
		t.Fatal("a module is repeated")
	}
}
