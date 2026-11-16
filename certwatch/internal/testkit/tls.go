package testkit

import (
	"context"
	"crypto/tls"
	"crypto/x509"
	"net"
	"os"
	"path/filepath"
	"strconv"
	"sync"
	"testing"
	"time"

	"github.com/unalivin/certwatch/internal/certgen"
)

type TLSFixture struct {
	t     testing.TB
	Root  *certgen.Authority
	port  int
	lock  sync.Mutex
	chain tls.Certificate
	dials []string
}

func StartTLS(t testing.TB, now time.Time, name string, days int) *TLSFixture {
	t.Helper()
	root, err := certgen.NewAuthority("Fixture Root", now.AddDate(-1, 0, 0), now.AddDate(5, 0, 0))
	if err != nil {
		t.Fatal(err)
	}
	fixture := &TLSFixture{t: t, Root: root}
	fixture.Present(now, name, days)
	listener, err := tls.Listen("tcp", "127.0.0.1:0", &tls.Config{GetCertificate: func(*tls.ClientHelloInfo) (*tls.Certificate, error) {
		fixture.lock.Lock()
		defer fixture.lock.Unlock()
		chain := fixture.chain
		return &chain, nil
	}})
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = listener.Close() })
	fixture.port = listener.Addr().(*net.TCPAddr).Port
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
	return fixture
}

func (f *TLSFixture) Present(now time.Time, name string, days int) {
	f.t.Helper()
	issued, err := f.Root.Issue(certgen.Request{CommonName: name, Names: []string{name}, NotBefore: now.AddDate(0, 0, -5), NotAfter: now.AddDate(0, 0, days)})
	if err != nil {
		f.t.Fatal(err)
	}
	f.lock.Lock()
	f.chain = tls.Certificate{Certificate: [][]byte{issued.Certificate.Raw}, PrivateKey: issued.Key}
	f.lock.Unlock()
}

func (f *TLSFixture) Pool() *x509.CertPool {
	return f.Root.Pool()
}

func (f *TLSFixture) Dial(ctx context.Context, network, address string) (net.Conn, error) {
	f.lock.Lock()
	f.dials = append(f.dials, address)
	f.lock.Unlock()
	return (&net.Dialer{}).DialContext(ctx, network, "127.0.0.1:"+strconv.Itoa(f.port))
}

func (f *TLSFixture) Dialed() []string {
	f.lock.Lock()
	defer f.lock.Unlock()
	return append([]string(nil), f.dials...)
}

func PEMFile(t testing.TB, now time.Time, name string, days int) string {
	t.Helper()
	issued, err := certgen.SelfSigned(certgen.Request{CommonName: name, Names: []string{name}, NotBefore: now.AddDate(0, 0, -10), NotAfter: now.AddDate(0, 0, days)})
	if err != nil {
		t.Fatal(err)
	}
	path := filepath.Join(t.TempDir(), "certificates.pem")
	if err := os.WriteFile(path, certgen.PEM(issued.Certificate), 0o600); err != nil {
		t.Fatal(err)
	}
	return path
}
