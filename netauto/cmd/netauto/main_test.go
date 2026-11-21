package main

import "testing"

func TestDecideLaunchMode(t *testing.T) {
	cases := []struct {
		name        string
		uiFlag      string
		envUI       string
		hasDisplay  bool
		interactive bool
		ask         bool
		noPrompt    bool
		want        string
	}{
		{"flag gui", "gui", "", true, true, false, false, "gui"},
		{"flag tui", "tui", "", true, true, false, false, "tui"},
		{"env tui", "", "tui", true, true, false, false, "tui"},
		{"headless", "", "", false, false, false, false, "tui"},
		{"ask", "", "", true, true, true, false, "prompt"},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			got := decideLaunchMode(tc.uiFlag, tc.envUI, tc.hasDisplay, tc.interactive, tc.ask, tc.noPrompt)
			if got != tc.want {
				t.Fatalf("got %q, want %q", got, tc.want)
			}
		})
	}
}
