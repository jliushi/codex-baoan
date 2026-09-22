package main

import (
	"encoding/json"
	"os"
	"path/filepath"
)

// guardConfig persists the real upstream (what CC Switch used before we attached),
// so the guard keeps chaining the real fetch through it (e.g. the user's Clash :7890).
type guardConfig struct {
	Upstream string `json:"upstream"`
}

func configPath() string {
	dir, _ := dataDir()
	return filepath.Join(dir, "config.json")
}

func loadConfig() guardConfig {
	var c guardConfig
	if b, err := os.ReadFile(configPath()); err == nil {
		json.Unmarshal(b, &c)
	}
	return c
}

func saveConfig(c guardConfig) {
	if b, err := json.MarshalIndent(c, "", "  "); err == nil {
		os.WriteFile(configPath(), b, 0o600)
	}
}

// resolveUpstream decides the guard's real upstream at startup:
//   - explicit --upstream flag wins (unless it's the "auto" sentinel)
//   - else the persisted config
//   - else CC Switch's current global proxy (if it isn't already us)
//   - else empty (direct)
func resolveUpstream(flagVal, guardURL string) string {
	if flagVal != "" && flagVal != "auto" {
		saveConfig(guardConfig{Upstream: flagVal})
		return flagVal
	}
	cfg := loadConfig()
	if cfg.Upstream != "" {
		return cfg.Upstream
	}
	if cc, err := ccGetGlobalProxy(); err == nil && cc != "" && cc != guardURL {
		saveConfig(guardConfig{Upstream: cc})
		return cc
	}
	return ""
}
