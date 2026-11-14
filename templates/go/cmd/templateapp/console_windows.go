//go:build windows

package main

import (
	"os"
	"unsafe"

	"golang.org/x/sys/windows"

	"github.com/NexzaDev/templateapp/internal/launcher"
)

const (
	windowHide          = 0
	windowShow          = 5
	attachParentProcess = ^uintptr(0)
)

var (
	kernel32              = windows.NewLazySystemDLL("kernel32.dll")
	user32                = windows.NewLazySystemDLL("user32.dll")
	getConsoleProcessList = kernel32.NewProc("GetConsoleProcessList")
	getConsoleWindow      = kernel32.NewProc("GetConsoleWindow")
	allocConsole          = kernel32.NewProc("AllocConsole")
	attachConsole         = kernel32.NewProc("AttachConsole")
	showWindow            = user32.NewProc("ShowWindow")
)

func consoleOwners() int {
	var processes [8]uint32
	count, _, _ := getConsoleProcessList.Call(uintptr(unsafe.Pointer(&processes[0])), uintptr(len(processes)))
	return int(count)
}

func usable(file *os.File) bool {
	_, err := file.Stat()
	return err == nil
}

func reopenOutputs(always bool) {
	output, err := os.OpenFile("CONOUT$", os.O_RDWR, 0)
	if err != nil {
		return
	}
	if always || !usable(os.Stdout) {
		os.Stdout = output
	}
	if always || !usable(os.Stderr) {
		os.Stderr = output
	}
}

func prepareConsole(arguments []string) bool {
	if console, _, _ := getConsoleWindow.Call(); console != 0 {
		return false
	}
	if launcher.RequestsTerminal(arguments) {
		if created, _, _ := allocConsole.Call(); created == 0 {
			return true
		}
		if input, err := os.OpenFile("CONIN$", os.O_RDWR, 0); err == nil {
			os.Stdin = input
		}
		reopenOutputs(true)
		return false
	}
	if attached, _, _ := attachConsole.Call(attachParentProcess); attached != 0 {
		reopenOutputs(false)
	}
	return true
}

func setConsoleVisible(visible bool) {
	console, _, _ := getConsoleWindow.Call()
	if console == 0 {
		return
	}
	state := uintptr(windowHide)
	if visible {
		state = windowShow
	}
	_, _, _ = showWindow.Call(console, state)
}
