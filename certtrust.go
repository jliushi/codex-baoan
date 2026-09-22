package main

import (
	"crypto/sha1"
	"crypto/x509"
	"encoding/pem"
	"fmt"
	"log"
	"os"
	"os/exec"
	"strings"
)

const caCommonName = "Codex 保安 Guard Local Root CA"

// mustInstallCA generates the CA if needed and trusts it in the current user's Root
// store. No administrator elevation is required for the per-user store.
func mustInstallCA() {
	if _, err := loadOrCreateCA(); err != nil {
		log.Fatalf("生成本地 CA 失败：%v", err)
	}
	if caTrusted() {
		fmt.Println("本地根证书已在信任库中。")
		return
	}
	certPath, _, err := caFiles()
	if err != nil {
		log.Fatalf("%v", err)
	}
	cmd := exec.Command("certutil", "-user", "-addstore", "Root", certPath)
	out, err := cmd.CombinedOutput()
	if err != nil {
		log.Fatalf("安装证书失败：%v\n%s", err, string(out))
	}
	fmt.Println("已把本地根证书装入当前用户信任库。")
}

// installCAQuiet generates (if needed) and trusts the CA, returning an error instead
// of exiting. Used by the GUI's one-click button.
func installCAQuiet() error {
	if _, err := loadOrCreateCA(); err != nil {
		return err
	}
	if caTrusted() {
		return nil
	}
	certPath, _, err := caFiles()
	if err != nil {
		return err
	}
	out, err := exec.Command("certutil", "-user", "-addstore", "Root", certPath).CombinedOutput()
	if err != nil {
		return fmt.Errorf("%v\n%s", err, string(out))
	}
	return nil
}

func uninstallCA() error {
	thumb, err := caThumbprint()
	if err != nil {
		return err
	}
	cmd := exec.Command("certutil", "-user", "-delstore", "Root", thumb)
	if out, err := cmd.CombinedOutput(); err != nil {
		return fmt.Errorf("%v\n%s", err, string(out))
	}
	return nil
}

// caTrusted reports whether our CA (by thumbprint) is present in the user Root store.
func caTrusted() bool {
	thumb, err := caThumbprint()
	if err != nil {
		return false
	}
	out, err := exec.Command("certutil", "-user", "-store", "Root", thumb).CombinedOutput()
	if err != nil {
		return false
	}
	return strings.Contains(string(out), caCommonName) || strings.Contains(strings.ToLower(string(out)), strings.ToLower(thumb))
}

// caThumbprint returns the SHA-1 fingerprint (hex) of the persisted CA certificate.
func caThumbprint() (string, error) {
	certPath, _, err := caFiles()
	if err != nil {
		return "", err
	}
	raw, err := os.ReadFile(certPath)
	if err != nil {
		return "", err
	}
	block, _ := pem.Decode(raw)
	if block == nil {
		return "", fmt.Errorf("CA 证书不是有效 PEM")
	}
	cert, err := x509.ParseCertificate(block.Bytes)
	if err != nil {
		return "", err
	}
	sum := sha1.Sum(cert.Raw)
	return fmt.Sprintf("%x", sum[:]), nil
}
