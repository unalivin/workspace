package tui

import (
	"fmt"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/sonix-devs/vulntrack/internal/service"
)

type model struct {
	overview service.OverviewCounts
	err      error
}

func (m model) Init() tea.Cmd { return nil }

func (m model) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	if k, ok := msg.(tea.KeyMsg); ok {
		switch k.String() {
		case "q", "ctrl+c", "esc":
			return m, tea.Quit
		}
	}
	return m, nil
}

func (m model) View() string {
	if m.err != nil {
		return fmt.Sprintf("VulnTrack error: %v\nPress q to quit.\n", m.err)
	}
	return fmt.Sprintf(
		"VulnTrack v0.1.0-dev (Terminal UI)\n\nOpen vulnerabilities: %d\nCritical open: %d\nAssets: %d\nActive exceptions: %d\n\nPress q to quit.\n",
		m.overview.OpenVulns, m.overview.CriticalOpen, m.overview.TotalAssets, m.overview.ActiveExceptions,
	)
}

func Run(svc *service.Service) error {
	c, err := svc.Overview()
	p := tea.NewProgram(model{overview: c, err: err}, tea.WithAltScreen())
	_, runErr := p.Run()
	return runErr
}
