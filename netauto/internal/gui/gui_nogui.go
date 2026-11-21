//go:build nogui

package gui

import (
	"github.com/sonix-devs/netauto/internal/service"
	"github.com/sonix-devs/netauto/internal/tui"
)

func Run(svc *service.Service) error {
	return tui.Run(svc)
}
