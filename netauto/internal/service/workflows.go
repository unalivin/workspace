package service

import (
	"fmt"
	"strings"
	"time"
)

// RejectRun records an explicit human decision; automation cannot self-approve.
func (s *Service) RejectRun(runID int64, actor, note string) error {
	if runID <= 0 || strings.TrimSpace(actor) == "" {
		return fmt.Errorf("run id and decision maker are required")
	}
	tx, err := s.db.Conn.Begin()
	if err != nil {
		return err
	}
	defer tx.Rollback()
	var status string
	if err = tx.QueryRow("SELECT status FROM runs WHERE id=?", runID).Scan(&status); err != nil {
		return err
	}
	if status != "pending_approval" {
		return fmt.Errorf("run %d is not awaiting approval", runID)
	}
	now := time.Now().UTC().Format(time.RFC3339Nano)
	if _, err = tx.Exec("INSERT INTO approvals(run_id,decision,decided_by,note,decided_at) VALUES(?,'rejected',?,?,?)", runID, actor, note, now); err != nil {
		return err
	}
	if _, err = tx.Exec("UPDATE runs SET status='rejected',updated_at=? WHERE id=?", now, runID); err != nil {
		return err
	}
	_, err = tx.Exec("INSERT INTO run_events(run_id,event,detail,created_at) VALUES(?,'rejected',?,?)", runID, note, now)
	if err != nil {
		return err
	}
	return tx.Commit()
}
func (s *Service) RunByID(id int64) (Run, error) {
	var r Run
	if id <= 0 {
		return r, fmt.Errorf("run id must be positive")
	}
	err := s.db.Conn.QueryRow("SELECT id,job_id,device_id,mode,status,diff_preview,rollback_guidance FROM runs WHERE id=?", id).Scan(&r.ID, &r.JobID, &r.DeviceID, &r.Mode, &r.Status, &r.DiffPreview, &r.RollbackGuidance)
	return r, err
}
func (s *Service) ListRuns(status string, limit int) ([]Run, error) {
	if limit <= 0 || limit > 500 {
		return nil, fmt.Errorf("limit must be between 1 and 500")
	}
	rows, err := s.db.Conn.Query("SELECT id,job_id,device_id,mode,status,diff_preview,rollback_guidance FROM runs WHERE (?='' OR status=?) ORDER BY id DESC LIMIT ?", status, status, limit)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	out := []Run{}
	for rows.Next() {
		var r Run
		if err := rows.Scan(&r.ID, &r.JobID, &r.DeviceID, &r.Mode, &r.Status, &r.DiffPreview, &r.RollbackGuidance); err != nil {
			return nil, err
		}
		out = append(out, r)
	}
	return out, rows.Err()
}
func (s *Service) AppendRunEvent(runID int64, event, detail string) error {
	if runID <= 0 || strings.TrimSpace(event) == "" {
		return fmt.Errorf("run id and event are required")
	}
	_, err := s.db.Conn.Exec("INSERT INTO run_events(run_id,event,detail,created_at) VALUES(?,?,?,?)", runID, event, detail, time.Now().UTC().Format(time.RFC3339Nano))
	return err
}

type RunEvent struct {
	ID        int64  `json:"id"`
	Event     string `json:"event"`
	Detail    string `json:"detail"`
	CreatedAt string `json:"created_at"`
}

func (s *Service) RunEvents(runID int64) ([]RunEvent, error) {
	rows, err := s.db.Conn.Query("SELECT id,event,detail,created_at FROM run_events WHERE run_id=? ORDER BY id", runID)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	out := []RunEvent{}
	for rows.Next() {
		var e RunEvent
		if err := rows.Scan(&e.ID, &e.Event, &e.Detail, &e.CreatedAt); err != nil {
			return nil, err
		}
		out = append(out, e)
	}
	return out, rows.Err()
}
func (s *Service) CompleteRun(runID int64, success bool, output string) error {
	status := "failed"
	if success {
		status = "succeeded"
	}
	now := time.Now().UTC().Format(time.RFC3339Nano)
	result, err := s.db.Conn.Exec("UPDATE runs SET status=?,output_log=?,updated_at=? WHERE id=? AND status='approved' AND mode='execute'", status, output, now, runID)
	if err != nil {
		return err
	}
	n, err := result.RowsAffected()
	if err != nil {
		return err
	}
	if n == 0 {
		return fmt.Errorf("run %d is not an approved execution", runID)
	}
	_, err = s.db.Conn.Exec("INSERT INTO run_events(run_id,event,detail,created_at) VALUES(?,?,?,?)", runID, status, "execution result recorded", now)
	return err
}
