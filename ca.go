package main

import (
	"crypto/rand"
	"crypto/rsa"
	"crypto/tls"
	"crypto/x509"
	"crypto/x509/pkix"
	"encoding/pem"
	"fmt"
	"math/big"
	"os"
	"path/filepath"
	"time"
)

// caFiles returns the on-disk paths for the persisted root CA (PEM cert + key).
func caFiles() (certPath, keyPath string, err error) {
	dir, err := dataDir()
	if err != nil {
		return "", "", err
	}
	return filepath.Join(dir, "codex-guard-ca.crt"), filepath.Join(dir, "codex-guard-ca.key"), nil
}

// dataDir is a stable per-user directory for the CA and captured samples.
func dataDir() (string, error) {
	base, err := os.UserConfigDir()
	if err != nil {
		return "", err
	}
	dir := filepath.Join(base, "codex-baoan-guard")
	if err := os.MkdirAll(dir, 0o700); err != nil {
		return "", err
	}
	return dir, nil
}

// loadOrCreateCA loads the persisted root CA, generating and saving a fresh one
// on first run. The CA only ever signs leaf certs for hosts this proxy MITMs.
func loadOrCreateCA() (tls.Certificate, error) {
	certPath, keyPath, err := caFiles()
	if err != nil {
		return tls.Certificate{}, err
	}
	if fileExists(certPath) && fileExists(keyPath) {
		ca, err := tls.LoadX509KeyPair(certPath, keyPath)
		if err != nil {
			return tls.Certificate{}, fmt.Errorf("加载本地 CA 失败：%w", err)
		}
		if ca.Leaf == nil {
			ca.Leaf, _ = x509.ParseCertificate(ca.Certificate[0])
		}
		return ca, nil
	}
	return createCA(certPath, keyPath)
}

func createCA(certPath, keyPath string) (tls.Certificate, error) {
	key, err := rsa.GenerateKey(rand.Reader, 3072)
	if err != nil {
		return tls.Certificate{}, err
	}
	serial, err := rand.Int(rand.Reader, new(big.Int).Lsh(big.NewInt(1), 128))
	if err != nil {
		return tls.Certificate{}, err
	}
	tmpl := &x509.Certificate{
		SerialNumber: serial,
		Subject: pkix.Name{
			CommonName:   "Codex 保安 Guard Local Root CA",
			Organization: []string{"codex-baoan-guard (local only)"},
		},
		NotBefore:             time.Now().Add(-time.Hour),
		NotAfter:              time.Now().AddDate(5, 0, 0),
		IsCA:                  true,
		KeyUsage:              x509.KeyUsageCertSign | x509.KeyUsageCRLSign | x509.KeyUsageDigitalSignature,
		BasicConstraintsValid: true,
		MaxPathLenZero:        true,
	}
	der, err := x509.CreateCertificate(rand.Reader, tmpl, tmpl, &key.PublicKey, key)
	if err != nil {
		return tls.Certificate{}, err
	}
	certPem := pem.EncodeToMemory(&pem.Block{Type: "CERTIFICATE", Bytes: der})
	keyPem := pem.EncodeToMemory(&pem.Block{Type: "RSA PRIVATE KEY", Bytes: x509.MarshalPKCS1PrivateKey(key)})
	if err := os.WriteFile(certPath, certPem, 0o600); err != nil {
		return tls.Certificate{}, err
	}
	if err := os.WriteFile(keyPath, keyPem, 0o600); err != nil {
		return tls.Certificate{}, err
	}
	ca, err := tls.X509KeyPair(certPem, keyPem)
	if err != nil {
		return tls.Certificate{}, err
	}
	ca.Leaf, _ = x509.ParseCertificate(ca.Certificate[0])
	return ca, nil
}

func fileExists(path string) bool {
	info, err := os.Stat(path)
	return err == nil && !info.IsDir()
}
