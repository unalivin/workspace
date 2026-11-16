package tlscheck

import (
	"context"
	"crypto/tls"
	"crypto/x509"
	"errors"
	"fmt"
	"net"
	"strconv"
	"strings"
	"time"

	"github.com/unalivin/certwatch/internal/theme"
)

const (
	OutcomeOK              = "ok"
	OutcomeUnreachable     = "unreachable"
	OutcomeHandshakeFailed = "handshake_failed"

	StatusValid   = "valid"
	StatusInvalid = "invalid"
	StatusUnknown = "unknown"

	HostnameMatch    = "match"
	HostnameMismatch = "mismatch"

	MinTimeout     = time.Second
	MaxTimeout     = time.Minute
	DefaultTimeout = 10 * time.Second

	maxErrorLength = 300
)

var versions = []uint16{tls.VersionTLS10, tls.VersionTLS11, tls.VersionTLS12, tls.VersionTLS13}

type Target struct {
	Host       string
	Port       int
	ServerName string
}

func (t Target) Address() string {
	return net.JoinHostPort(t.Host, strconv.Itoa(t.Port))
}

func (t Target) Name() string {
	if t.ServerName != "" {
		return t.ServerName
	}
	return t.Host
}

type DialFunc func(ctx context.Context, network, address string) (net.Conn, error)

type Options struct {
	Timeout   time.Duration
	Roots     *x509.CertPool
	Now       func() time.Time
	Dial      DialFunc
	Protocols bool
}

type Result struct {
	Target          Target
	ObservedAt      time.Time
	Duration        time.Duration
	Outcome         string
	Error           string
	Presented       []*x509.Certificate
	Verified        []*x509.Certificate
	ChainStatus     string
	ChainError      string
	HostnameStatus  string
	HostnameError   string
	Version         string
	CipherSuite     string
	WeakCipher      bool
	Accepted        []string
	ProtocolsTested bool
}

func (r Result) Leaf() *x509.Certificate {
	if len(r.Presented) == 0 {
		return nil
	}
	return r.Presented[0]
}

func VersionName(version uint16) string {
	switch version {
	case tls.VersionTLS10:
		return "TLS 1.0"
	case tls.VersionTLS11:
		return "TLS 1.1"
	case tls.VersionTLS12:
		return "TLS 1.2"
	case tls.VersionTLS13:
		return "TLS 1.3"
	}
	return ""
}

func ClampTimeout(timeout time.Duration) time.Duration {
	switch {
	case timeout <= 0:
		return DefaultTimeout
	case timeout < MinTimeout:
		return MinTimeout
	case timeout > MaxTimeout:
		return MaxTimeout
	}
	return timeout
}

func shorten(text string) string {
	text = Printable(strings.Join(strings.Fields(text), " "))
	if runes := []rune(text); len(runes) > maxErrorLength {
		return string(runes[:maxErrorLength]) + "…"
	}
	return text
}

type dialError struct{ err error }

func (e *dialError) Error() string { return e.err.Error() }
func (e *dialError) Unwrap() error { return e.err }

func describeDial(err error, timeout time.Duration) string {
	var dns *net.DNSError
	switch {
	case errors.As(err, &dns) && dns.IsNotFound:
		return "the host name could not be resolved; check the spelling and the DNS of this computer"
	case errors.Is(err, context.DeadlineExceeded) || isTimeout(err):
		return fmt.Sprintf("no answer within %s; the host may be down, filtered, or need a longer timeout", timeout)
	case strings.Contains(err.Error(), "refused"):
		return "the connection was refused; nothing is listening on this port"
	}
	return shorten(err.Error())
}

func isTimeout(err error) bool {
	var timeout net.Error
	return errors.As(err, &timeout) && timeout.Timeout()
}

func describeChain(err error) string {
	var unknown x509.UnknownAuthorityError
	var invalid x509.CertificateInvalidError
	switch {
	case errors.As(err, &unknown):
		return "the chain does not lead to a trusted root certificate"
	case errors.As(err, &invalid) && invalid.Reason == x509.Expired:
		return "a certificate in the chain has expired or is not valid yet"
	case errors.As(err, &invalid) && invalid.Reason == x509.IncompatibleUsage:
		return "the certificate is not allowed to identify a server"
	}
	return shorten(strings.TrimPrefix(err.Error(), "x509: "))
}

func VerifyChain(presented []*x509.Certificate, roots *x509.CertPool, now time.Time) ([]*x509.Certificate, error) {
	if len(presented) == 0 {
		return nil, errors.New("the server presented no certificate")
	}
	intermediates := x509.NewCertPool()
	for _, certificate := range presented[1:] {
		intermediates.AddCert(certificate)
	}
	chains, err := presented[0].Verify(x509.VerifyOptions{
		Roots:         roots,
		Intermediates: intermediates,
		CurrentTime:   now,
		KeyUsages:     []x509.ExtKeyUsage{x509.ExtKeyUsageServerAuth},
	})
	if err != nil {
		return nil, err
	}
	return chains[0], nil
}

func (r *Result) evaluate(roots *x509.CertPool, now time.Time) {
	verified, err := VerifyChain(r.Presented, roots, now)
	if err != nil {
		r.ChainStatus, r.ChainError = StatusInvalid, describeChain(err)
	} else {
		r.ChainStatus, r.Verified = StatusValid, verified
	}
	if err := r.Presented[0].VerifyHostname(r.Target.Name()); err != nil {
		r.HostnameStatus = HostnameMismatch
		r.HostnameError = shorten(strings.TrimPrefix(err.Error(), "x509: "))
	} else {
		r.HostnameStatus = HostnameMatch
	}
}

func handshake(ctx context.Context, target Target, options Options, timeout time.Duration, version uint16) (tls.ConnectionState, error) {
	ctx, cancel := context.WithTimeout(ctx, timeout)
	defer cancel()
	dial := options.Dial
	if dial == nil {
		dial = (&net.Dialer{}).DialContext
	}
	connection, err := dial(ctx, "tcp", target.Address())
	if err != nil {
		return tls.ConnectionState{}, &dialError{err: err}
	}
	defer func() { _ = connection.Close() }()
	config := &tls.Config{ServerName: target.Name(), RootCAs: options.Roots, MinVersion: tls.VersionTLS10, Time: options.Now}
	if version != 0 {
		config.MinVersion, config.MaxVersion = version, version
	}
	client := tls.Client(connection, config)
	err = client.HandshakeContext(ctx)
	return client.ConnectionState(), err
}

func weakSuite(id uint16) bool {
	for _, suite := range tls.InsecureCipherSuites() {
		if suite.ID == id {
			return true
		}
	}
	return false
}

func (r *Result) negotiated(state tls.ConnectionState) {
	r.Version = VersionName(state.Version)
	if state.CipherSuite != 0 {
		r.CipherSuite = tls.CipherSuiteName(state.CipherSuite)
		r.WeakCipher = weakSuite(state.CipherSuite)
	}
}

func accepts(err error) (accepted, reachable bool) {
	var dial *dialError
	var verification *tls.CertificateVerificationError
	switch {
	case err == nil, errors.As(err, &verification):
		return true, true
	case errors.As(err, &dial):
		return false, false
	}
	return false, true
}

func Check(ctx context.Context, target Target, options Options) Result {
	if options.Now == nil {
		options.Now = time.Now
	}
	timeout := ClampTimeout(options.Timeout)
	result := Result{Target: target, ObservedAt: options.Now().UTC(), ChainStatus: StatusUnknown, HostnameStatus: StatusUnknown}
	started := time.Now()
	state, err := handshake(ctx, target, options, timeout, 0)
	var dial *dialError
	var verification *tls.CertificateVerificationError
	switch {
	case err == nil:
		result.Outcome = OutcomeOK
		result.Presented = state.PeerCertificates
		result.negotiated(state)
		result.ChainStatus, result.HostnameStatus = StatusValid, HostnameMatch
		if len(state.VerifiedChains) > 0 {
			result.Verified = state.VerifiedChains[0]
		}
	case errors.As(err, &verification):
		result.Outcome = OutcomeOK
		result.Presented = verification.UnverifiedCertificates
		result.negotiated(state)
		if len(result.Presented) == 0 {
			result.Outcome, result.Error = OutcomeHandshakeFailed, "the server presented no certificate"
		} else {
			result.evaluate(options.Roots, options.Now())
		}
	case errors.As(err, &dial):
		result.Outcome, result.Error = OutcomeUnreachable, describeDial(err, timeout)
	case errors.Is(err, context.DeadlineExceeded) || isTimeout(err):
		result.Outcome, result.Error = OutcomeHandshakeFailed, fmt.Sprintf("the TLS handshake did not finish within %s", timeout)
	default:
		result.Outcome, result.Error = OutcomeHandshakeFailed, shorten(strings.TrimPrefix(err.Error(), "remote error: "))
	}
	if options.Protocols && result.Outcome == OutcomeOK {
		result.ProtocolsTested = true
		for _, version := range versions {
			name := VersionName(version)
			if name == result.Version {
				result.Accepted = append(result.Accepted, name)
				continue
			}
			_, err := handshake(ctx, target, options, timeout, version)
			accepted, reachable := accepts(err)
			if !reachable {
				result.ProtocolsTested = false
				result.Accepted = nil
				break
			}
			if accepted {
				result.Accepted = append(result.Accepted, name)
			}
		}
	}
	result.Duration = time.Since(started)
	return result
}

type Finding struct {
	Code     string
	Severity theme.Severity
	Detail   string
}

const (
	FindingExpired        = "certificate.expired"
	FindingNotYetValid    = "certificate.not_yet_valid"
	FindingChainUntrusted = "chain.untrusted"
	FindingHostname       = "hostname.mismatch"
	FindingSelfSigned     = "certificate.self_signed"
	FindingWeakKey        = "key.weak"
	FindingWeakSignature  = "signature.weak"
	FindingLongValidity   = "validity.long"
	FindingOldProtocol    = "protocol.old"
	FindingNoModern       = "protocol.no_tls13"
	FindingWeakCipher     = "cipher.weak"

	maxPublicValidity = 398 * 24 * time.Hour
)

var findingTitles = map[string]string{
	FindingExpired:        "Certificate expired",
	FindingNotYetValid:    "Certificate not valid yet",
	FindingChainUntrusted: "Chain not trusted",
	FindingHostname:       "Host name mismatch",
	FindingSelfSigned:     "Self-signed certificate",
	FindingWeakKey:        "Weak key",
	FindingWeakSignature:  "Weak signature algorithm",
	FindingLongValidity:   "Long validity period",
	FindingOldProtocol:    "Old TLS version accepted",
	FindingNoModern:       "TLS 1.3 not accepted",
	FindingWeakCipher:     "Weak cipher suite negotiated",
}

func FindingCodes() []string {
	return []string{FindingExpired, FindingNotYetValid, FindingChainUntrusted, FindingHostname, FindingSelfSigned, FindingWeakKey, FindingWeakSignature, FindingLongValidity, FindingOldProtocol, FindingNoModern, FindingWeakCipher}
}

func FindingTitle(code string) string {
	if title, found := findingTitles[code]; found {
		return title
	}
	return code
}

func weakSignature(algorithm x509.SignatureAlgorithm) bool {
	switch algorithm {
	case x509.MD2WithRSA, x509.MD5WithRSA, x509.SHA1WithRSA, x509.DSAWithSHA1, x509.ECDSAWithSHA1:
		return true
	}
	return false
}

func weakKey(info Info) bool {
	switch info.KeyAlgorithm {
	case "RSA":
		return info.KeyBits < 2048
	case "ECDSA":
		return info.KeyBits < 224
	}
	return false
}

func contains(list []string, wanted string) bool {
	for _, item := range list {
		if item == wanted {
			return true
		}
	}
	return false
}

func Findings(result Result, now time.Time) []Finding {
	leaf := result.Leaf()
	if result.Outcome != OutcomeOK || leaf == nil {
		return nil
	}
	info := Describe(leaf)
	var findings []Finding
	add := func(code string, severity theme.Severity, format string, arguments ...any) {
		findings = append(findings, Finding{Code: code, Severity: severity, Detail: fmt.Sprintf(format, arguments...)})
	}
	switch {
	case now.After(leaf.NotAfter):
		add(FindingExpired, theme.SeverityCritical, "The certificate expired on %s. Clients refuse the connection until it is replaced.", leaf.NotAfter.UTC().Format("2006-01-02"))
	case now.Before(leaf.NotBefore):
		add(FindingNotYetValid, theme.SeverityHigh, "The certificate becomes valid on %s. Check the clock of the server and of this computer.", leaf.NotBefore.UTC().Format("2006-01-02"))
	}
	if info.SelfSigned {
		add(FindingSelfSigned, theme.SeverityMedium, "The certificate is signed by its own key, so clients cannot verify who issued it unless they trust it directly.")
	} else if result.ChainStatus == StatusInvalid {
		add(FindingChainUntrusted, theme.SeverityHigh, "Chain validation failed: %s.", result.ChainError)
	}
	if result.HostnameStatus == HostnameMismatch {
		add(FindingHostname, theme.SeverityHigh, "The certificate is not valid for %s. It names: %s.", result.Target.Name(), namesOrNone(info.Names))
	}
	if weakKey(info) {
		add(FindingWeakKey, theme.SeverityHigh, "The %s key has %d bits. Use RSA with at least 2048 bits or an elliptic curve key.", info.KeyAlgorithm, info.KeyBits)
	}
	for position, certificate := range result.Presented {
		if position > 0 && selfSigned(certificate) {
			continue
		}
		if weakSignature(certificate.SignatureAlgorithm) {
			add(FindingWeakSignature, theme.SeverityHigh, "Certificate %d of the chain is signed with %s, which is no longer considered safe.", position+1, certificate.SignatureAlgorithm)
			break
		}
	}
	if !info.SelfSigned && leaf.NotAfter.Sub(leaf.NotBefore) > maxPublicValidity {
		add(FindingLongValidity, theme.SeverityInfo, "The certificate is valid for %d days. Publicly trusted certificates are limited to 398 days; private authorities may allow more.", int(leaf.NotAfter.Sub(leaf.NotBefore)/(24*time.Hour)))
	}
	if result.ProtocolsTested {
		var old []string
		for _, name := range []string{"TLS 1.0", "TLS 1.1"} {
			if contains(result.Accepted, name) {
				old = append(old, name)
			}
		}
		if len(old) > 0 {
			add(FindingOldProtocol, theme.SeverityMedium, "The server accepts %s. These versions are deprecated; turn them off on the server.", strings.Join(old, " and "))
		}
		if !contains(result.Accepted, "TLS 1.3") {
			add(FindingNoModern, theme.SeverityLow, "The server did not accept TLS 1.3. Accepted: %s.", namesOrNone(result.Accepted))
		}
	}
	if result.WeakCipher {
		add(FindingWeakCipher, theme.SeverityMedium, "The negotiated cipher suite %s is considered weak.", result.CipherSuite)
	}
	return findings
}

func namesOrNone(names []string) string {
	if len(names) == 0 {
		return "none"
	}
	if len(names) > 8 {
		return strings.Join(names[:8], ", ") + fmt.Sprintf(" and %d more", len(names)-8)
	}
	return strings.Join(names, ", ")
}
