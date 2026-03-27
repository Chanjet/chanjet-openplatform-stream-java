package security

import (
	"crypto/tls"
	"crypto/x509"
	"fmt"
	"net"
	"strings"
)

// Firewall implements the "3D Cross-Check Gate" for TLS connections to Chanjet domains.
type Firewall interface {
	VerifyConnection(cs tls.ConnectionState) error
	GetTLSConfig() *tls.Config
}

type chanjetFirewall struct {
	internalCAPool *x509.CertPool
}

func NewChanjetFirewall(caCerts []byte) (Firewall, error) {
	pool := x509.NewCertPool()
	if caCerts != nil {
		if ok := pool.AppendCertsFromPEM(caCerts); !ok {
			return nil, fmt.Errorf("failed to append internal CA certs")
		}
	} else {
		// Fallback to system pool if no internal pool provided
		var err error
		pool, err = x509.SystemCertPool()
		if err != nil {
			return nil, err
		}
	}

	return &chanjetFirewall{
		internalCAPool: pool,
	}, nil
}

func (f *chanjetFirewall) VerifyConnection(cs tls.ConnectionState) error {
	if len(cs.PeerCertificates) == 0 {
		return fmt.Errorf("no peer certificates provided")
	}

	cert := cs.PeerCertificates[0]
	
	// 1. Domain Check: Must be *.chanjet.com
	hasChanjetDomain := false
	for _, name := range cert.DNSNames {
		if strings.HasSuffix(name, ".chanjet.com") {
			hasChanjetDomain = true
			
			// 2. Wildcard Prohibition: No '*' allowed in leaf certificate for maximum security
			if strings.Contains(name, "*") {
				return fmt.Errorf("wildcard certificates are prohibited: %s", name)
			}
		}
	}

	if !hasChanjetDomain {
		return fmt.Errorf("certificate does not belong to chanjet.com domain")
	}

	return nil
}

func (f *chanjetFirewall) GetTLSConfig() *tls.Config {
	return &tls.Config{
		RootCAs:               f.internalCAPool,
		VerifyConnection:      f.VerifyConnection,
		InsecureSkipVerify:    false, // Must be false for 3D Cross-Check
		MinVersion:            tls.VersionTLS12,
	}
}

// DialContext provides a secure dialer that enforces the firewall.
func DialContext(network, addr string, fw Firewall) (net.Conn, error) {
	config := fw.GetTLSConfig()
	return tls.Dial(network, addr, config)
}
