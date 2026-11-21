//go:build !nogui

package gui

import (
	"fmt"

	"fyne.io/fyne/v2/app"
	"fyne.io/fyne/v2/container"
	"fyne.io/fyne/v2/widget"
	"github.com/sonix-devs/vulntrack/internal/service"
)

func Run(svc *service.Service) error {
	a := app.New()
	w := a.NewWindow("VulnTrack")
	c, err := svc.Overview()
	text := "VulnTrack v0.1.0-dev"
	if err == nil {
		text = fmt.Sprintf(
			"Open vulnerabilities: %d\nCritical open: %d\nAssets: %d\nActive exceptions: %d",
			c.OpenVulns, c.CriticalOpen, c.TotalAssets, c.ActiveExceptions,
		)
	}
	w.SetContent(container.NewVBox(widget.NewLabel("VulnTrack"), widget.NewLabel(text)))
	w.ShowAndRun()
	return nil
}
