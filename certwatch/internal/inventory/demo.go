package inventory

import (
	"context"
	"crypto/x509"
	"database/sql"
	"time"

	"github.com/NexzaDev/certwatch/internal/certgen"
	"github.com/NexzaDev/certwatch/internal/tlscheck"
)

const (
	sqlDemoEndpoints          = "SELECT count(*) FROM endpoints WHERE is_demo = 1"
	sqlDemoCertificates       = "SELECT count(*) FROM certificates WHERE is_demo = 1"
	sqlDemoDeleteAlerts       = "DELETE FROM notifications WHERE (record_kind = 'endpoint' AND record_id IN (SELECT id FROM endpoints WHERE is_demo = 1)) OR (record_kind = 'certificate' AND record_id IN (SELECT id FROM certificates WHERE is_demo = 1))"
	sqlDemoDeleteEndpoints    = "DELETE FROM endpoints WHERE is_demo = 1"
	sqlDemoDeleteCertificates = "DELETE FROM certificates WHERE is_demo = 1"
)

type demoCase struct {
	host      string
	note      string
	names     []string
	validFrom int
	validTo   int
	chain     string
	selfSign  bool
	weakKey   bool
	version   string
	accepted  []string
	outcome   string
	failure   string
	renewed   bool
	disabled  bool
}

var demoCases = []demoCase{
	{host: "shop.demo.invalid", note: "Demo endpoint. Healthy certificate with a long remaining lifetime.", validFrom: -60, validTo: 215, version: "TLS 1.3", accepted: []string{"TLS 1.2", "TLS 1.3"}},
	{host: "api.demo.invalid", note: "Demo endpoint. The certificate expires within a week.", validFrom: -85, validTo: 5, version: "TLS 1.3", accepted: []string{"TLS 1.2", "TLS 1.3"}},
	{host: "mail.demo.invalid", note: "Demo endpoint. The certificate expires within a month and old protocol versions are accepted.", validFrom: -65, validTo: 25, version: "TLS 1.2", accepted: []string{"TLS 1.0", "TLS 1.1", "TLS 1.2"}},
	{host: "intranet.demo.invalid", note: "Demo endpoint. The certificate expires within three months.", validFrom: -10, validTo: 80, version: "TLS 1.3", accepted: []string{"TLS 1.2", "TLS 1.3"}},
	{host: "legacy.demo.invalid", note: "Demo endpoint. The certificate expired three days ago.", validFrom: -93, validTo: -3, chain: "expired", version: "TLS 1.2", accepted: []string{"TLS 1.2"}},
	{host: "blog.demo.invalid", note: "Demo endpoint. The certificate was issued for another host name.", names: []string{"www.demo.invalid"}, validFrom: -30, validTo: 150, version: "TLS 1.3", accepted: []string{"TLS 1.2", "TLS 1.3"}},
	{host: "printer.demo.invalid", note: "Demo endpoint. A self-signed certificate with a weak key.", validFrom: -400, validTo: 330, selfSign: true, weakKey: true, version: "TLS 1.2", accepted: []string{"TLS 1.2"}},
	{host: "staging.demo.invalid", note: "Demo endpoint. The chain leads to a private authority that this computer does not trust.", validFrom: -20, validTo: 70, chain: "untrusted", version: "TLS 1.3", accepted: []string{"TLS 1.2", "TLS 1.3"}},
	{host: "portal.demo.invalid", note: "Demo endpoint. The certificate was renewed between two observations.", validFrom: -2, validTo: 88, version: "TLS 1.3", accepted: []string{"TLS 1.2", "TLS 1.3"}, renewed: true},
	{host: "backup.demo.invalid", note: "Demo endpoint. The host did not answer.", outcome: tlscheck.OutcomeUnreachable, failure: "no answer within 10s; the host may be down, filtered, or need a longer timeout"},
	{host: "archive.demo.invalid", note: "Demo endpoint. Disabled, so it is left out of checks and counts.", disabled: true},
}

func (s *Service) DemoCount(ctx context.Context) (int, error) {
	var endpoints, certificates int
	if err := s.db.QueryRowContext(ctx, sqlDemoEndpoints).Scan(&endpoints); err != nil {
		return 0, err
	}
	err := s.db.QueryRowContext(ctx, sqlDemoCertificates).Scan(&certificates)
	return endpoints + certificates, err
}

type demoAuthorities struct {
	root    *certgen.Authority
	issuing *certgen.Authority
	private *certgen.Authority
}

func newDemoAuthorities(now time.Time) (demoAuthorities, error) {
	var a demoAuthorities
	var err error
	if a.root, err = certgen.NewAuthority("CertWatch Demo Root (demo)", now.AddDate(-2, 0, 0), now.AddDate(8, 0, 0)); err != nil {
		return a, err
	}
	if a.issuing, err = a.root.Intermediate("CertWatch Demo Issuing CA (demo)", now.AddDate(-1, 0, 0), now.AddDate(4, 0, 0)); err != nil {
		return a, err
	}
	a.private, err = certgen.NewAuthority("Private Lab Authority (demo)", now.AddDate(-1, 0, 0), now.AddDate(4, 0, 0))
	return a, err
}

func (c demoCase) result(a demoAuthorities, now time.Time, observed time.Time, shift int) (tlscheck.Result, error) {
	target := tlscheck.Target{Host: c.host, Port: DefaultPort}
	result := tlscheck.Result{Target: target, ObservedAt: observed, Duration: 120 * time.Millisecond, Outcome: tlscheck.OutcomeOK, ChainStatus: tlscheck.StatusValid, HostnameStatus: tlscheck.HostnameMatch,
		Version: c.version, Accepted: c.accepted, ProtocolsTested: len(c.accepted) > 0}
	if c.outcome != "" {
		result.Outcome, result.Error = c.outcome, c.failure
		result.ChainStatus, result.HostnameStatus = tlscheck.StatusUnknown, tlscheck.StatusUnknown
		result.Version, result.Accepted, result.ProtocolsTested = "", nil, false
		return result, nil
	}
	if c.version == "TLS 1.3" {
		result.CipherSuite = "TLS_AES_128_GCM_SHA256"
	} else {
		result.CipherSuite = "TLS_ECDHE_ECDSA_WITH_AES_128_GCM_SHA256"
	}
	names := c.names
	if names == nil {
		names = []string{c.host}
	}
	request := certgen.Request{CommonName: names[0], Names: names, NotBefore: now.AddDate(0, 0, c.validFrom+shift), NotAfter: now.AddDate(0, 0, c.validTo+shift)}
	if c.weakKey {
		request.RSABits = 1024
	}
	switch {
	case c.selfSign:
		issued, err := certgen.SelfSigned(request)
		if err != nil {
			return result, err
		}
		result.Presented = []*x509.Certificate{issued.Certificate}
		result.ChainStatus, result.ChainError = tlscheck.StatusInvalid, "the chain does not lead to a trusted root certificate"
	case c.chain == "untrusted":
		issued, err := a.private.Issue(request)
		if err != nil {
			return result, err
		}
		result.Presented = []*x509.Certificate{issued.Certificate}
		result.ChainStatus, result.ChainError = tlscheck.StatusInvalid, "the chain does not lead to a trusted root certificate"
	default:
		issued, err := a.issuing.Issue(request)
		if err != nil {
			return result, err
		}
		result.Presented = []*x509.Certificate{issued.Certificate, a.issuing.Certificate}
		if c.chain == "expired" {
			result.ChainStatus, result.ChainError = tlscheck.StatusInvalid, "a certificate in the chain has expired or is not valid yet"
		} else {
			result.Verified = []*x509.Certificate{issued.Certificate, a.issuing.Certificate, a.root.Certificate}
		}
	}
	if err := result.Presented[0].VerifyHostname(c.host); err != nil {
		result.HostnameStatus, result.HostnameError = tlscheck.HostnameMismatch, "certificate is valid for "+names[0]+", not "+c.host
	}
	return result, nil
}

func (s *Service) LoadDemo(ctx context.Context, policy Policy) (int, error) {
	existing, err := s.DemoCount(ctx)
	if err != nil || existing > 0 {
		return 0, err
	}
	now := s.now()
	authorities, err := newDemoAuthorities(now)
	if err != nil {
		return 0, err
	}
	type prepared struct {
		demo    demoCase
		results []tlscheck.Result
	}
	var plan []prepared
	for _, demo := range demoCases {
		item := prepared{demo: demo}
		if !demo.disabled {
			if demo.renewed {
				earlier, err := demo.result(authorities, now, now.Add(-26*time.Hour), -80)
				if err != nil {
					return 0, err
				}
				item.results = append(item.results, earlier)
			}
			latest, err := demo.result(authorities, now, now.Add(-2*time.Hour), 0)
			if err != nil {
				return 0, err
			}
			item.results = append(item.results, latest)
		}
		plan = append(plan, item)
	}
	added := 0
	err = s.inTx(ctx, func(tx *sql.Tx) error {
		added = 0
		for _, item := range plan {
			endpoint, err := createEndpoint(ctx, tx, now, EndpointInput{Host: item.demo.host, Port: DefaultPort, Note: item.demo.note, Enabled: !item.demo.disabled}, true)
			if err != nil {
				return err
			}
			added++
			for _, result := range item.results {
				if _, err := record(ctx, tx, now, endpoint.ID, result, policy); err != nil {
					return err
				}
			}
		}
		return nil
	})
	return added, err
}

func (s *Service) RemoveDemo(ctx context.Context, dryRun bool) (int, error) {
	count, err := s.DemoCount(ctx)
	if err != nil || dryRun || count == 0 {
		return count, err
	}
	err = s.inTx(ctx, func(tx *sql.Tx) error {
		if _, err := tx.ExecContext(ctx, sqlDemoDeleteAlerts); err != nil {
			return err
		}
		if _, err := tx.ExecContext(ctx, sqlDemoDeleteEndpoints); err != nil {
			return err
		}
		_, err := tx.ExecContext(ctx, sqlDemoDeleteCertificates)
		return err
	})
	return count, err
}
