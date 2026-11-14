package certgen

import (
	"crypto/x509"
	"encoding/pem"
	"testing"
	"time"
)

func TestIssuedChainsVerifyAgainstTheirRootOnly(t *testing.T) {
	now := time.Date(2026, 3, 2, 9, 30, 0, 0, time.UTC)
	root, err := NewAuthority("Test Root", now.AddDate(-1, 0, 0), now.AddDate(5, 0, 0))
	if err != nil {
		t.Fatal(err)
	}
	issuing, err := root.Intermediate("Test Issuing CA", now.AddDate(0, -6, 0), now.AddDate(2, 0, 0))
	if err != nil {
		t.Fatal(err)
	}
	leaf, err := issuing.Issue(Request{CommonName: "www.fixture.test", Names: []string{"www.fixture.test", "192.0.2.10"}, NotBefore: now.AddDate(0, -1, 0), NotAfter: now.AddDate(0, 2, 0)})
	if err != nil {
		t.Fatal(err)
	}
	if len(leaf.Certificate.DNSNames) != 1 || len(leaf.Certificate.IPAddresses) != 1 || leaf.Certificate.IsCA {
		t.Fatalf("names were not split into DNS names and addresses: %v %v", leaf.Certificate.DNSNames, leaf.Certificate.IPAddresses)
	}
	intermediates := x509.NewCertPool()
	intermediates.AddCert(issuing.Certificate)
	options := x509.VerifyOptions{Roots: root.Pool(), Intermediates: intermediates, CurrentTime: now, DNSName: "www.fixture.test"}
	chains, err := leaf.Certificate.Verify(options)
	if err != nil || len(chains) != 1 || len(chains[0]) != 3 {
		t.Fatalf("the issued chain does not verify: %v %v", chains, err)
	}
	options.DNSName = "other.fixture.test"
	if _, err := leaf.Certificate.Verify(options); err == nil {
		t.Fatal("a certificate verified for a name it does not carry")
	}
	options.DNSName = "www.fixture.test"
	options.CurrentTime = now.AddDate(0, 3, 0)
	if _, err := leaf.Certificate.Verify(options); err == nil {
		t.Fatal("an expired certificate verified")
	}
	stranger, err := NewAuthority("Other Root", now.AddDate(-1, 0, 0), now.AddDate(5, 0, 0))
	if err != nil {
		t.Fatal(err)
	}
	options.CurrentTime = now
	options.Roots = stranger.Pool()
	if _, err := leaf.Certificate.Verify(options); err == nil {
		t.Fatal("a certificate verified against a root that did not issue it")
	}
	if root.Certificate.SerialNumber.Cmp(issuing.Certificate.SerialNumber) == 0 || leaf.Certificate.SerialNumber.Sign() <= 0 {
		t.Fatal("serial numbers are not random positive numbers")
	}
}

func TestSelfSignedAndPEM(t *testing.T) {
	now := time.Date(2026, 3, 2, 9, 30, 0, 0, time.UTC)
	own, err := SelfSigned(Request{CommonName: "printer.fixture.test", Names: []string{"printer.fixture.test"}, NotBefore: now, NotAfter: now.AddDate(1, 0, 0)})
	if err != nil {
		t.Fatal(err)
	}
	if own.Certificate.Issuer.CommonName != own.Certificate.Subject.CommonName || own.Certificate.CheckSignature(own.Certificate.SignatureAlgorithm, own.Certificate.RawTBSCertificate, own.Certificate.Signature) != nil {
		t.Fatal("the self-signed certificate is not signed by itself")
	}
	if _, err := own.Certificate.Verify(x509.VerifyOptions{Roots: x509.NewCertPool(), CurrentTime: now}); err == nil {
		t.Fatal("a self-signed certificate verified against an empty trust store")
	}
	rest := PEM(own.Certificate, own.Certificate)
	count := 0
	for {
		var block *pem.Block
		block, rest = pem.Decode(rest)
		if block == nil {
			break
		}
		if block.Type != "CERTIFICATE" {
			t.Fatalf("PEM block type %q", block.Type)
		}
		if _, err := x509.ParseCertificate(block.Bytes); err != nil {
			t.Fatal(err)
		}
		count++
	}
	if count != 2 {
		t.Fatalf("PEM holds %d certificates, want 2", count)
	}
}
