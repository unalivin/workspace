package service

import (
	"fmt"
	"time"

	"github.com/sonix-devs/vulntrack/internal/db"
)

type OverviewCounts struct {
	OpenVulns        int `json:"open_vulns"`
	CriticalOpen     int `json:"critical_open"`
	TotalAssets      int `json:"total_assets"`
	ActiveExceptions int `json:"active_exceptions"`
}

type Vulnerability struct {
	ID             int64   `json:"id"`
	AssetID        int64   `json:"asset_id"`
	CVEID          string  `json:"cve_id"`
	Title          string  `json:"title"`
	Severity       string  `json:"severity"`
	Exploitability string  `json:"exploitability"`
	RiskScore      float64 `json:"risk_score"`
	Status         string  `json:"status"`
	Owner          string  `json:"owner"`
}

type Service struct {
	db *db.Database
}

func New(database *db.Database) *Service {
	return &Service{db: database}
}

func ComputeRiskScore(severity, exploitability string) float64 {
	base := 2.0
	switch severity {
	case "critical":
		base = 9.0
	case "high":
		base = 7.0
	case "medium":
		base = 4.5
	}
	switch exploitability {
	case "active":
		base += 1.0
	case "poc":
		base += 0.5
	}
	if base > 10.0 {
		return 10.0
	}
	return base
}

func (s *Service) Overview() (OverviewCounts, error) {
	var c OverviewCounts
	err := s.db.Conn.QueryRow(
		"SELECT open_vulns, critical_open, total_assets, active_exceptions FROM v_overview_counts",
	).Scan(&c.OpenVulns, &c.CriticalOpen, &c.TotalAssets, &c.ActiveExceptions)
	return c, err
}

func (s *Service) AddAsset(name, kind, owner string) (int64, error) {
	now := time.Now().UTC().Format(time.RFC3339)
	res, err := s.db.Conn.Exec(
		"INSERT INTO assets (name, kind, owner, created_at) VALUES (?, ?, ?, ?)",
		name, kind, owner, now,
	)
	if err != nil {
		return 0, fmt.Errorf("add asset %s: %w", name, err)
	}
	return res.LastInsertId()
}

func (s *Service) ImportFinding(assetID int64, cveID, title, severity, exploitability, owner string) (int64, error) {
	now := time.Now().UTC()
	slaDue := now.Add(14 * 24 * time.Hour).Format(time.RFC3339)
	if severity == "critical" {
		slaDue = now.Add(48 * time.Hour).Format(time.RFC3339)
	}
	score := ComputeRiskScore(severity, exploitability)
	dedupKey := fmt.Sprintf("%d:%s", assetID, cveID)
	nowStr := now.Format(time.RFC3339)

	res, err := s.db.Conn.Exec(
		`INSERT INTO vulnerabilities (asset_id, cve_id, title, severity, exploitability, risk_score, status, owner, sla_due_at, dedup_key, created_at, updated_at)
		 VALUES (?, ?, ?, ?, ?, ?, 'reported', ?, ?, ?, ?, ?)
		 ON CONFLICT(dedup_key) DO UPDATE SET updated_at = excluded.updated_at`,
		assetID, cveID, title, severity, exploitability, score, owner, slaDue, dedupKey, nowStr, nowStr,
	)
	if err != nil {
		return 0, err
	}
	return res.LastInsertId()
}

func (s *Service) ListVulnerabilities() ([]Vulnerability, error) {
	rows, err := s.db.Conn.Query(
		"SELECT id, asset_id, cve_id, title, severity, exploitability, risk_score, status, owner FROM vulnerabilities ORDER BY risk_score DESC, id ASC",
	)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	var out []Vulnerability
	for rows.Next() {
		var v Vulnerability
		if err := rows.Scan(&v.ID, &v.AssetID, &v.CVEID, &v.Title, &v.Severity, &v.Exploitability, &v.RiskScore, &v.Status, &v.Owner); err != nil {
			return nil, err
		}
		out = append(out, v)
	}
	return out, rows.Err()
}

func (s *Service) SeedDemo() error {
	c, err := s.Overview()
	if err != nil || c.TotalAssets > 0 {
		return err
	}
	a1, err := s.AddAsset("edge-proxy-01", "container", "platform-team")
	if err != nil {
		return err
	}
	if _, err := s.ImportFinding(a1, "CVE-2026-1001", "OpenSSL buffer over-read", "critical", "active", "platform-team"); err != nil {
		return err
	}
	_, err = s.ImportFinding(a1, "CVE-2026-1002", "Outdated curl dependency", "medium", "unknown", "platform-team")
	return err
}
