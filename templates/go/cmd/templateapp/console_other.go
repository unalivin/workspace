//go:build !windows

package main

func consoleOwners() int { return 0 }

func prepareConsole([]string) bool { return false }

func setConsoleVisible(bool) {}
