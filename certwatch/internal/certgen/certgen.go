package certgen

import (
	"crypto"
	"crypto/ecdsa"
	"crypto/elliptic"
	"crypto/rand"
	"crypto/rsa"
	"crypto/x509"
	"crypto/x509/pkix"
	"encoding/pem"
	"errors"
	"math/big"
	"net"
	"time"
)

type Authority struct {
	Certificate *x509.Certificate
	Key         crypto.Signer
}

type Request struct {
	CommonName   string
	Names        []string
	NotBefore    time.Time
	NotAfter     time.Time
	RSABits      int
	Algorithm    x509.SignatureAlgorithm
	Intermediate bool
}

type Issued struct {
	Certificate *x509.Certificate
	Key         crypto.Signer
}

func newKey(bits int) (crypto.Signer, error) {
	if bits > 0 {
		return rsa.GenerateKey(rand.Reader, bits)
	}
	return ecdsa.GenerateKey(elliptic.P256(), rand.Reader)
}

func serial() (*big.Int, error) {
	limit := new(big.Int).Lsh(big.NewInt(1), 120)
	return rand.Int(rand.Reader, limit)
}

func split(names []string) ([]string, []net.IP) {
	var hosts []string
	var addresses []net.IP
	for _, name := range names {
		if ip := net.ParseIP(name); ip != nil {
			addresses = append(addresses, ip)
			continue
		}
		hosts = append(hosts, name)
	}
	return hosts, addresses
}

func build(template, parent *x509.Certificate, key, signer crypto.Signer) (*x509.Certificate, error) {
	der, err := x509.CreateCertificate(rand.Reader, template, parent, key.Public(), signer)
	if err != nil {
		return nil, err
	}
	return x509.ParseCertificate(der)
}

func NewAuthority(commonName string, notBefore, notAfter time.Time) (*Authority, error) {
	key, err := newKey(0)
	if err != nil {
		return nil, err
	}
	number, err := serial()
	if err != nil {
		return nil, err
	}
	template := &x509.Certificate{
		SerialNumber:          number,
		Subject:               pkix.Name{CommonName: commonName, Organization: []string{"CertWatch test and demo data"}},
		NotBefore:             notBefore,
		NotAfter:              notAfter,
		KeyUsage:              x509.KeyUsageCertSign | x509.KeyUsageCRLSign,
		BasicConstraintsValid: true,
		IsCA:                  true,
	}
	certificate, err := build(template, template, key, key)
	if err != nil {
		return nil, err
	}
	return &Authority{Certificate: certificate, Key: key}, nil
}

func (a *Authority) template(request Request) (*x509.Certificate, crypto.Signer, error) {
	if request.CommonName == "" && len(request.Names) == 0 {
		return nil, nil, errors.New("a certificate request needs a common name or at least one name")
	}
	key, err := newKey(request.RSABits)
	if err != nil {
		return nil, nil, err
	}
	number, err := serial()
	if err != nil {
		return nil, nil, err
	}
	hosts, addresses := split(request.Names)
	template := &x509.Certificate{
		SerialNumber:          number,
		Subject:               pkix.Name{CommonName: request.CommonName},
		NotBefore:             request.NotBefore,
		NotAfter:              request.NotAfter,
		DNSNames:              hosts,
		IPAddresses:           addresses,
		KeyUsage:              x509.KeyUsageDigitalSignature,
		ExtKeyUsage:           []x509.ExtKeyUsage{x509.ExtKeyUsageServerAuth},
		BasicConstraintsValid: true,
		SignatureAlgorithm:    request.Algorithm,
	}
	if request.Intermediate {
		template.IsCA = true
		template.KeyUsage = x509.KeyUsageCertSign | x509.KeyUsageCRLSign
		template.ExtKeyUsage = nil
	}
	return template, key, nil
}

func (a *Authority) Issue(request Request) (*Issued, error) {
	template, key, err := a.template(request)
	if err != nil {
		return nil, err
	}
	certificate, err := build(template, a.Certificate, key, a.Key)
	if err != nil {
		return nil, err
	}
	return &Issued{Certificate: certificate, Key: key}, nil
}

func (a *Authority) Intermediate(commonName string, notBefore, notAfter time.Time) (*Authority, error) {
	issued, err := a.Issue(Request{CommonName: commonName, NotBefore: notBefore, NotAfter: notAfter, Intermediate: true})
	if err != nil {
		return nil, err
	}
	return &Authority{Certificate: issued.Certificate, Key: issued.Key}, nil
}

func SelfSigned(request Request) (*Issued, error) {
	template, key, err := (&Authority{}).template(request)
	if err != nil {
		return nil, err
	}
	certificate, err := build(template, template, key, key)
	if err != nil {
		return nil, err
	}
	return &Issued{Certificate: certificate, Key: key}, nil
}

func (a *Authority) Pool() *x509.CertPool {
	pool := x509.NewCertPool()
	pool.AddCert(a.Certificate)
	return pool
}

func PEM(certificates ...*x509.Certificate) []byte {
	var out []byte
	for _, certificate := range certificates {
		out = append(out, pem.EncodeToMemory(&pem.Block{Type: "CERTIFICATE", Bytes: certificate.Raw})...)
	}
	return out
}
