package main

import (
	"encoding/json"
	"fmt"
	"math"
	"net/http"
	"sort"
	"strings"
	"time"
)

var shanghai = time.FixedZone("CST", 8*3600)

const (
	minFingerprintSamples = 8
	slopeLo               = 0.85
	slopeHi               = 1.20
	separation            = 1.3
)

type GroupVerdict struct {
	RequestedModel string   `json:"requested_model"`
	ReportedModels []string `json:"reported_models"`
	Samples        int      `json:"samples"`
	Family         string   `json:"family"`         // o200k / cl100k / suspected_non_gpt / insufficient / ambiguous
	FamilyLabel    string   `json:"family_label"`
	Slope          float64  `json:"slope"`
	RMSE           float64  `json:"rmse"`
	Note           string   `json:"note"`
}

type Report struct {
	Date               string         `json:"date"`
	GeneratedAt        string         `json:"generated_at"`
	TotalRequests      int            `json:"total_requests"`
	RoutingSubstitutions int          `json:"routing_substitutions"`
	TokenAnomalies     int            `json:"token_anomalies"`
	Groups             []GroupVerdict `json:"groups"`
	ActualModels       []ActualModel  `json:"actual_models"`
	Limitations        []string       `json:"limitations"`
}

type ActualModel struct {
	RequestedModel string `json:"requested_model"`
	ActualModel    string `json:"actual_model"`
	Evidence       string `json:"evidence"` // self_reported / tokenizer_fingerprint / undetermined
	Count          int    `json:"count"`
}

type dashboard struct {
	store     *Store
	proxyPort int
	upstream  string
}

func newDashboard(store *Store, proxyPort int, upstream string) http.Handler {
	d := &dashboard{store: store, proxyPort: proxyPort, upstream: upstream}
	mux := http.NewServeMux()
	mux.HandleFunc("/api/report", d.reportHandler)
	mux.HandleFunc("/api/status", d.statusHandler)
	mux.HandleFunc("/api/cert/install", d.certInstallHandler)
	mux.HandleFunc("/api/cert/uninstall", d.certUninstallHandler)
	mux.HandleFunc("/api/attach", d.attachHandler)
	mux.HandleFunc("/api/detach", d.detachHandler)
	mux.HandleFunc("/", d.indexHandler)
	return mux
}

func (d *dashboard) guardURL() string { return fmt.Sprintf("http://127.0.0.1:%d", d.proxyPort) }

func (d *dashboard) statusHandler(w http.ResponseWriter, r *http.Request) {
	certPath, _, _ := caFiles()
	writeJSON(w, map[string]any{
		"cert_trusted":      caTrusted(),
		"cert_exists":       fileExists(certPath),
		"proxy_port":        d.proxyPort,
		"upstream":          d.upstream,
		"proxy_url":         d.guardURL(),
		"fingerprint_ready": encodersReady(),
		"routing":           computeAttachState(d.guardURL(), d.store.LastSeen()),
	})
}

// attachHandler points CC Switch's outbound proxy at this guard (writes its settings;
// CC Switch applies it on next restart). Preserves the previous proxy as guard upstream.
func (d *dashboard) attachHandler(w http.ResponseWriter, r *http.Request) {
	prev, _ := ccGetGlobalProxy()
	if prev != "" && prev != d.guardURL() {
		saveConfig(guardConfig{Upstream: prev})
	}
	if err := ccSetGlobalProxy(d.guardURL()); err != nil {
		writeJSON(w, map[string]any{"ok": false, "error": err.Error()})
		return
	}
	writeJSON(w, map[string]any{"ok": true, "restart_required": true,
		"message": "已把 CC Switch 出口代理指向本工具。请重启一次 CC Switch 生效。"})
}

func (d *dashboard) detachHandler(w http.ResponseWriter, r *http.Request) {
	restore := loadConfig().Upstream
	if err := ccSetGlobalProxy(restore); err != nil {
		writeJSON(w, map[string]any{"ok": false, "error": err.Error()})
		return
	}
	writeJSON(w, map[string]any{"ok": true, "restart_required": true,
		"message": "已恢复 CC Switch 原出口设置。请重启一次 CC Switch 生效。"})
}

func (d *dashboard) certInstallHandler(w http.ResponseWriter, r *http.Request) {
	if err := installCAQuiet(); err != nil {
		writeJSON(w, map[string]any{"ok": false, "error": err.Error()})
		return
	}
	writeJSON(w, map[string]any{"ok": true, "cert_trusted": caTrusted()})
}

func (d *dashboard) certUninstallHandler(w http.ResponseWriter, r *http.Request) {
	if err := uninstallCA(); err != nil {
		writeJSON(w, map[string]any{"ok": false, "error": err.Error()})
		return
	}
	writeJSON(w, map[string]any{"ok": true, "cert_trusted": caTrusted()})
}

func writeJSON(w http.ResponseWriter, v any) {
	w.Header().Set("Content-Type", "application/json; charset=utf-8")
	json.NewEncoder(w).Encode(v)
}

func (d *dashboard) reportHandler(w http.ResponseWriter, r *http.Request) {
	date := r.URL.Query().Get("date")
	if date == "" {
		date = time.Now().In(shanghai).Format("2006-01-02")
	}
	day, err := time.ParseInLocation("2006-01-02", date, shanghai)
	if err != nil {
		http.Error(w, "bad date", http.StatusBadRequest)
		return
	}
	report := buildReport(d.store.snapshot(day, day.Add(24*time.Hour)), date)
	w.Header().Set("Content-Type", "application/json; charset=utf-8")
	json.NewEncoder(w).Encode(report)
}

func buildReport(samples []Sample, date string) Report {
	report := Report{
		Date:        date,
		GeneratedAt: time.Now().In(shanghai).Format(time.RFC3339),
		TotalRequests: len(samples),
		Limitations: []string{
			"分词器指纹粒度到「家族」（GPT o200k / 老 GPT cl100k / 疑似非 GPT），不精确到快照；同族偷换看不出。",
			"「实际模型」= 上游自报名 或 分词器家族推断，非权重身份认证；自报名可被中转改写。",
			"拟合不出任何家族（slope 偏离 1 / 残差过大）只报「疑似非 GPT / 无法判定」，不瞎猜。",
		},
	}
	type groupData struct {
		reported map[string]int
		o2, cl   []float64
		ys       []float64
	}
	groups := map[string]*groupData{}
	breakdown := map[[3]string]int{}

	for _, s := range samples {
		if s.RequestedModel != "" && s.ReportedModel != "" &&
			s.RequestedModel != s.ReportedModel && !isSnapshotOf(s.RequestedModel, s.ReportedModel) {
			report.RoutingSubstitutions++
			breakdown[[3]string{s.RequestedModel, s.ReportedModel, "self_reported"}]++
		}
		if s.OutputTokens > 0 && s.ReasoningTokens > s.OutputTokens {
			report.TokenAnomalies++
		}
		g := groups[s.RequestedModel]
		if g == nil {
			g = &groupData{reported: map[string]int{}}
			groups[s.RequestedModel] = g
		}
		if s.ReportedModel != "" {
			g.reported[s.ReportedModel]++
		}
		if s.InputTokens > 0 && s.PromptTokensO2 > 0 {
			g.o2 = append(g.o2, float64(s.PromptTokensO2))
			g.cl = append(g.cl, float64(s.PromptTokensCl))
			g.ys = append(g.ys, float64(s.InputTokens))
		}
	}

	for model, g := range groups {
		v := fingerprintGroup(g.o2, g.cl, g.ys)
		v.RequestedModel = model
		for m := range g.reported {
			v.ReportedModels = append(v.ReportedModels, m)
		}
		sort.Strings(v.ReportedModels)
		report.Groups = append(report.Groups, v)

		// A fingerprint that names a non-GPT family (or contradicts a GPT model name)
		// is the "clean rename" signal.
		if v.Family == "suspected_non_gpt" {
			breakdown[[3]string{model, "疑似非 GPT 家族", "tokenizer_fingerprint"}] += v.Samples
		}
	}
	sort.Slice(report.Groups, func(i, j int) bool { return report.Groups[i].Samples > report.Groups[j].Samples })

	for k, count := range breakdown {
		report.ActualModels = append(report.ActualModels, ActualModel{
			RequestedModel: k[0], ActualModel: k[1], Evidence: k[2], Count: count,
		})
	}
	sort.Slice(report.ActualModels, func(i, j int) bool { return report.ActualModels[i].Count > report.ActualModels[j].Count })
	if !encodersReady() {
		report.Limitations = append([]string{
			"⚠ 分词器未成功加载，本次无法做指纹判定（仅路由/自报名可用）。请重装或反馈。",
		}, report.Limitations...)
	}
	return report
}

func fingerprintGroup(o2, cl, ys []float64) GroupVerdict {
	v := GroupVerdict{Samples: len(ys)}
	if len(ys) < minFingerprintSamples {
		v.Family, v.FamilyLabel = "insufficient", "样本不足"
		v.Note = fmt.Sprintf("有效样本 %d < %d，拒绝下结论", len(ys), minFingerprintSamples)
		return v
	}
	so2, _, ro2, ok2 := regress(o2, ys)
	scl, _, rcl, okcl := regress(cl, ys)
	if !ok2 || !okcl {
		v.Family, v.FamilyLabel = "insufficient", "样本不足"
		return v
	}
	// Pick the GPT tokenizer that fits best.
	best, bestSlope, bestRMSE, bestLabel := "o200k", so2, ro2, "GPT o200k (4o/5/6)"
	other := rcl
	if rcl < ro2 {
		best, bestSlope, bestRMSE, bestLabel = "cl100k", scl, rcl, "老 GPT cl100k"
		other = ro2
	}
	v.Slope = round(bestSlope, 3)
	v.RMSE = round(bestRMSE, 1)
	slopeOK := bestSlope >= slopeLo && bestSlope <= slopeHi
	separated := other > bestRMSE*separation
	switch {
	case slopeOK:
		v.Family, v.FamilyLabel = best, bestLabel
		if separated {
			v.Note = fmt.Sprintf("上报 input_tokens 最贴合 %s（slope=%.3f）", bestLabel, bestSlope)
		} else {
			v.Note = fmt.Sprintf("贴合 %s，但与另一 GPT 分词器区分度有限", bestLabel)
		}
	default:
		v.Family, v.FamilyLabel = "suspected_non_gpt", "疑似非 GPT 家族"
		v.Note = fmt.Sprintf("上报 token 与 GPT 分词器都对不齐（最佳 slope=%.3f，偏离 1）→ 可能被换成非 GPT 模型（Qwen/DeepSeek 等），或请求体被中转改写", bestSlope)
	}
	return v
}

// regress does ordinary least squares; returns slope, intercept, rmse, ok.
func regress(xs, ys []float64) (float64, float64, float64, bool) {
	n := len(xs)
	if n < 2 || len(ys) != n {
		return 0, 0, 0, false
	}
	var sx, sy float64
	for i := range xs {
		sx += xs[i]
		sy += ys[i]
	}
	mx, my := sx/float64(n), sy/float64(n)
	var sxx, sxy float64
	for i := range xs {
		sxx += (xs[i] - mx) * (xs[i] - mx)
		sxy += (xs[i] - mx) * (ys[i] - my)
	}
	if sxx <= 0 {
		return 0, 0, 0, false
	}
	slope := sxy / sxx
	intercept := my - slope*mx
	var se float64
	for i := range xs {
		d := ys[i] - (slope*xs[i] + intercept)
		se += d * d
	}
	return slope, intercept, math.Sqrt(se / float64(n)), true
}

func round(f float64, places int) float64 {
	p := math.Pow(10, float64(places))
	return math.Round(f*p) / p
}

// isSnapshotOf reports whether reported is an honest, more-specific resolution of the
// requested model rather than a substitution — e.g. request "gpt-5" self-reported as
// "gpt-5-2025-08-01", or the reverse. Providers routinely echo the dated snapshot they
// resolved an alias to; treating that as a routing substitution is a false positive.
func isSnapshotOf(requested, reported string) bool {
	return strings.HasPrefix(reported, requested+"-") || strings.HasPrefix(requested, reported+"-")
}

func (d *dashboard) indexHandler(w http.ResponseWriter, r *http.Request) {
	if r.URL.Path != "/" {
		http.NotFound(w, r)
		return
	}
	w.Header().Set("Content-Type", "text/html; charset=utf-8")
	fmt.Fprint(w, indexHTML)
}
