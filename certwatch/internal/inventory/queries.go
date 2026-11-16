package inventory

import (
	"context"
	"database/sql"
	"errors"
	"strings"
	"time"

	"github.com/unalivin/certwatch/internal/theme"
	"github.com/unalivin/certwatch/internal/tlscheck"
)

const (
	certificateColumns = "c.id, c.fingerprint, c.display_name, c.subject, c.issuer, c.serial, c.not_before, c.not_after, c.names, c.key_algorithm, c.key_bits, c.signature_algorithm, c.is_ca, c.self_signed, c.source, c.is_demo, c.first_seen_at, c.last_seen_at, coalesce(t.endpoint_count, 0), coalesce(t.used_by, ''), t.certificate_id IS NOT NULL"

	sqlCertificateGet  = "SELECT " + certificateColumns + ", c.pem FROM certificates c LEFT JOIN tracked_certificates t ON t.certificate_id = c.id WHERE c.id = ?"
	sqlCertificateList = "SELECT " + certificateColumns + ` FROM certificates c
LEFT JOIN tracked_certificates t ON t.certificate_id = c.id
WHERE (?1 = '' OR instr(lower(c.display_name || ' ' || c.names || ' ' || c.issuer || ' ' || c.fingerprint || ' ' || c.serial), ?1) > 0)
  AND (?2 = 'all'
    OR (?2 = 'authorities' AND c.is_ca = 1)
    OR (t.certificate_id IS NOT NULL AND (?2 = 'tracked'
      OR (?2 = 'expired' AND c.not_after <= ?3)
      OR (?2 = 'expiring' AND c.not_after > ?3 AND c.not_after <= ?4))))
ORDER BY c.not_after, c.id
LIMIT ?5`
	sqlBuckets = `SELECT
  coalesce(sum(not_after <= ?1), 0),
  coalesce(sum(not_after > ?1 AND not_after <= ?2), 0),
  coalesce(sum(not_after > ?1 AND not_after <= ?3), 0),
  coalesce(sum(not_after > ?1 AND not_after <= ?4), 0),
  count(*)
FROM tracked_certificates`
	sqlAttention     = "SELECT endpoints, disabled_endpoints, never_checked, failed_checks, chain_failures, hostname_mismatches, certificates, findings, demo_rows, unread_notifications FROM overview_attention"
	sqlStopTracking  = "UPDATE certificates SET source = 'observed' WHERE id = ? AND source = 'imported'"
	sqlChainOf       = "SELECT c.id, c.display_name, c.issuer, c.not_after, c.key_algorithm, c.key_bits, c.signature_algorithm, c.self_signed, c.is_ca, c.fingerprint, x.position, x.origin FROM observation_certificates x JOIN certificates c ON c.id = x.certificate_id WHERE x.observation_id = ? ORDER BY x.position"
	sqlLatestChainOf = "SELECT id FROM observations WHERE leaf_certificate_id = ? ORDER BY id DESC LIMIT 1"
	sqlUsage         = `SELECT e.id, e.host, e.port, min(o.observed_at), max(o.observed_at), count(*), e.last_observation_id IS NOT NULL AND max(o.id) = e.last_observation_id
FROM observations o JOIN endpoints e ON e.id = o.endpoint_id
WHERE o.leaf_certificate_id = ?
GROUP BY e.id
ORDER BY max(o.id) DESC`
	sqlChangesOfCertificate = `SELECT o.id, o.observed_at, e.id, e.host, e.port, coalesce(p.id, 0), coalesce(p.display_name, ''), coalesce(p.fingerprint, ''), c.id, c.display_name, c.fingerprint
FROM observations o
JOIN endpoints e ON e.id = o.endpoint_id
JOIN certificates c ON c.id = o.leaf_certificate_id
LEFT JOIN certificates p ON p.id = o.previous_certificate_id
WHERE o.changed = 1 AND (o.leaf_certificate_id = ?1 OR o.previous_certificate_id = ?1)
ORDER BY o.id DESC
LIMIT ?2`
	sqlChangesOfEndpoint = `SELECT o.id, o.observed_at, e.id, e.host, e.port, coalesce(p.id, 0), coalesce(p.display_name, ''), coalesce(p.fingerprint, ''), c.id, c.display_name, c.fingerprint
FROM observations o
JOIN endpoints e ON e.id = o.endpoint_id
JOIN certificates c ON c.id = o.leaf_certificate_id
LEFT JOIN certificates p ON p.id = o.previous_certificate_id
WHERE o.changed = 1 AND o.endpoint_id = ?1
ORDER BY o.id DESC
LIMIT ?2`
	sqlHistory = `SELECT o.id, o.observed_at, o.outcome, o.error, o.chain_status, o.hostname_status, o.tls_version, o.changed, coalesce(c.display_name, ''), coalesce(c.not_after, ''), o.duration_ms
FROM observations o LEFT JOIN certificates c ON c.id = o.leaf_certificate_id
WHERE o.endpoint_id = ?
ORDER BY o.id DESC
LIMIT ?`
	sqlHistoryCount = "SELECT count(*) FROM observations WHERE endpoint_id = ?"
	sqlFindingsOf   = "SELECT finding_id, code, severity, detail, endpoint_id, host, port, observed_at, is_demo FROM latest_findings WHERE (?1 = 0 OR endpoint_id = ?1) ORDER BY CASE severity WHEN 'critical' THEN 0 WHEN 'high' THEN 1 WHEN 'medium' THEN 2 WHEN 'low' THEN 3 ELSE 4 END, host, port, finding_id"
	sqlChainDepth   = "SELECT observation_id, count(*), sum(origin = 'trust_store') FROM observation_certificates WHERE observation_id IN (SELECT last_observation_id FROM endpoints) GROUP BY observation_id"
	sqlFindingStats = "SELECT endpoint_id, count(*), min(CASE severity WHEN 'critical' THEN 0 WHEN 'high' THEN 1 WHEN 'medium' THEN 2 WHEN 'low' THEN 3 ELSE 4 END) FROM latest_findings GROUP BY endpoint_id"
)

var CertificateFilters = []string{"tracked", "expiring-7", "expiring-30", "expiring-90", "expired", "authorities", "all"}

var severityOrder = []theme.Severity{theme.SeverityCritical, theme.SeverityHigh, theme.SeverityMedium, theme.SeverityLow, theme.SeverityInfo}

type Certificate struct {
	ID                 int64
	Fingerprint        string
	Name               string
	Subject            string
	Issuer             string
	Serial             string
	NotBefore          time.Time
	NotAfter           time.Time
	Names              []string
	KeyAlgorithm       string
	KeyBits            int
	SignatureAlgorithm string
	IsCA               bool
	SelfSigned         bool
	Source             string
	Demo               bool
	FirstSeen          time.Time
	LastSeen           time.Time
	EndpointCount      int
	UsedBy             string
	Tracked            bool
	PEM                string
}

func (c Certificate) DaysLeft(now time.Time) int {
	return tlscheck.DaysLeft(c.NotAfter, now)
}

func ExpiryState(notAfter, now time.Time) (string, theme.Severity) {
	days := tlscheck.DaysLeft(notAfter, now)
	switch {
	case !notAfter.After(now):
		return "expired", theme.SeverityCritical
	case days < 7:
		return "expiring", theme.SeverityHigh
	case days < 30:
		return "expiring", theme.SeverityMedium
	case days < 90:
		return "expiring", theme.SeverityLow
	}
	return "valid", theme.SeverityInfo
}

func scanCertificate(row scanner, withPEM bool) (Certificate, error) {
	var c Certificate
	var notBefore, notAfter, names, first, last string
	targets := []any{&c.ID, &c.Fingerprint, &c.Name, &c.Subject, &c.Issuer, &c.Serial, &notBefore, &notAfter, &names, &c.KeyAlgorithm, &c.KeyBits, &c.SignatureAlgorithm, &c.IsCA, &c.SelfSigned, &c.Source, &c.Demo, &first, &last, &c.EndpointCount, &c.UsedBy, &c.Tracked}
	if withPEM {
		targets = append(targets, &c.PEM)
	}
	err := row.Scan(targets...)
	if errors.Is(err, sql.ErrNoRows) {
		return c, ErrNotFound
	}
	c.NotBefore, c.NotAfter, c.FirstSeen, c.LastSeen = parseTime(notBefore), parseTime(notAfter), parseTime(first), parseTime(last)
	if names != "" {
		c.Names = strings.Split(names, "\n")
	}
	return c, err
}

func certificateFilter(filter string) (string, int, error) {
	filter, err := pick(filter, CertificateFilters)
	if err != nil {
		return "", 0, err
	}
	switch filter {
	case "expiring-7":
		return "expiring", 7, nil
	case "expiring-30":
		return "expiring", 30, nil
	case "expiring-90":
		return "expiring", 90, nil
	}
	return filter, 0, nil
}

func (s *Service) Certificates(ctx context.Context, query Query) ([]Certificate, error) {
	filter, days, err := certificateFilter(query.Filter)
	if err != nil {
		return nil, err
	}
	now := s.now()
	rows, err := s.db.QueryContext(ctx, sqlCertificateList, query.search(), filter, stamp(now), stamp(now.AddDate(0, 0, days)), query.limit())
	if err != nil {
		return nil, err
	}
	defer func() { _ = rows.Close() }()
	var list []Certificate
	for rows.Next() {
		certificate, err := scanCertificate(rows, false)
		if err != nil {
			return nil, err
		}
		list = append(list, certificate)
	}
	return list, rows.Err()
}

func (s *Service) Certificate(ctx context.Context, id int64) (Certificate, error) {
	return scanCertificate(s.db.QueryRowContext(ctx, sqlCertificateGet, id), true)
}

func (s *Service) StopTracking(ctx context.Context, id int64) error {
	return s.inTx(ctx, func(tx *sql.Tx) error {
		result, err := tx.ExecContext(ctx, sqlStopTracking, id)
		if err != nil {
			return err
		}
		if count, _ := result.RowsAffected(); count == 0 {
			if _, err := scanCertificate(tx.QueryRowContext(ctx, sqlCertificateGet, id), true); err != nil {
				return err
			}
			return &ValidationError{Field: "certificate", Problem: "this certificate was not imported from a file", Fix: "Certificates observed on endpoints stay in the inventory while an endpoint presents them; remove or disable the endpoint instead"}
		}
		_, err = tx.ExecContext(ctx, sqlOrphans)
		return err
	})
}

type ChainLink struct {
	CertificateID      int64
	Name               string
	Issuer             string
	NotAfter           time.Time
	KeyAlgorithm       string
	KeyBits            int
	SignatureAlgorithm string
	SelfSigned         bool
	IsCA               bool
	Fingerprint        string
	Position           int
	Origin             string
}

func (l ChainLink) Role() string {
	switch {
	case l.Position == 0:
		return "leaf"
	case l.SelfSigned:
		return "root"
	}
	return "intermediate"
}

func (s *Service) Chain(ctx context.Context, observationID int64) ([]ChainLink, error) {
	rows, err := s.db.QueryContext(ctx, sqlChainOf, observationID)
	if err != nil {
		return nil, err
	}
	defer func() { _ = rows.Close() }()
	var chain []ChainLink
	for rows.Next() {
		var link ChainLink
		var notAfter string
		if err := rows.Scan(&link.CertificateID, &link.Name, &link.Issuer, &notAfter, &link.KeyAlgorithm, &link.KeyBits, &link.SignatureAlgorithm, &link.SelfSigned, &link.IsCA, &link.Fingerprint, &link.Position, &link.Origin); err != nil {
			return nil, err
		}
		link.NotAfter = parseTime(notAfter)
		chain = append(chain, link)
	}
	return chain, rows.Err()
}

func (s *Service) LatestChainOf(ctx context.Context, certificateID int64) ([]ChainLink, error) {
	var observation int64
	err := s.db.QueryRowContext(ctx, sqlLatestChainOf, certificateID).Scan(&observation)
	if errors.Is(err, sql.ErrNoRows) {
		return nil, nil
	}
	if err != nil {
		return nil, err
	}
	return s.Chain(ctx, observation)
}

type ChainStats struct {
	Depth     int
	FromStore int
}

func (s *Service) ChainStats(ctx context.Context) (map[int64]ChainStats, error) {
	rows, err := s.db.QueryContext(ctx, sqlChainDepth)
	if err != nil {
		return nil, err
	}
	defer func() { _ = rows.Close() }()
	stats := map[int64]ChainStats{}
	for rows.Next() {
		var observation int64
		var item ChainStats
		if err := rows.Scan(&observation, &item.Depth, &item.FromStore); err != nil {
			return nil, err
		}
		stats[observation] = item
	}
	return stats, rows.Err()
}

type Usage struct {
	EndpointID int64
	Address    string
	First      time.Time
	Last       time.Time
	Count      int
	Current    bool
}

func (s *Service) Usage(ctx context.Context, certificateID int64) ([]Usage, error) {
	rows, err := s.db.QueryContext(ctx, sqlUsage, certificateID)
	if err != nil {
		return nil, err
	}
	defer func() { _ = rows.Close() }()
	var list []Usage
	for rows.Next() {
		var item Usage
		var host, first, last string
		var port int
		if err := rows.Scan(&item.EndpointID, &host, &port, &first, &last, &item.Count, &item.Current); err != nil {
			return nil, err
		}
		item.Address, item.First, item.Last = Endpoint{Host: host, Port: port}.Address(), parseTime(first), parseTime(last)
		list = append(list, item)
	}
	return list, rows.Err()
}

type Change struct {
	ObservationID int64
	At            time.Time
	EndpointID    int64
	Address       string
	FromID        int64
	FromName      string
	FromPrint     string
	ToID          int64
	ToName        string
	ToPrint       string
}

func scanChanges(rows *sql.Rows, err error) ([]Change, error) {
	if err != nil {
		return nil, err
	}
	defer func() { _ = rows.Close() }()
	var list []Change
	for rows.Next() {
		var item Change
		var at, host string
		var port int
		if err := rows.Scan(&item.ObservationID, &at, &item.EndpointID, &host, &port, &item.FromID, &item.FromName, &item.FromPrint, &item.ToID, &item.ToName, &item.ToPrint); err != nil {
			return nil, err
		}
		item.At, item.Address = parseTime(at), Endpoint{Host: host, Port: port}.Address()
		list = append(list, item)
	}
	return list, rows.Err()
}

func (s *Service) CertificateChanges(ctx context.Context, certificateID int64, limit int) ([]Change, error) {
	return scanChanges(s.db.QueryContext(ctx, sqlChangesOfCertificate, certificateID, limit))
}

func (s *Service) EndpointChanges(ctx context.Context, endpointID int64, limit int) ([]Change, error) {
	return scanChanges(s.db.QueryContext(ctx, sqlChangesOfEndpoint, endpointID, limit))
}

type HistoryEntry struct {
	ID              int64
	At              time.Time
	Outcome         string
	Error           string
	ChainStatus     string
	HostnameStatus  string
	Version         string
	Changed         bool
	CertificateName string
	NotAfter        time.Time
	Duration        time.Duration
}

func (s *Service) History(ctx context.Context, endpointID int64, limit int) ([]HistoryEntry, int, error) {
	var total int
	if err := s.db.QueryRowContext(ctx, sqlHistoryCount, endpointID).Scan(&total); err != nil {
		return nil, 0, err
	}
	rows, err := s.db.QueryContext(ctx, sqlHistory, endpointID, limit)
	if err != nil {
		return nil, 0, err
	}
	defer func() { _ = rows.Close() }()
	var list []HistoryEntry
	for rows.Next() {
		var item HistoryEntry
		var at, notAfter string
		var milliseconds int64
		if err := rows.Scan(&item.ID, &at, &item.Outcome, &item.Error, &item.ChainStatus, &item.HostnameStatus, &item.Version, &item.Changed, &item.CertificateName, &notAfter, &milliseconds); err != nil {
			return nil, 0, err
		}
		item.At, item.NotAfter, item.Duration = parseTime(at), parseTime(notAfter), time.Duration(milliseconds)*time.Millisecond
		list = append(list, item)
	}
	return list, total, rows.Err()
}

type Finding struct {
	ID         int64
	Code       string
	Severity   theme.Severity
	Detail     string
	EndpointID int64
	Address    string
	ObservedAt time.Time
	Demo       bool
}

func (s *Service) Findings(ctx context.Context, endpointID int64) ([]Finding, error) {
	rows, err := s.db.QueryContext(ctx, sqlFindingsOf, endpointID)
	if err != nil {
		return nil, err
	}
	defer func() { _ = rows.Close() }()
	var list []Finding
	for rows.Next() {
		var item Finding
		var severity, host, observed string
		var port int
		if err := rows.Scan(&item.ID, &item.Code, &severity, &item.Detail, &item.EndpointID, &host, &port, &observed, &item.Demo); err != nil {
			return nil, err
		}
		item.Severity, item.Address, item.ObservedAt = theme.Severity(severity), Endpoint{Host: host, Port: port}.Address(), parseTime(observed)
		list = append(list, item)
	}
	return list, rows.Err()
}

type FindingStats struct {
	Count int
	Worst theme.Severity
}

func (s *Service) FindingStats(ctx context.Context) (map[int64]FindingStats, error) {
	rows, err := s.db.QueryContext(ctx, sqlFindingStats)
	if err != nil {
		return nil, err
	}
	defer func() { _ = rows.Close() }()
	stats := map[int64]FindingStats{}
	for rows.Next() {
		var endpoint int64
		var count, worst int
		if err := rows.Scan(&endpoint, &count, &worst); err != nil {
			return nil, err
		}
		stats[endpoint] = FindingStats{Count: count, Worst: severityOrder[worst]}
	}
	return stats, rows.Err()
}

type Overview struct {
	Expired            int
	Within7            int
	Within30           int
	Within90           int
	Certificates       int
	Endpoints          int
	DisabledEndpoints  int
	NeverChecked       int
	FailedChecks       int
	ChainFailures      int
	HostnameMismatches int
	Findings           int
	DemoRows           int
	UnreadAlerts       int
}

func (o Overview) Empty() bool {
	return o.Endpoints == 0 && o.Certificates == 0
}

func (s *Service) Overview(ctx context.Context) (Overview, error) {
	var o Overview
	now := s.now()
	err := s.db.QueryRowContext(ctx, sqlBuckets, stamp(now), stamp(now.AddDate(0, 0, 7)), stamp(now.AddDate(0, 0, 30)), stamp(now.AddDate(0, 0, 90))).Scan(&o.Expired, &o.Within7, &o.Within30, &o.Within90, new(int))
	if err != nil {
		return o, err
	}
	err = s.db.QueryRowContext(ctx, sqlAttention).Scan(&o.Endpoints, &o.DisabledEndpoints, &o.NeverChecked, &o.FailedChecks, &o.ChainFailures, &o.HostnameMismatches, &o.Certificates, &o.Findings, &o.DemoRows, &o.UnreadAlerts)
	return o, err
}

var HotQueries = map[string]string{
	"endpoint list":               sqlEndpointList,
	"endpoints due for a check":   sqlEndpointDue,
	"certificate list":            sqlCertificateList,
	"expiry buckets":              sqlBuckets,
	"overview attention":          sqlAttention,
	"endpoint history":            sqlHistory,
	"latest findings":             sqlFindingsOf,
	"certificate change history":  sqlChangesOfCertificate,
	"previous leaf of endpoint":   sqlPreviousLeaf,
	"prune observation history":   sqlPrune,
	"expiry threshold candidates": sqlExpiryDue,
}
