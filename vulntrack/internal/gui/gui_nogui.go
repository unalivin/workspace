//go:build nogui

package gui

import (
	"github.com/sonix-devs/vulntrack/internal/service"
	"github.com/sonix-devs/vulntrack/internal/tui"
)

func Run(svc *service.Service) error {
	return tui.Run(svc)
}
