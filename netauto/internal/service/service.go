package service

import (
	"crypto/sha256"
	"encoding/hex"
	"fmt"
	"time"

	"github.com/sonix-devs/netauto/internal/db"
)

type OverviewCounts struct {
	ActiveDevices    int `json:"active_devices"`
	PendingApprovals int `json:"pending_approvals"`
	DryRunCount      int `json:"dry_run_count"`
	ExecutedCount    int `json:"executed_count"`
	TotalBackups     int `json:"total_backups"`
}

type Device struct {
	ID      int64  `json:"id"`
	Name    string `json:"name"`
	Adapter string `json:"adapter"`
	Host    string `json:"host"`
	Enabled bool   `json:"enabled"`
}

type Run struct {
	ID               int64  `json:"id"`
	JobID            int64  `json:"job_id"`
	DeviceID         int64  `json:"device_id"`
	Mode             string `json:"mode"`
	Status           string `json:"status"`
	DiffPreview      string `json:"diff_preview"`
	RollbackGuidance string `json:"rollback_guidance"`
}

type Service struct {
	db *db.Database
}

func New(database *db.Database) *Service {
	return &Service{db: database}
}

func (s *Service) Overview() (OverviewCounts, error) {
	var c OverviewCounts
	err := s.db.Conn.QueryRow(
		"SELECT active_devices, pending_approvals, dry_run_count, executed_count, total_backups FROM v_overview_counts",
	).Scan(&c.ActiveDevices, &c.PendingApprovals, &c.DryRunCount, &c.ExecutedCount, &c.TotalBackups)
	return c, err
}

func (s *Service) AddDevice(name, adapter, host string) (int64, error) {
	now := time.Now().UTC().Format(time.RFC3339)
	res, err := s.db.Conn.Exec(
		"INSERT INTO devices (name, adapter, host, enabled, created_at) VALUES (?, ?, ?, 1, ?)",
		name, adapter, host, now,
	)
	if err != nil {
		return 0, fmt.Errorf("add device %s: %w", name, err)
	}
	return res.LastInsertId()
}

func (s *Service) ListDevices() ([]Device, error) {
	rows, err := s.db.Conn.Query("SELECT id, name, adapter, host, enabled FROM devices ORDER BY name ASC")
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	var out []Device
	for rows.Next() {
		var d Device
		var enabled int
		if err := rows.Scan(&d.ID, &d.Name, &d.Adapter, &d.Host, &enabled); err != nil {
			return nil, err
		}
		d.Enabled = enabled != 0
		out = append(out, d)
	}
	return out, rows.Err()
}

func (s *Service) AddJob(name, description, commandTemplate string) (int64, error) {
	now := time.Now().UTC().Format(time.RFC3339)
	res, err := s.db.Conn.Exec(
		"INSERT INTO jobs (name, description, command_template, created_at) VALUES (?, ?, ?, ?)",
		name, description, commandTemplate, now,
	)
	if err != nil {
		return 0, fmt.Errorf("add job %s: %w", name, err)
	}
	return res.LastInsertId()
}

func (s *Service) CreateDryRun(jobID, deviceID int64, diffPreview, rollback string) (int64, error) {
	now := time.Now().UTC().Format(time.RFC3339)
	res, err := s.db.Conn.Exec(
		"INSERT INTO runs (job_id, device_id, mode, status, diff_preview, rollback_guidance, created_at, updated_at) VALUES (?, ?, 'dry-run', 'pending_approval', ?, ?, ?, ?)",
		jobID, deviceID, diffPreview, rollback, now, now,
	)
	if err != nil {
		return 0, err
	}
	return res.LastInsertId()
}

func (s *Service) ApproveRun(runID int64, actor, note string) error {
	now := time.Now().UTC().Format(time.RFC3339)
	if _, err := s.db.Conn.Exec(
		"INSERT INTO approvals (run_id, decision, decided_by, note, decided_at) VALUES (?, 'approved', ?, ?, ?)",
		runID, actor, note, now,
	); err != nil {
		return err
	}
	_, err := s.db.Conn.Exec(
		"UPDATE runs SET status = 'approved', mode = 'execute', updated_at = ? WHERE id = ?",
		now, runID,
	)
	return err
}

func (s *Service) SaveBackup(deviceID int64, configText string) (int64, error) {
	now := time.Now().UTC().Format(time.RFC3339)
	sum := sha256.Sum256([]byte(configText))
	hash := hex.EncodeToString(sum[:])
	res, err := s.db.Conn.Exec(
		"INSERT INTO backups (device_id, config_text, sha256, created_at) VALUES (?, ?, ?, ?)",
		deviceID, configText, hash, now,
	)
	if err != nil {
		return 0, err
	}
	return res.LastInsertId()
}

func (s *Service) SeedDemo() error {
	devices, err := s.ListDevices()
	if err != nil || len(devices) > 0 {
		return err
	}
	devID, err := s.AddDevice("lab-core-01", "mock", "192.0.2.10")
	if err != nil {
		return err
	}
	jobID, err := s.AddJob("ntp-sync", "Configure NTP servers", "set system ntp server 192.0.2.1")
	if err != nil {
		return err
	}
	if _, err := s.SaveBackup(devID, "hostname lab-core-01\ninterface eth0\n ip address 192.0.2.10/24\n"); err != nil {
		return err
	}
	_, err = s.CreateDryRun(jobID, devID, "+ set system ntp server 192.0.2.1", "delete system ntp server 192.0.2.1")
	return err
}
