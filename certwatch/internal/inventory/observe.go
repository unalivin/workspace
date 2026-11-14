package inventory

import (
	"context"
	"crypto/x509"
	"database/sql"
	"errors"
	"fmt"
	"strings"
	"time"

	"github.com/NexzaDev/certwatch/internal/notify"
	"github.com/NexzaDev/certwatch/internal/theme"
	"github.com/NexzaDev/certwatch/internal/tlscheck"
)

const (
	DefaultKeep = 200
	MaxKeep     = 5000

	OriginServer     = "server"
	OriginTrustStore = "trust_store"

	SourceObserved = "observed"
	SourceImported = "imported"

	sqlCertificateUpsert = `INSERT INTO certificates (fingerprint, display_name, subject, issuer, serial, not_before, not_after, names, key_algorithm, key_bits, signature_algorithm, is_ca, self_signed, pem, source, is_demo, first_seen_at, last_seen_at)
VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
ON CONFLICT (fingerprint) DO UPDATE SET last_seen_at = excluded.last_seen_at, source = CASE WHEN excluded.source = 'imported' THEN 'imported' ELSE certificates.source END
RETURNING id`
	sqlCertificateKnown  = "SELECT id FROM certificates WHERE fingerprint = ? AND source = 'imported'"
	sqlPreviousLeaf      = "SELECT leaf_certificate_id FROM observations WHERE endpoint_id = ? AND outcome = 'ok' ORDER BY id DESC LIMIT 1"
	sqlPreviousState     = "SELECT chain_status, hostname_status FROM observations WHERE endpoint_id = ? ORDER BY id DESC LIMIT 1"
	sqlObservationInsert = `INSERT INTO observations (endpoint_id, observed_at, outcome, error, duration_ms, leaf_certificate_id, previous_certificate_id, chain_status, chain_error, hostname_status, hostname_error, tls_version, cipher_suite, accepted_versions, protocols_tested, changed)
VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`
	sqlChainInsert     = "INSERT INTO observation_certificates (observation_id, position, certificate_id, origin) VALUES (?, ?, ?, ?)"
	sqlFindingInsert   = "INSERT INTO findings (observation_id, code, severity, detail) VALUES (?, ?, ?, ?)"
	sqlEndpointObserve = "UPDATE endpoints SET last_observation_id = ? WHERE id = ?"
	sqlPrune           = "DELETE FROM observations WHERE endpoint_id = ?1 AND id NOT IN (SELECT id FROM observations WHERE endpoint_id = ?1 ORDER BY id DESC LIMIT ?2)"
	sqlExpiryDue       = "SELECT certificate_id, display_name, not_after, is_demo FROM tracked_certificates WHERE not_after <= ? ORDER BY not_after, certificate_id"
	sqlExpiryKnown     = "SELECT threshold_days FROM expiry_alerts WHERE certificate_id = ?"
	sqlExpiryInsert    = "INSERT INTO expiry_alerts (certificate_id, threshold_days, alerted_at) VALUES (?, ?, ?) ON CONFLICT DO NOTHING"
)

var DefaultThresholds = []int{30, 14, 7, 1}

type Policy struct {
	Thresholds []int
	Keep       int
}

func (p Policy) keep() int {
	if p.Keep <= 0 {
		return DefaultKeep
	}
	if p.Keep > MaxKeep {
		return MaxKeep
	}
	return p.Keep
}

func (p Policy) thresholds() []int {
	if p.Thresholds == nil {
		return DefaultThresholds
	}
	return p.Thresholds
}

type Observation struct {
	ID            int64
	EndpointID    int64
	CertificateID int64
	Changed       bool
	Findings      []tlscheck.Finding
	Alerts        int
}

func upsertCertificate(ctx context.Context, tx *sql.Tx, certificate *x509.Certificate, moment, source string, demo bool) (int64, error) {
	info := tlscheck.Describe(certificate)
	name := info.DisplayName()
	if len(name) > 400 {
		name = name[:400]
	}
	var id int64
	err := tx.QueryRowContext(ctx, sqlCertificateUpsert, info.Fingerprint, name, info.Subject, info.Issuer, info.Serial, stamp(info.NotBefore), stamp(info.NotAfter),
		strings.Join(info.Names, "\n"), info.KeyAlgorithm, info.KeyBits, info.SignatureAlgorithm, flag(info.IsCA), flag(info.SelfSigned), info.PEM, source, flag(demo), moment, moment).Scan(&id)
	return id, err
}

func expirySeverity(days int) theme.Severity {
	switch {
	case days < 0:
		return theme.SeverityCritical
	case days <= 7:
		return theme.SeverityHigh
	case days <= 30:
		return theme.SeverityMedium
	}
	return theme.SeverityLow
}

func demoSuffix(demo bool) string {
	if demo {
		return " (demo)"
	}
	return ""
}

func evaluateExpiry(ctx context.Context, tx *sql.Tx, now time.Time, thresholds []int) (int, error) {
	longest := 0
	for _, days := range thresholds {
		if days > longest {
			longest = days
		}
	}
	type due struct {
		id       int64
		name     string
		notAfter time.Time
		demo     bool
	}
	rows, err := tx.QueryContext(ctx, sqlExpiryDue, stamp(now.AddDate(0, 0, longest)))
	if err != nil {
		return 0, err
	}
	var list []due
	for rows.Next() {
		var item due
		var notAfter string
		if err := rows.Scan(&item.id, &item.name, &notAfter, &item.demo); err != nil {
			_ = rows.Close()
			return 0, err
		}
		item.notAfter = parseTime(notAfter)
		list = append(list, item)
	}
	if err := rows.Close(); err != nil {
		return 0, err
	}
	published := 0
	for _, item := range list {
		known := map[int]bool{}
		knownRows, err := tx.QueryContext(ctx, sqlExpiryKnown, item.id)
		if err != nil {
			return published, err
		}
		for knownRows.Next() {
			var days int
			if err := knownRows.Scan(&days); err != nil {
				_ = knownRows.Close()
				return published, err
			}
			known[days] = true
		}
		if err := knownRows.Close(); err != nil {
			return published, err
		}
		left := item.notAfter.Sub(now)
		crossed := []int{}
		if left <= 0 {
			crossed = append(crossed, 0)
		}
		for _, days := range thresholds {
			if left <= time.Duration(days)*24*time.Hour {
				crossed = append(crossed, days)
			}
		}
		smallest, fresh := -1, false
		for _, days := range crossed {
			if smallest < 0 || days < smallest {
				smallest = days
			}
			if !known[days] {
				fresh = true
				if _, err := tx.ExecContext(ctx, sqlExpiryInsert, item.id, days, stamp(now)); err != nil {
					return published, err
				}
			}
		}
		if !fresh {
			continue
		}
		title := fmt.Sprintf("Certificate for %s expires within %d days%s", item.name, smallest, demoSuffix(item.demo))
		switch smallest {
		case 0:
			title = fmt.Sprintf("Certificate for %s has expired%s", item.name, demoSuffix(item.demo))
		case 1:
			title = fmt.Sprintf("Certificate for %s expires within 1 day%s", item.name, demoSuffix(item.demo))
		}
		event := notify.Event{Type: notify.EventExpiring, Severity: expirySeverity(tlscheck.DaysLeft(item.notAfter, now)), Title: title, RecordKind: KindCertificate, RecordID: item.id, DedupKey: fmt.Sprintf("certificate:%d:%d", item.id, smallest)}
		outcome, _, err := notify.PublishWith(ctx, tx, now, event)
		if err != nil {
			return published, err
		}
		if outcome == notify.Delivered {
			published++
		}
	}
	return published, nil
}

func (s *Service) EvaluateExpiry(ctx context.Context, policy Policy) (int, error) {
	published := 0
	err := s.inTx(ctx, func(tx *sql.Tx) error {
		var err error
		published, err = evaluateExpiry(ctx, tx, s.now(), policy.thresholds())
		return err
	})
	return published, err
}

func publish(ctx context.Context, tx *sql.Tx, now time.Time, event notify.Event, count *int) error {
	outcome, _, err := notify.PublishWith(ctx, tx, now, event)
	if err == nil && outcome == notify.Delivered {
		*count++
	}
	return err
}

func record(ctx context.Context, tx *sql.Tx, now time.Time, endpointID int64, result tlscheck.Result, policy Policy) (Observation, error) {
	observation := Observation{EndpointID: endpointID}
	endpoint, err := scanEndpoint(tx.QueryRowContext(ctx, sqlEndpointGet, endpointID))
	if err != nil {
		return observation, err
	}
	observedAt := result.ObservedAt
	if observedAt.IsZero() {
		observedAt = now
	}
	moment := stamp(observedAt)
	var previousLeaf sql.NullInt64
	if err := tx.QueryRowContext(ctx, sqlPreviousLeaf, endpointID).Scan(&previousLeaf); err != nil && !errors.Is(err, sql.ErrNoRows) {
		return observation, err
	}
	previousChain, previousHostname := tlscheck.StatusUnknown, tlscheck.StatusUnknown
	if err := tx.QueryRowContext(ctx, sqlPreviousState, endpointID).Scan(&previousChain, &previousHostname); err != nil && !errors.Is(err, sql.ErrNoRows) {
		return observation, err
	}
	type link struct {
		id     int64
		origin string
	}
	var chain []link
	if result.Outcome == tlscheck.OutcomeOK {
		presented := map[string]bool{}
		for _, certificate := range result.Presented {
			id, err := upsertCertificate(ctx, tx, certificate, moment, SourceObserved, endpoint.Demo)
			if err != nil {
				return observation, err
			}
			presented[tlscheck.Fingerprint(certificate)] = true
			chain = append(chain, link{id: id, origin: OriginServer})
		}
		for _, certificate := range result.Verified {
			if presented[tlscheck.Fingerprint(certificate)] {
				continue
			}
			id, err := upsertCertificate(ctx, tx, certificate, moment, SourceObserved, endpoint.Demo)
			if err != nil {
				return observation, err
			}
			chain = append(chain, link{id: id, origin: OriginTrustStore})
		}
		if len(chain) == 0 {
			return observation, errors.New("a successful observation needs at least one certificate")
		}
		observation.CertificateID = chain[0].id
		observation.Changed = previousLeaf.Valid && previousLeaf.Int64 != chain[0].id
		observation.Findings = tlscheck.Findings(result, observedAt)
	}
	var leaf, previous any
	if observation.CertificateID != 0 {
		leaf = observation.CertificateID
	}
	if observation.Changed {
		previous = previousLeaf.Int64
	}
	inserted, err := tx.ExecContext(ctx, sqlObservationInsert, endpointID, moment, result.Outcome, result.Error, result.Duration.Milliseconds(), leaf, previous,
		result.ChainStatus, result.ChainError, result.HostnameStatus, result.HostnameError, result.Version, result.CipherSuite, strings.Join(result.Accepted, ", "), flag(result.ProtocolsTested), flag(observation.Changed))
	if err != nil {
		return observation, err
	}
	if observation.ID, err = inserted.LastInsertId(); err != nil {
		return observation, err
	}
	for position, item := range chain {
		if _, err := tx.ExecContext(ctx, sqlChainInsert, observation.ID, position, item.id, item.origin); err != nil {
			return observation, err
		}
	}
	for _, finding := range observation.Findings {
		if _, err := tx.ExecContext(ctx, sqlFindingInsert, observation.ID, finding.Code, string(finding.Severity), finding.Detail); err != nil {
			return observation, err
		}
	}
	if _, err := tx.ExecContext(ctx, sqlEndpointObserve, observation.ID, endpointID); err != nil {
		return observation, err
	}
	if _, err := tx.ExecContext(ctx, sqlPrune, endpointID, policy.keep()); err != nil {
		return observation, err
	}
	if _, err := tx.ExecContext(ctx, sqlOrphans); err != nil {
		return observation, err
	}
	if !endpoint.Enabled {
		return observation, nil
	}
	address, suffix := endpoint.Address(), demoSuffix(endpoint.Demo)
	if result.ChainStatus == tlscheck.StatusInvalid && previousChain != tlscheck.StatusInvalid {
		event := notify.Event{Type: notify.EventValidation, Severity: theme.SeverityHigh, Title: "Chain validation failed for " + address + suffix, RecordKind: KindEndpoint, RecordID: endpointID, DedupKey: fmt.Sprintf("endpoint:%d:chain", endpointID)}
		if err := publish(ctx, tx, now, event, &observation.Alerts); err != nil {
			return observation, err
		}
	}
	if result.HostnameStatus == tlscheck.HostnameMismatch && previousHostname != tlscheck.HostnameMismatch {
		event := notify.Event{Type: notify.EventValidation, Severity: theme.SeverityHigh, Title: "Host name validation failed for " + address + suffix, RecordKind: KindEndpoint, RecordID: endpointID, DedupKey: fmt.Sprintf("endpoint:%d:hostname", endpointID)}
		if err := publish(ctx, tx, now, event, &observation.Alerts); err != nil {
			return observation, err
		}
	}
	if observation.Changed {
		event := notify.Event{Type: notify.EventChanged, Severity: theme.SeverityMedium, Title: "Certificate changed on " + address + suffix, RecordKind: KindEndpoint, RecordID: endpointID, DedupKey: fmt.Sprintf("endpoint:%d:changed:%d", endpointID, observation.CertificateID)}
		if err := publish(ctx, tx, now, event, &observation.Alerts); err != nil {
			return observation, err
		}
	}
	expiring, err := evaluateExpiry(ctx, tx, now, policy.thresholds())
	observation.Alerts += expiring
	return observation, err
}

func (s *Service) Record(ctx context.Context, endpointID int64, result tlscheck.Result, policy Policy) (Observation, error) {
	var observation Observation
	err := s.inTx(ctx, func(tx *sql.Tx) error {
		var err error
		observation, err = record(ctx, tx, s.now(), endpointID, result, policy)
		return err
	})
	return observation, err
}

type ImportedCertificates struct {
	Added   int
	Known   int
	Names   []string
	Alerts  int
	DryRun  bool
	Skipped int
}

func (r ImportedCertificates) Summary() string {
	verb := "imported"
	if r.DryRun {
		verb = "would import"
	}
	return fmt.Sprintf("%s %d certificates, %d already tracked", verb, r.Added, r.Known)
}

func (s *Service) ImportCertificates(ctx context.Context, data []byte, dryRun bool, policy Policy) (ImportedCertificates, error) {
	result := ImportedCertificates{DryRun: dryRun}
	certificates, err := tlscheck.ParsePEM(data)
	if err != nil {
		return result, err
	}
	err = s.inTx(ctx, func(tx *sql.Tx) error {
		result = ImportedCertificates{DryRun: dryRun}
		now := s.now()
		for _, certificate := range certificates {
			info := tlscheck.Describe(certificate)
			var id int64
			err := tx.QueryRowContext(ctx, sqlCertificateKnown, info.Fingerprint).Scan(&id)
			switch {
			case err == nil:
				result.Known++
				continue
			case !errors.Is(err, sql.ErrNoRows):
				return err
			}
			result.Added++
			result.Names = append(result.Names, info.DisplayName())
			if dryRun {
				continue
			}
			if _, err := upsertCertificate(ctx, tx, certificate, stamp(now), SourceImported, false); err != nil {
				return err
			}
		}
		if dryRun {
			return nil
		}
		var err error
		result.Alerts, err = evaluateExpiry(ctx, tx, now, policy.thresholds())
		return err
	})
	return result, err
}
