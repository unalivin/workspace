package tui

import (
	"fmt"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/sonix-devs/netauto/internal/service"
)

type model struct {
	svc      *service.Service
	overview service.OverviewCounts
	err      error
}

func (m model) Init() tea.Cmd {
	return nil
}

func (m model) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	switch msg := msg.(type) {
	case tea.KeyMsg:
		switch msg.String() {
		case "q", "ctrl+c", "esc":
			return m, tea.Quit
		}
	}
	return m, nil
}

func (m model) View() string {
	if m.err != nil {
		return fmt.Sprintf("NetAuto error: %v\nPress q to quit.\n", m.err)
	}
	return fmt.Sprintf(
		"NetAuto v0.1.0-dev (Terminal UI)\n\nActive devices: %d\nPending approvals: %d\nDry runs: %d | Executed: %d\nBackups: %d\n\nPress q to quit.\n",
		m.overview.ActiveDevices,
		m.overview.PendingApprovals,
		m.overview.DryRunCount,
		m.overview.ExecutedCount,
		m.overview.TotalBackups,
	)
}

func Run(svc *service.Service) error {
	c, err := svc.Overview()
	p := tea.NewProgram(model{svc: svc, overview: c, err: err}, tea.WithAltScreen())
	_, runErr := p.Run()
	return runErr
}
