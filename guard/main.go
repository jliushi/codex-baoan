// codex-baoan-guard: a local MITM observer that sits behind CC Switch
//
//	Codex -> CC Switch (:15721) -> [this guard] -> upstream proxy (:7890) -> relay
//
// It terminates TLS with a locally-trusted root CA so it can read the exact request
// body CC Switch sends upstream, paired with the upstream's self-reported model and
// token usage. That clean (body <-> input_tokens) pair is what tokenizer fingerprinting
// needs and what neither the CC Switch DB nor Codex rollout logs can provide.
//
// Single static .exe, no Python, no runtime deps. Subcommands:
//
//	guard install     generate+trust the CA, then run
//	guard run         run the proxy + dashboard (assumes CA already trusted)
//	guard uninstall   remove the trusted CA
//	guard status      print CA/trust/port info
package main

import (
	"crypto/tls"
	"flag"
	"fmt"
	"log"
	"net/http"
	"net/url"
	"os"
	"strings"

	"github.com/elazarl/goproxy"
)

const (
	defaultProxyPort = 8899   // CC Switch's outbound proxy should point here
	defaultDashPort  = 8766   // local dashboard
	defaultUpstream  = "auto" // auto = inherit CC Switch's existing outbound proxy
)

func main() {
	log.SetFlags(log.LstdFlags)
	cmd := "gui"
	if len(os.Args) > 1 && !strings.HasPrefix(os.Args[1], "-") {
		cmd = os.Args[1]
	}
	fs := flag.NewFlagSet(cmd, flag.ExitOnError)
	proxyPort := fs.Int("proxy-port", defaultProxyPort, "MITM proxy listen port")
	dashPort := fs.Int("dash-port", defaultDashPort, "dashboard listen port")
	upstream := fs.String("upstream", defaultUpstream, "upstream proxy URL for the real fetch, empty for direct")
	_ = fs.Parse(argsAfter(cmd))

	switch cmd {
	case "gui", "":
		startEngine(*proxyPort, *dashPort, *upstream)
		launchGUI(*dashPort)
	case "install":
		mustInstallCA()
		startEngine(*proxyPort, *dashPort, *upstream)
		launchGUI(*dashPort)
	case "run":
		startEngine(*proxyPort, *dashPort, *upstream)
		select {} // headless: keep serving
	case "uninstall":
		if err := uninstallCA(); err != nil {
			log.Fatalf("卸载证书失败：%v", err)
		}
		fmt.Println("已移除本地根证书。")
	case "status":
		printStatus(*proxyPort, *dashPort)
	default:
		fmt.Println("用法: guard [gui|install|run|uninstall|status]")
		os.Exit(2)
	}
}

func argsAfter(cmd string) []string {
	// os.Args[0]=exe; if os.Args[1] is a known subcommand, flags start at [2], else [1].
	if len(os.Args) > 1 && os.Args[1] == cmd && !strings.HasPrefix(cmd, "-") {
		return os.Args[2:]
	}
	return os.Args[1:]
}

// startEngine boots the MITM proxy and the dashboard in background goroutines and
// returns immediately so the GUI (or a headless wait) can run on the main goroutine.
func startEngine(proxyPort, dashPort int, upstream string) {
	ca, err := loadOrCreateCA()
	if err != nil {
		log.Fatalf("准备本地 CA 失败：%v", err)
	}
	configureGoproxyCA(ca)

	store := newStore()
	if err := store.load(); err != nil {
		log.Printf("载入历史样本：%v", err)
	}

	proxy := goproxy.NewProxyHttpServer()
	proxy.Verbose = false

	guardURL := fmt.Sprintf("http://127.0.0.1:%d", proxyPort)
	realUpstream := resolveUpstream(upstream, guardURL)
	// Chain the real fetch through the user's existing proxy (e.g. Clash :7890) so
	// their routing/VPN is preserved. Empty = direct.
	if realUpstream != "" {
		if u, err := url.Parse(realUpstream); err == nil {
			proxy.Tr.Proxy = http.ProxyURL(u)
			proxy.ConnectDial = proxy.NewConnectDialToProxy(realUpstream)
			log.Printf("上游经代理：%s", realUpstream)
		} else {
			log.Printf("上游代理地址无效，改为直连：%v", err)
		}
	}

	proxy.OnRequest().HandleConnect(goproxy.AlwaysMitm)
	installCapture(proxy, store)

	dash := newDashboard(store, proxyPort, realUpstream)
	go func() {
		addr := fmt.Sprintf("127.0.0.1:%d", dashPort)
		log.Printf("看板 http://%s", addr)
		if err := http.ListenAndServe(addr, dash); err != nil {
			log.Printf("看板启动失败：%v", err)
		}
	}()
	go func() {
		addr := fmt.Sprintf("127.0.0.1:%d", proxyPort)
		log.Printf("MITM 代理监听 %s", addr)
		if err := http.ListenAndServe(addr, proxy); err != nil {
			log.Printf("代理启动失败：%v", err)
		}
	}()
}

// configureGoproxyCA points goproxy's MITM machinery at our own root CA.
func configureGoproxyCA(ca tls.Certificate) {
	goproxy.GoproxyCa = ca
	tlsConfig := goproxy.TLSConfigFromCA(&ca)
	goproxy.OkConnect = &goproxy.ConnectAction{Action: goproxy.ConnectAccept, TLSConfig: tlsConfig}
	goproxy.MitmConnect = &goproxy.ConnectAction{Action: goproxy.ConnectMitm, TLSConfig: tlsConfig}
	goproxy.HTTPMitmConnect = &goproxy.ConnectAction{Action: goproxy.ConnectHTTPMitm, TLSConfig: tlsConfig}
	goproxy.RejectConnect = &goproxy.ConnectAction{Action: goproxy.ConnectReject, TLSConfig: tlsConfig}
}

func printStatus(proxyPort, dashPort int) {
	certPath, _, _ := caFiles()
	fmt.Printf("CA 文件: %s (存在=%v)\n", certPath, fileExists(certPath))
	fmt.Printf("已信任: %v\n", caTrusted())
	fmt.Printf("代理端口: %d  看板端口: %d\n", proxyPort, dashPort)
}
