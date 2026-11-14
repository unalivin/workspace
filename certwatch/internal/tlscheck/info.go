package tlscheck

import (
	"bytes"
	"crypto/ecdsa"
	"crypto/ed25519"
	"crypto/rsa"
	"crypto/sha256"
	"crypto/x509"
	"encoding/hex"
	"encoding/pem"
	"errors"
	"fmt"
	"sort"
	"strings"
	"time"
	"unicode"
	"unicode/utf8"
)

const (
	MaxPEMSize         = 4 << 20
	MaxPEMCertificates = 500
	MaxTextLength      = 512
)

type Info struct {
	Fingerprint        string
	Subject            string
	Issuer             string
	CommonName         string
	Serial             string
	NotBefore          time.Time
	NotAfter           time.Time
	Names              []string
	KeyAlgorithm       string
	KeyBits            int
	SignatureAlgorithm string
	IsCA               bool
	SelfSigned         bool
	PEM                string
}

func Clean(text string) string {
	return strings.Map(func(r rune) rune {
		if r == utf8.RuneError || unicode.IsControl(r) || unicode.In(r, unicode.Cf, unicode.Zl, unicode.Zp, unicode.Co, unicode.Cs) {
			return unicode.ReplacementChar
		}
		return r
	}, text)
}

func Printable(text string) string {
	text = Clean(text)
	if utf8.RuneCountInString(text) > MaxTextLength {
		return string([]rune(text)[:MaxTextLength]) + "…"
	}
	return text
}

func Fingerprint(certificate *x509.Certificate) string {
	sum := sha256.Sum256(certificate.Raw)
	return hex.EncodeToString(sum[:])
}

func keyDetails(certificate *x509.Certificate) (string, int) {
	switch key := certificate.PublicKey.(type) {
	case *rsa.PublicKey:
		return "RSA", key.N.BitLen()
	case *ecdsa.PublicKey:
		return "ECDSA", key.Curve.Params().BitSize
	case ed25519.PublicKey:
		return "Ed25519", 256
	}
	return certificate.PublicKeyAlgorithm.String(), 0
}

func selfSigned(certificate *x509.Certificate) bool {
	if !bytes.Equal(certificate.RawSubject, certificate.RawIssuer) {
		return false
	}
	return certificate.CheckSignature(certificate.SignatureAlgorithm, certificate.RawTBSCertificate, certificate.Signature) == nil
}

func Names(certificate *x509.Certificate) []string {
	names := make([]string, 0, len(certificate.DNSNames)+len(certificate.IPAddresses))
	for _, name := range certificate.DNSNames {
		names = append(names, Printable(name))
	}
	for _, address := range certificate.IPAddresses {
		names = append(names, address.String())
	}
	sort.Strings(names)
	return names
}

func Describe(certificate *x509.Certificate) Info {
	algorithm, bits := keyDetails(certificate)
	return Info{
		Fingerprint:        Fingerprint(certificate),
		Subject:            Printable(certificate.Subject.String()),
		Issuer:             Printable(certificate.Issuer.String()),
		CommonName:         Printable(certificate.Subject.CommonName),
		Serial:             certificate.SerialNumber.Text(16),
		NotBefore:          certificate.NotBefore.UTC(),
		NotAfter:           certificate.NotAfter.UTC(),
		Names:              Names(certificate),
		KeyAlgorithm:       algorithm,
		KeyBits:            bits,
		SignatureAlgorithm: certificate.SignatureAlgorithm.String(),
		IsCA:               certificate.IsCA,
		SelfSigned:         selfSigned(certificate),
		PEM:                string(pem.EncodeToMemory(&pem.Block{Type: "CERTIFICATE", Bytes: certificate.Raw})),
	}
}

func (i Info) DisplayName() string {
	switch {
	case i.CommonName != "":
		return i.CommonName
	case len(i.Names) > 0:
		return i.Names[0]
	case i.Subject != "":
		return i.Subject
	}
	return "certificate " + ShortFingerprint(i.Fingerprint)
}

func ShortFingerprint(fingerprint string) string {
	if len(fingerprint) > 12 {
		return fingerprint[:12]
	}
	return fingerprint
}

func DaysLeft(notAfter, now time.Time) int {
	left := notAfter.Sub(now)
	days := int(left / (24 * time.Hour))
	if left < 0 && left%(24*time.Hour) != 0 {
		days--
	}
	return days
}

func ParsePEM(data []byte) ([]*x509.Certificate, error) {
	if len(data) > MaxPEMSize {
		return nil, fmt.Errorf("the file is larger than %d MB; split it into smaller files", MaxPEMSize>>20)
	}
	var certificates []*x509.Certificate
	seen := map[string]bool{}
	rest := data
	blocks := 0
	for {
		var block *pem.Block
		block, rest = pem.Decode(rest)
		if block == nil {
			break
		}
		blocks++
		if block.Type != "CERTIFICATE" {
			continue
		}
		certificate, err := x509.ParseCertificate(block.Bytes)
		if err != nil {
			return nil, fmt.Errorf("PEM block %d is not a valid certificate: %s", blocks, strings.TrimPrefix(err.Error(), "x509: "))
		}
		fingerprint := Fingerprint(certificate)
		if seen[fingerprint] {
			continue
		}
		seen[fingerprint] = true
		certificates = append(certificates, certificate)
		if len(certificates) > MaxPEMCertificates {
			return nil, fmt.Errorf("the file holds more than %d certificates; split it into smaller files", MaxPEMCertificates)
		}
	}
	if len(certificates) == 0 {
		if blocks > 0 {
			return nil, errors.New("the file holds PEM data but no certificate; private keys and requests are never imported")
		}
		return nil, errors.New("the file holds no PEM certificate; expected a block that starts with -----BEGIN CERTIFICATE-----")
	}
	return certificates, nil
}
