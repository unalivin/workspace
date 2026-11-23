package service

import (
	"fmt"
	"strings"
	"time"
)

// ValidateDeviceInput protects the inventory boundary from malformed records.
func ValidateDeviceInput(name, adapter, host string) error {
	if strings.TrimSpace(name) == "" {
		return fmt.Errorf("device name is required")
	}
	if adapter != "mock" && adapter != "lab" && adapter != "ssh" {
		return fmt.Errorf("unsupported adapter %q", adapter)
	}
	if strings.TrimSpace(host) == "" {
		return fmt.Errorf("device host is required")
	}
	if strings.ContainsAny(host, "\r\n\x00") {
		return fmt.Errorf("host contains control characters")
	}
	return nil
}

func (s *Service) DisableDevice(id int64) error { return s.setDeviceEnabled(id, false) }
func (s *Service) EnableDevice(id int64) error  { return s.setDeviceEnabled(id, true) }
func (s *Service) setDeviceEnabled(id int64, enabled bool) error {
	if id <= 0 {
		return fmt.Errorf("device id must be positive")
	}
	value := 0
	if enabled {
		value = 1
	}
	result, err := s.db.Conn.Exec("UPDATE devices SET enabled=? WHERE id=?", value, id)
	if err != nil {
		return err
	}
	n, err := result.RowsAffected()
	if err != nil {
		return err
	}
	if n == 0 {
		return fmt.Errorf("device %d not found", id)
	}
	return nil
}

func (s *Service) AddDeviceTag(deviceID int64, tag string) error {
	tag = strings.TrimSpace(tag)
	if deviceID <= 0 || tag == "" || len(tag) > 80 {
		return fmt.Errorf("invalid device tag")
	}
	_, err := s.db.Conn.Exec("INSERT OR IGNORE INTO device_tags(device_id,tag) VALUES(?,?)", deviceID, tag)
	return err
}
func (s *Service) RemoveDeviceTag(deviceID int64, tag string) error {
	_, err := s.db.Conn.Exec("DELETE FROM device_tags WHERE device_id=? AND tag=?", deviceID, strings.TrimSpace(tag))
	return err
}
func (s *Service) DeviceTags(deviceID int64) ([]string, error) {
	rows, err := s.db.Conn.Query("SELECT tag FROM device_tags WHERE device_id=? ORDER BY tag", deviceID)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	out := []string{}
	for rows.Next() {
		var tag string
		if err := rows.Scan(&tag); err != nil {
			return nil, err
		}
		out = append(out, tag)
	}
	return out, rows.Err()
}
func (s *Service) FindDevicesByTag(tag string) ([]Device, error) {
	rows, err := s.db.Conn.Query("SELECT d.id,d.name,d.adapter,d.host,d.enabled FROM devices d JOIN device_tags t ON t.device_id=d.id WHERE t.tag=? ORDER BY d.name", strings.TrimSpace(tag))
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	out := []Device{}
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
func (s *Service) RecordChange(actor, action, target string) error {
	if strings.TrimSpace(actor) == "" || strings.TrimSpace(action) == "" || strings.TrimSpace(target) == "" {
		return fmt.Errorf("actor, action and target are required")
	}
	_, err := s.db.Conn.Exec("INSERT INTO change_log(actor,action,target,created_at) VALUES(?,?,?,?)", actor, action, target, time.Now().UTC().Format(time.RFC3339Nano))
	return err
}
func (s *Service) ChangeHistory(limit int) ([]ChangeRecord, error) {
	if limit <= 0 || limit > 1000 {
		return nil, fmt.Errorf("limit must be between 1 and 1000")
	}
	rows, err := s.db.Conn.Query("SELECT id,actor,action,target,created_at FROM change_log ORDER BY id DESC LIMIT ?", limit)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	out := []ChangeRecord{}
	for rows.Next() {
		var r ChangeRecord
		if err := rows.Scan(&r.ID, &r.Actor, &r.Action, &r.Target, &r.CreatedAt); err != nil {
			return nil, err
		}
		out = append(out, r)
	}
	return out, rows.Err()
}

type ChangeRecord struct {
	ID        int64  `json:"id"`
	Actor     string `json:"actor"`
	Action    string `json:"action"`
	Target    string `json:"target"`
	CreatedAt string `json:"created_at"`
}
