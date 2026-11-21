package main

import "testing"

func TestDecideLaunchMode(t *testing.T) {
	if decideLaunchMode("tui", "", true, true, false, false) != "tui" {
		t.Fatal("expected tui")
	}
	if decideLaunchMode("", "", false, false, false, false) != "tui" {
		t.Fatal("expected tui for headless")
	}
}
