//go:build !nogui

package gui

import (
	"fmt"

	"fyne.io/fyne/v2/app"
	"fyne.io/fyne/v2/container"
	"fyne.io/fyne/v2/widget"
	"github.com/sonix-devs/netauto/internal/service"
)

func Run(svc *service.Service) error {
	a := app.New()
	w := a.NewWindow("NetAuto")
	c, err := svc.Overview()
	text := "NetAuto v0.1.0-dev"
	if err == nil {
		text = fmt.Sprintf(
			"Active devices: %d\nPending approvals: %d\nDry runs: %d | Executed: %d\nBackups: %d",
			c.ActiveDevices, c.PendingApprovals, c.DryRunCount, c.ExecutedCount, c.TotalBackups,
		)
	}
	w.SetContent(container.NewVBox(
		widget.NewLabel("NetAuto"),
		widget.NewLabel(text),
	))
	w.ShowAndRun()
	return nil
}
