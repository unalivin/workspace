package main

import (
	"encoding/json"
	"flag"
	"fmt"
	"os"

	"github.com/sonix-devs/netauto/internal/db"
	"github.com/sonix-devs/netauto/internal/gui"
	"github.com/sonix-devs/netauto/internal/service"
	"github.com/sonix-devs/netauto/internal/tui"
)

func decideLaunchMode(uiFlag, envUI string, hasDisplay, isInteractive, ask, noPrompt bool) string {
	if uiFlag == "gui" || uiFlag == "tui" {
		return uiFlag
	}
	if envUI == "gui" || envUI == "tui" {
		return envUI
	}
	if !hasDisplay || !isInteractive || noPrompt {
		return "tui"
	}
	if ask {
		return "prompt"
	}
	return "gui"
}

func main() {
	uiFlag := flag.String("ui", "", "ui mode: gui or tui")
	ask := flag.Bool("ask", false, "prompt for ui mode")
	noPrompt := flag.Bool("no-prompt", false, "skip prompt")
	dbPath := flag.String("db", "netauto.db", "sqlite database path")
	flag.Parse()

	database, err := db.Open(*dbPath)
	if err != nil {
		fmt.Fprintf(os.Stderr, "open database: %v\n", err)
		os.Exit(1)
	}
	defer database.Close()

	svc := service.New(database)
	args := flag.Args()
	if len(args) > 0 {
		switch args[0] {
		case "demo":
			if err := svc.SeedDemo(); err != nil {
				fmt.Fprintf(os.Stderr, "seed demo: %v\n", err)
				os.Exit(1)
			}
			fmt.Println("Demo data loaded.")
		case "overview":
			c, err := svc.Overview()
			if err != nil {
				fmt.Fprintf(os.Stderr, "overview: %v\n", err)
				os.Exit(1)
			}
			enc, _ := json.MarshalIndent(c, "", "  ")
			fmt.Println(string(enc))
		case "devices":
			devs, err := svc.ListDevices()
			if err != nil {
				fmt.Fprintf(os.Stderr, "devices: %v\n", err)
				os.Exit(1)
			}
			enc, _ := json.MarshalIndent(devs, "", "  ")
			fmt.Println(string(enc))
		default:
			fmt.Fprintf(os.Stderr, "unknown command: %s\n", args[0])
			os.Exit(1)
		}
		return
	}

	hasDisplay := os.Getenv("DISPLAY") != "" || os.Getenv("WAYLAND_DISPLAY") != ""
	mode := decideLaunchMode(*uiFlag, os.Getenv("NETAUTO_UI"), hasDisplay, true, *ask, *noPrompt)
	if mode == "gui" {
		if err := gui.Run(svc); err != nil {
			fmt.Fprintf(os.Stderr, "gui error: %v\n", err)
			os.Exit(1)
		}
		return
	}
	if err := tui.Run(svc); err != nil {
		fmt.Fprintf(os.Stderr, "tui error: %v\n", err)
		os.Exit(1)
	}
}
