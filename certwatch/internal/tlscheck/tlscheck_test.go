package tlscheck

import (
	"context"
	"crypto/tls"
	"crypto/x509"
	"net"
	"strings"
	"testing"
	"time"
	"unicode"
	"unicode/utf8"

	"github.com/unalivin/certwatch/internal/certgen"
	"github.com/unalivin/certwatch/internal/theme"
)

type pki struct {
	root         *certgen.Authority
	intermediate *certgen.Authority
	now          time.Time
}

func newPKI(t *testing.T) pki {
	t.Helper()
	now := time.Now().UTC().Truncate(time.Second)
	root, err := certgen.NewAuthority("Fixture Root", now.AddDate(-1, 0, 0), now.AddDate(5, 0, 0))
	if err != nil {
		t.Fatal(err)
	}
	intermediate, err := root.Intermediate("Fixture Issuing", now.AddDate(-1, 0, 0), now.AddDate(3, 0, 0))
	if err != nil {
		t.Fatal(err)
	}
	return pki{root: root, intermediate: intermediate, now: now}
}

func (p pki) issue(t *testing.T, request certgen.Request) tls.Certificate {
	t.Helper()
	if request.NotBefore.IsZero() {
		request.NotBefore = p.now.AddDate(0, 0, -10)
	}
	if request.NotAfter.IsZero() {
		request.NotAfter = p.now.AddDate(0, 0, 80)
	}
	issued, err := p.intermediate.Issue(request)
	if err != nil {
		t.Fatal(err)
	}
	return tls.Certificate{Certificate: [][]byte{issued.Certificate.Raw, p.intermediate.Certificate.Raw}, PrivateKey: issued.Key}
}

func serve(t *testing.T, config *tls.Config) Target {
	t.Helper()
	listener, err := tls.Listen("tcp", "127.0.0.1:0", config)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = listener.Close() })
	go func() {
		for {
			connection, err := listener.Accept()
			if err != nil {
				return
			}
			go func() {
				_ = connection.SetDeadline(time.Now().Add(5 * time.Second))
				_ = connection.(*tls.Conn).Handshake()
				_ = connection.Close()
			}()
		}
	}()
	return Target{Host: "127.0.0.1", Port: listener.Addr().(*net.TCPAddr).Port}
}

func codes(findings []Finding) string {
	var list []string
	for _, finding := range findings {
		list = append(list, finding.Code)
	}
	return strings.Join(list, " ")
}

func TestCheckAcceptsATrustedChain(t *testing.T) {
	p := newPKI(t)
	target := serve(t, &tls.Config{Certificates: []tls.Certificate{p.issue(t, certgen.Request{CommonName: "app.fixture.test", Names: []string{"app.fixture.test", "127.0.0.1"}})}})
	result := Check(context.Background(), target, Options{Roots: p.root.Pool(), Timeout: 5 * time.Second})
	if result.Outcome != OutcomeOK || result.ChainStatus != StatusValid || result.HostnameStatus != HostnameMatch {
		t.Fatalf("unexpected result: %+v", result)
	}
	if len(result.Presented) != 2 || len(result.Verified) != 3 {
		t.Fatalf("presented %d verified %d", len(result.Presented), len(result.Verified))
	}
	if result.Version != "TLS 1.3" || result.CipherSuite == "" {
		t.Fatalf("version %q suite %q", result.Version, result.CipherSuite)
	}
	if got := codes(Findings(result, p.now)); got != "" {
		t.Fatalf("unexpected findings: %s", got)
	}
}

func TestCheckKeepsVerificationOnAndStillRecordsAnUntrustedChain(t *testing.T) {
	p := newPKI(t)
	other := newPKI(t)
	target := serve(t, &tls.Config{Certificates: []tls.Certificate{p.issue(t, certgen.Request{CommonName: "app.fixture.test", Names: []string{"127.0.0.1"}})}})
	result := Check(context.Background(), target, Options{Roots: other.root.Pool(), Timeout: 5 * time.Second})
	if result.Outcome != OutcomeOK || result.ChainStatus != StatusInvalid || result.HostnameStatus != HostnameMatch {
		t.Fatalf("unexpected result: %+v", result)
	}
	if result.Leaf() == nil || result.Leaf().Subject.CommonName != "app.fixture.test" {
		t.Fatalf("the presented certificate was not recorded")
	}
	if !strings.Contains(result.ChainError, "trusted root") {
		t.Fatalf("chain error %q", result.ChainError)
	}
	if got := codes(Findings(result, p.now)); got != FindingChainUntrusted {
		t.Fatalf("findings: %s", got)
	}
}

func TestCheckReportsAHostnameMismatch(t *testing.T) {
	p := newPKI(t)
	target := serve(t, &tls.Config{Certificates: []tls.Certificate{p.issue(t, certgen.Request{CommonName: "other.fixture.test", Names: []string{"other.fixture.test"}})}})
	target.ServerName = "app.fixture.test"
	result := Check(context.Background(), target, Options{Roots: p.root.Pool(), Timeout: 5 * time.Second})
	if result.Outcome != OutcomeOK || result.ChainStatus != StatusValid || result.HostnameStatus != HostnameMismatch {
		t.Fatalf("unexpected result: %+v", result)
	}
	findings := Findings(result, p.now)
	if codes(findings) != FindingHostname || !strings.Contains(findings[0].Detail, "other.fixture.test") {
		t.Fatalf("findings: %+v", findings)
	}
}

func TestCheckReportsAnExpiredCertificate(t *testing.T) {
	p := newPKI(t)
	target := serve(t, &tls.Config{Certificates: []tls.Certificate{p.issue(t, certgen.Request{CommonName: "old.fixture.test", Names: []string{"127.0.0.1"}, NotBefore: p.now.AddDate(0, 0, -90), NotAfter: p.now.AddDate(0, 0, -2)})}})
	result := Check(context.Background(), target, Options{Roots: p.root.Pool(), Timeout: 5 * time.Second})
	if result.Outcome != OutcomeOK || result.ChainStatus != StatusInvalid {
		t.Fatalf("unexpected result: %+v", result)
	}
	findings := Findings(result, p.now)
	if codes(findings) != FindingExpired+" "+FindingChainUntrusted || findings[0].Severity != theme.SeverityCritical {
		t.Fatalf("findings: %+v", findings)
	}
}

func TestCheckReportsASelfSignedCertificateAndAWeakKey(t *testing.T) {
	now := time.Now()
	issued, err := certgen.SelfSigned(certgen.Request{CommonName: "self.fixture.test", Names: []string{"127.0.0.1"}, NotBefore: now.AddDate(0, 0, -1), NotAfter: now.AddDate(0, 0, 30), RSABits: 1024})
	if err != nil {
		t.Fatal(err)
	}
	target := serve(t, &tls.Config{Certificates: []tls.Certificate{{Certificate: [][]byte{issued.Certificate.Raw}, PrivateKey: issued.Key}}})
	result := Check(context.Background(), target, Options{Roots: x509.NewCertPool(), Timeout: 5 * time.Second})
	if result.Outcome != OutcomeOK {
		t.Fatalf("unexpected result: %+v", result)
	}
	if got := codes(Findings(result, now)); got != FindingSelfSigned+" "+FindingWeakKey {
		t.Fatalf("findings: %s", got)
	}
}

func TestCheckObservesAcceptedProtocolVersions(t *testing.T) {
	p := newPKI(t)
	certificate := p.issue(t, certgen.Request{CommonName: "legacy.fixture.test", Names: []string{"127.0.0.1"}})
	target := serve(t, &tls.Config{Certificates: []tls.Certificate{certificate}, MinVersion: tls.VersionTLS10, MaxVersion: tls.VersionTLS12})
	result := Check(context.Background(), target, Options{Roots: p.root.Pool(), Timeout: 5 * time.Second, Protocols: true})
	if !result.ProtocolsTested || strings.Join(result.Accepted, ",") != "TLS 1.0,TLS 1.1,TLS 1.2" {
		t.Fatalf("accepted %v tested %v", result.Accepted, result.ProtocolsTested)
	}
	if result.Version != "TLS 1.2" {
		t.Fatalf("version %q", result.Version)
	}
	if got := codes(Findings(result, p.now)); got != FindingOldProtocol+" "+FindingNoModern {
		t.Fatalf("findings: %s", got)
	}
	modern := serve(t, &tls.Config{Certificates: []tls.Certificate{certificate}, MinVersion: tls.VersionTLS12})
	result = Check(context.Background(), modern, Options{Roots: p.root.Pool(), Timeout: 5 * time.Second, Protocols: true})
	if strings.Join(result.Accepted, ",") != "TLS 1.2,TLS 1.3" || len(Findings(result, p.now)) != 0 {
		t.Fatalf("accepted %v findings %v", result.Accepted, Findings(result, p.now))
	}
	plain := Check(context.Background(), modern, Options{Roots: p.root.Pool(), Timeout: 5 * time.Second})
	if plain.ProtocolsTested || len(plain.Accepted) != 0 {
		t.Fatalf("protocol versions were probed without being asked for")
	}
}

func TestCheckReportsAnUnreachableHost(t *testing.T) {
	listener, err := net.Listen("tcp", "127.0.0.1:0")
	if err != nil {
		t.Fatal(err)
	}
	port := listener.Addr().(*net.TCPAddr).Port
	_ = listener.Close()
	result := Check(context.Background(), Target{Host: "127.0.0.1", Port: port}, Options{Timeout: 2 * time.Second})
	if result.Outcome != OutcomeUnreachable || result.Error == "" || result.ChainStatus != StatusUnknown {
		t.Fatalf("unexpected result: %+v", result)
	}
	if Findings(result, time.Now()) != nil {
		t.Fatalf("an unreachable host must not produce certificate findings")
	}
}

func TestCheckReportsAServerThatDoesNotSpeakTLS(t *testing.T) {
	listener, err := net.Listen("tcp", "127.0.0.1:0")
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = listener.Close() })
	go func() {
		for {
			connection, err := listener.Accept()
			if err != nil {
				return
			}
			_, _ = connection.Write([]byte("220 plain text service ready\r\n"))
			_ = connection.Close()
		}
	}()
	result := Check(context.Background(), Target{Host: "127.0.0.1", Port: listener.Addr().(*net.TCPAddr).Port}, Options{Timeout: 2 * time.Second})
	if result.Outcome != OutcomeHandshakeFailed || result.Error == "" {
		t.Fatalf("unexpected result: %+v", result)
	}
}

func TestCheckStopsAtTheTimeout(t *testing.T) {
	listener, err := net.Listen("tcp", "127.0.0.1:0")
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = listener.Close() })
	go func() {
		for {
			connection, err := listener.Accept()
			if err != nil {
				return
			}
			t.Cleanup(func() { _ = connection.Close() })
		}
	}()
	started := time.Now()
	result := Check(context.Background(), Target{Host: "127.0.0.1", Port: listener.Addr().(*net.TCPAddr).Port}, Options{Timeout: time.Second})
	if result.Outcome != OutcomeHandshakeFailed || !strings.Contains(result.Error, "1s") {
		t.Fatalf("unexpected result: %+v", result)
	}
	if time.Since(started) > 4*time.Second {
		t.Fatalf("the check ran for %s", time.Since(started))
	}
}

func TestCheckUsesTheGivenDialerAndNothingElse(t *testing.T) {
	var dialed []string
	options := Options{Timeout: time.Second, Dial: func(ctx context.Context, network, address string) (net.Conn, error) {
		dialed = append(dialed, network+" "+address)
		return nil, &net.DNSError{Err: "no such host", Name: "nowhere.fixture.test", IsNotFound: true}
	}}
	result := Check(context.Background(), Target{Host: "nowhere.fixture.test", Port: 8443}, options)
	if strings.Join(dialed, "|") != "tcp nowhere.fixture.test:8443" {
		t.Fatalf("dialed %v", dialed)
	}
	if result.Outcome != OutcomeUnreachable || !strings.Contains(result.Error, "could not be resolved") {
		t.Fatalf("unexpected result: %+v", result)
	}
}

func TestClampTimeout(t *testing.T) {
	cases := map[time.Duration]time.Duration{0: DefaultTimeout, time.Millisecond: MinTimeout, 5 * time.Second: 5 * time.Second, time.Hour: MaxTimeout}
	for in, want := range cases {
		if got := ClampTimeout(in); got != want {
			t.Errorf("ClampTimeout(%s) = %s, want %s", in, got, want)
		}
	}
}

func TestDaysLeftRoundsTowardsThePast(t *testing.T) {
	now := time.Date(2026, 5, 1, 12, 0, 0, 0, time.UTC)
	cases := []struct {
		after time.Duration
		want  int
	}{{36 * time.Hour, 1}, {24 * time.Hour, 1}, {23 * time.Hour, 0}, {0, 0}, {-time.Hour, -1}, {-24 * time.Hour, -1}, {-25 * time.Hour, -2}}
	for _, test := range cases {
		if got := DaysLeft(now.Add(test.after), now); got != test.want {
			t.Errorf("DaysLeft(%s) = %d, want %d", test.after, got, test.want)
		}
	}
}

func TestFindingsFlagLongValidityAndEveryCodeHasATitle(t *testing.T) {
	p := newPKI(t)
	issued, err := p.root.Issue(certgen.Request{CommonName: "long.fixture.test", Names: []string{"long.fixture.test"}, NotBefore: p.now.AddDate(0, 0, -1), NotAfter: p.now.AddDate(3, 0, 0)})
	if err != nil {
		t.Fatal(err)
	}
	result := Result{Target: Target{Host: "long.fixture.test", Port: 443}, Outcome: OutcomeOK, Presented: []*x509.Certificate{issued.Certificate}, ChainStatus: StatusValid, HostnameStatus: HostnameMatch}
	findings := Findings(result, p.now)
	if codes(findings) != FindingLongValidity || findings[0].Severity != theme.SeverityInfo {
		t.Fatalf("findings: %+v", findings)
	}
	for _, code := range FindingCodes() {
		if FindingTitle(code) == code {
			t.Errorf("finding %s has no title", code)
		}
	}
}

func TestParsePEM(t *testing.T) {
	p := newPKI(t)
	issued, err := p.intermediate.Issue(certgen.Request{CommonName: "pem.fixture.test", Names: []string{"pem.fixture.test", "10.0.0.7"}, NotBefore: p.now, NotAfter: p.now.AddDate(0, 1, 0)})
	if err != nil {
		t.Fatal(err)
	}
	bundle := certgen.PEM(issued.Certificate, p.intermediate.Certificate, issued.Certificate)
	certificates, err := ParsePEM(append([]byte("notes before the first block\n"), bundle...))
	if err != nil || len(certificates) != 2 {
		t.Fatalf("got %d certificates, error %v", len(certificates), err)
	}
	info := Describe(certificates[0])
	if info.CommonName != "pem.fixture.test" || strings.Join(info.Names, ",") != "10.0.0.7,pem.fixture.test" || info.KeyAlgorithm != "ECDSA" || info.KeyBits != 256 || info.IsCA || info.SelfSigned {
		t.Fatalf("unexpected description: %+v", info)
	}
	if len(info.Fingerprint) != 64 || ShortFingerprint(info.Fingerprint) != info.Fingerprint[:12] {
		t.Fatalf("fingerprint %q", info.Fingerprint)
	}
	if !Describe(p.root.Certificate).SelfSigned || !Describe(p.intermediate.Certificate).IsCA {
		t.Fatalf("authority flags are wrong")
	}
	failures := map[string]string{
		"":     "no PEM certificate",
		"text": "no PEM certificate",
		"-----BEGIN PRIVATE KEY-----\nAAAA\n-----END PRIVATE KEY-----\n": "never imported",
		"-----BEGIN CERTIFICATE-----\nAAAA\n-----END CERTIFICATE-----\n": "block 1 is not a valid certificate",
	}
	for input, want := range failures {
		if _, err := ParsePEM([]byte(input)); err == nil || !strings.Contains(err.Error(), want) {
			t.Errorf("ParsePEM(%q) error = %v, want %q", input, err, want)
		}
	}
	if _, err := ParsePEM(make([]byte, MaxPEMSize+1)); err == nil || !strings.Contains(err.Error(), "larger than") {
		t.Errorf("an oversized file was accepted: %v", err)
	}
}

func FuzzParsePEM(f *testing.F) {
	now := time.Now()
	issued, err := certgen.SelfSigned(certgen.Request{CommonName: "fuzz.fixture.test", NotBefore: now, NotAfter: now.AddDate(0, 0, 1)})
	if err != nil {
		f.Fatal(err)
	}
	f.Add(certgen.PEM(issued.Certificate))
	f.Add([]byte("-----BEGIN CERTIFICATE-----\nAAAA\n-----END CERTIFICATE-----\n"))
	f.Add([]byte(""))
	f.Fuzz(func(t *testing.T, data []byte) {
		certificates, err := ParsePEM(data)
		if err != nil {
			if len(certificates) != 0 {
				t.Fatalf("certificates returned together with an error")
			}
			return
		}
		if len(certificates) == 0 || len(certificates) > MaxPEMCertificates {
			t.Fatalf("%d certificates without an error", len(certificates))
		}
		for _, certificate := range certificates {
			info := Describe(certificate)
			if len(info.Fingerprint) != 64 || info.DisplayName() == "" {
				t.Fatalf("incomplete description: %+v", info)
			}
			for _, text := range append([]string{info.Subject, info.Issuer, info.CommonName}, info.Names...) {
				if unsafeText(text) {
					t.Fatalf("certificate text is not printable: %q", text)
				}
			}
		}
	})
}

func unsafeText(text string) bool {
	if !utf8.ValidString(text) {
		return true
	}
	for _, r := range text {
		if unicode.IsControl(r) || unicode.In(r, unicode.Cf, unicode.Zl, unicode.Zp, unicode.Co) {
			return true
		}
	}
	return false
}

func TestCheckNeutralizesHostileTextInCertificates(t *testing.T) {
	p := newPKI(t)
	hostile := "evil\x1b[2J\x1b]0;owned\x07\nsecond line\u202egnp.exe\u200b"
	target := serve(t, &tls.Config{Certificates: []tls.Certificate{p.issue(t, certgen.Request{CommonName: hostile, Names: []string{"other\x1b[31m.fixture.test"}})}})
	target.ServerName = "app.fixture.test"
	result := Check(context.Background(), target, Options{Roots: p.root.Pool(), Timeout: 5 * time.Second})
	if result.Outcome != OutcomeOK || len(result.Presented) == 0 || result.HostnameStatus != HostnameMismatch {
		t.Fatalf("unexpected result: %+v", result)
	}
	if !strings.Contains(result.Presented[0].Subject.CommonName, "\x1b") {
		t.Fatal("the fixture certificate does not carry the hostile text")
	}
	info := Describe(result.Presented[0])
	texts := append([]string{info.Subject, info.Issuer, info.CommonName, info.DisplayName(), result.Error, result.ChainError, result.HostnameError}, info.Names...)
	for _, finding := range Findings(result, p.now) {
		texts = append(texts, finding.Detail)
	}
	for _, text := range texts {
		if unsafeText(text) {
			t.Errorf("text from the server reaches the interfaces unfiltered: %q", text)
		}
	}
	if !strings.Contains(info.CommonName, "evil") || !strings.Contains(info.CommonName, "second line") {
		t.Fatalf("the readable part of the name was lost: %q", info.CommonName)
	}
}

func TestPrintableKeepsTextAndLimitsLength(t *testing.T) {
	for in, want := range map[string]string{
		"www.example.org":        "www.example.org",
		"Zürich Ærø 東京, O=Näme":  "Zürich Ærø 東京, O=Näme",
		"a\tb\r\nc":              "a\ufffdb\ufffd\ufffdc",
		"\x1b[0m":                "\ufffd[0m",
		"bad\xffbyte":            "bad\ufffdbyte",
		"right\u202eleft\u2028x": "right\ufffdleft\ufffdx",
		"":                       "",
	} {
		if got := Printable(in); got != want {
			t.Errorf("Printable(%q) = %q, want %q", in, got, want)
		}
	}
	long := Printable(strings.Repeat("é", MaxTextLength+50))
	if utf8.RuneCountInString(long) != MaxTextLength+1 || !strings.HasSuffix(long, "…") {
		t.Fatalf("long text is not cut at the limit: %d characters", utf8.RuneCountInString(long))
	}
	if got := shorten(strings.Repeat("é", maxErrorLength+50) + "\x1b"); !utf8.ValidString(got) || utf8.RuneCountInString(got) != maxErrorLength+1 {
		t.Fatalf("a long error is not cut on a character boundary: %d characters", utf8.RuneCountInString(got))
	}
}

func FuzzPrintable(f *testing.F) {
	for _, seed := range []string{"www.example.org", "a\x1b[2Jb", "\xff\xfe", "x\u202ey", strings.Repeat("a", MaxTextLength+1)} {
		f.Add(seed)
	}
	f.Fuzz(func(t *testing.T, text string) {
		got := Printable(text)
		if unsafeText(got) {
			t.Fatalf("Printable(%q) = %q is not printable", text, got)
		}
		if utf8.RuneCountInString(got) > MaxTextLength+1 {
			t.Fatalf("Printable(%q) has %d characters", text, utf8.RuneCountInString(got))
		}
		if again := Printable(got); utf8.RuneCountInString(got) <= MaxTextLength && again != got {
			t.Fatalf("Printable is not stable: %q then %q", got, again)
		}
	})
}
