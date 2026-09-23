package main

import (
	"database/sql"
	"fmt"
	"net"
	"net/url"
	"os"
	"path/filepath"
	"strconv"
	"time"

	_ "modernc.org/sqlite"
)

// ccSwitchDBPath locates cc-switch.db in the usual per-user locations.
func ccSwitchDBPath() string {
	var candidates []string
	if d, err := os.UserConfigDir(); err == nil { // %APPDATA%
		candidates = append(candidates, filepath.Join(d, "cc-switch", "cc-switch.db"))
	}
	if h, err := os.UserHomeDir(); err == nil {
		candidates = append(candidates,
			filepath.Join(h, "AppData", "Local", "cc-switch", "cc-switch.db"),
			filepath.Join(h, ".cc-switch", "cc-switch.db"))
	}
	for _, p := range candidates {
		if fileExists(p) {
			return p
		}
	}
	return ""
}

func openCCSwitch(mode string) (*sql.DB, error) {
	path := ccSwitchDBPath()
	if path == "" {
		return nil, fmt.Errorf("未找到 cc-switch.db")
	}
	dsn := fmt.Sprintf("file:%s?_pragma=busy_timeout(4000)&mode=%s", filepath.ToSlash(path), mode)
	return sql.Open("sqlite", dsn)
}

// ccGetGlobalProxy reads CC Switch's configured outbound proxy (settings.global_proxy_url).
func ccGetGlobalProxy() (string, error) {
	db, err := openCCSwitch("ro")
	if err != nil {
		return "", err
	}
	defer db.Close()
	var value string
	err = db.QueryRow("SELECT value FROM settings WHERE key='global_proxy_url'").Scan(&value)
	if err == sql.ErrNoRows {
		return "", nil
	}
	return value, err
}

// ccSetGlobalProxy writes CC Switch's outbound proxy. Empty clears it (direct).
// CC Switch reads this at startup, so a one-time restart applies the change.
func ccSetGlobalProxy(url string) error {
	db, err := openCCSwitch("rw")
	if err != nil {
		return err
	}
	defer db.Close()
	if url == "" {
		_, err = db.Exec("DELETE FROM settings WHERE key='global_proxy_url'")
		return err
	}
	_, err = db.Exec(
		"INSERT INTO settings(key,value) VALUES('global_proxy_url',?1) ON CONFLICT(key) DO UPDATE SET value=?1",
		url)
	return err
}

// ccCurrentProviderName returns the current Codex provider's display name, best effort.
func ccCurrentProviderName() string {
	db, err := openCCSwitch("ro")
	if err != nil {
		return ""
	}
	defer db.Close()
	var name string
	db.QueryRow("SELECT name FROM providers WHERE app_type='codex' AND is_current=1").Scan(&name)
	return name
}

// attachState summarizes whether CC Switch currently routes through this guard.
type attachState struct {
	Found       bool   `json:"ccswitch_found"`
	CCProxy     string `json:"ccswitch_proxy"`
	Attached    bool   `json:"attached"`
	Provider    string `json:"provider"`
	LastSeenAgo int64  `json:"last_seen_sec"` // seconds since last observed sample, -1 if never
}

func computeAttachState(guardURL string, lastSample time.Time) attachState {
	st := attachState{LastSeenAgo: -1}
	if ccSwitchDBPath() == "" {
		return st
	}
	st.Found = true
	st.Provider = ccCurrentProviderName()
	if p, err := ccGetGlobalProxy(); err == nil {
		st.CCProxy = p
		st.Attached = p == guardURL
	}
	if !lastSample.IsZero() {
		st.LastSeenAgo = int64(time.Since(lastSample).Seconds())
	}
	return st
}

// guardListening reports whether the local proxy from a previous session is
// still alive. A missing listener means the DB entry is stale and can be safely
// recovered on the next launch.
func guardListening(guardURL string) bool {
	u, err := url.Parse(guardURL)
	if err != nil || u.Hostname() == "" {
		return false
	}
	port, err := strconv.Atoi(u.Port())
	if err != nil {
		return false
	}
	conn, err := net.DialTimeout("tcp", net.JoinHostPort(u.Hostname(), strconv.Itoa(port)), 150*time.Millisecond)
	if err != nil {
		return false
	}
	conn.Close()
	return true
}

// recoverStaleAttachment repairs a CC Switch setting left behind by a crashed
// guard process. It only runs when the previous guard listener is gone.
func recoverStaleAttachment(guardURL string) error {
	c := loadConfig()
	if !c.Attached || c.OwnerPID == 0 {
		return nil
	}
	current, err := ccGetGlobalProxy()
	if err != nil || current != guardURL || guardListening(guardURL) {
		return err
	}
	if err := ccSetGlobalProxy(c.OriginalProxy); err != nil {
		return err
	}
	clearAttachment()
	return nil
}

// detachOwnedAttachment restores the setting only when this process still owns
// the attachment and CC Switch has not been changed by the user meanwhile.
func detachOwnedAttachment(guardURL string) error {
	c := loadConfig()
	if !c.Attached || c.OwnerPID != os.Getpid() {
		return nil
	}
	current, err := ccGetGlobalProxy()
	if err != nil {
		return err
	}
	if current != guardURL {
		clearAttachment()
		return nil
	}
	if err := ccSetGlobalProxy(c.OriginalProxy); err != nil {
		return err
	}
	clearAttachment()
	return nil
}
