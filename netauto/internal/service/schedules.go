package service

import (
	"fmt"
	"strings"
	"time"
)

type Schedule struct {
	ID        int64  `json:"id"`
	JobID     int64  `json:"job_id"`
	DeviceID  int64  `json:"device_id"`
	Cron      string `json:"cron"`
	Enabled   bool   `json:"enabled"`
	NextRunAt string `json:"next_run_at"`
}
type MaintenanceWindow struct {
	ID        int64  `json:"id"`
	Name      string `json:"name"`
	StartsAt  string `json:"starts_at"`
	EndsAt    string `json:"ends_at"`
	CreatedBy string `json:"created_by"`
}

func (s *Service) AddSchedule(jobID, deviceID int64, cron, nextRun string) (int64, error) {
	if jobID <= 0 || deviceID <= 0 || !validCronShape(cron) {
		return 0, fmt.Errorf("invalid schedule")
	}
	if _, err := time.Parse(time.RFC3339, nextRun); err != nil {
		return 0, fmt.Errorf("next run must be RFC3339: %w", err)
	}
	res, err := s.db.Conn.Exec("INSERT INTO schedules(job_id,device_id,cron,next_run_at,created_at) VALUES(?,?,?,?,?)", jobID, deviceID, strings.TrimSpace(cron), nextRun, time.Now().UTC().Format(time.RFC3339Nano))
	if err != nil {
		return 0, err
	}
	return res.LastInsertId()
}
func validCronShape(cron string) bool {
	fields := strings.Fields(cron)
	if len(fields) != 5 {
		return false
	}
	for _, f := range fields {
		if f == "" {
			return false
		}
		for _, r := range f {
			if !(r >= '0' && r <= '9' || r == '*' || r == '/' || r == ',' || r == '-') {
				return false
			}
		}
	}
	return true
}
func (s *Service) SetScheduleEnabled(id int64, enabled bool) error {
	v := 0
	if enabled {
		v = 1
	}
	r, e := s.db.Conn.Exec("UPDATE schedules SET enabled=? WHERE id=?", v, id)
	if e != nil {
		return e
	}
	n, e := r.RowsAffected()
	if e != nil {
		return e
	}
	if n == 0 {
		return fmt.Errorf("schedule not found")
	}
	return nil
}
func (s *Service) ListSchedules() ([]Schedule, error) {
	rows, e := s.db.Conn.Query("SELECT id,job_id,device_id,cron,enabled,next_run_at FROM schedules ORDER BY id")
	if e != nil {
		return nil, e
	}
	defer rows.Close()
	out := []Schedule{}
	for rows.Next() {
		var x Schedule
		var enabled int
		if e := rows.Scan(&x.ID, &x.JobID, &x.DeviceID, &x.Cron, &enabled, &x.NextRunAt); e != nil {
			return nil, e
		}
		x.Enabled = enabled != 0
		out = append(out, x)
	}
	return out, rows.Err()
}
func (s *Service) AddMaintenanceWindow(name, starts, ends, actor string) (int64, error) {
	a, e := time.Parse(time.RFC3339, starts)
	if e != nil {
		return 0, e
	}
	b, e := time.Parse(time.RFC3339, ends)
	if e != nil {
		return 0, e
	}
	if strings.TrimSpace(name) == "" || strings.TrimSpace(actor) == "" || !b.After(a) {
		return 0, fmt.Errorf("invalid maintenance window")
	}
	r, e := s.db.Conn.Exec("INSERT INTO maintenance_windows(name,starts_at,ends_at,created_by,created_at) VALUES(?,?,?,?,?)", name, a.UTC().Format(time.RFC3339), b.UTC().Format(time.RFC3339), actor, time.Now().UTC().Format(time.RFC3339Nano))
	if e != nil {
		return 0, e
	}
	return r.LastInsertId()
}
func (s *Service) MaintenanceWindows() ([]MaintenanceWindow, error) {
	rows, e := s.db.Conn.Query("SELECT id,name,starts_at,ends_at,created_by FROM maintenance_windows ORDER BY starts_at")
	if e != nil {
		return nil, e
	}
	defer rows.Close()
	out := []MaintenanceWindow{}
	for rows.Next() {
		var x MaintenanceWindow
		if e := rows.Scan(&x.ID, &x.Name, &x.StartsAt, &x.EndsAt, &x.CreatedBy); e != nil {
			return nil, e
		}
		out = append(out, x)
	}
	return out, rows.Err()
}
func (s *Service) IsMaintenance(at time.Time) (bool, error) {
	var n int
	e := s.db.Conn.QueryRow("SELECT COUNT(*) FROM maintenance_windows WHERE starts_at<=? AND ends_at>?", at.UTC().Format(time.RFC3339), at.UTC().Format(time.RFC3339)).Scan(&n)
	return n > 0, e
}
