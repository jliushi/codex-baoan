package main

import (
	"math/rand"
	"path/filepath"
	"strings"
	"testing"
)

func TestEncodersLoadOffline(t *testing.T) {
	initEncoders()
	if encO2 == nil || encCl == nil {
		t.Fatal("tiktoken 离线加载失败（o200k/cl100k 为空）")
	}
	n := countTokens(encO2, "hello world 数据库事务 123456789")
	if n == 0 {
		t.Fatal("o200k 计数为 0")
	}
}

func TestParseUsageSSE(t *testing.T) {
	body := "event: response.created\n" +
		"data: {\"type\":\"response.created\",\"response\":{\"model\":\"gpt-6\"}}\n\n" +
		"event: response.completed\n" +
		"data: {\"type\":\"response.completed\",\"response\":{\"model\":\"gpt-5.6-luna\",\"usage\":{\"input_tokens\":123,\"output_tokens\":9,\"output_tokens_details\":{\"reasoning_tokens\":4}}}}\n\n"
	model, in, out, reasoning := parseUsage([]byte(body))
	if model != "gpt-5.6-luna" || in != 123 || out != 9 || reasoning != 4 {
		t.Fatalf("解析错误：model=%s in=%d out=%d reasoning=%d", model, in, out, reasoning)
	}
}

func TestCountableText(t *testing.T) {
	payload := map[string]any{
		"instructions": "sys",
		"input": []any{
			map[string]any{"role": "user", "content": []any{map[string]any{"type": "input_text", "text": "hello"}}},
		},
		"tools": []any{map[string]any{"name": "t"}},
	}
	text := countableText(payload)
	if !strings.Contains(text, "sys") || !strings.Contains(text, "hello") || !strings.Contains(text, "\"name\"") {
		t.Fatalf("countableText 缺内容：%q", text)
	}
}

func TestFingerprintPicksO200k(t *testing.T) {
	initEncoders()
	if encO2 == nil {
		t.Skip("no encoder")
	}
	rng := rand.New(rand.NewSource(7))
	words := strings.Fields("the quick brown fox 数据库 事务 123456789 def foo return αβγ https://example.com/x 缓存 命中")
	var o2, cl, ys []float64
	for i := 0; i < 15; i++ {
		var sb strings.Builder
		for j := 0; j < rng.Intn(300)+40; j++ {
			sb.WriteString(words[rng.Intn(len(words))])
			sb.WriteByte(' ')
		}
		text := sb.String()
		trueTokens := countTokens(encO2, text)
		o2 = append(o2, float64(trueTokens))
		cl = append(cl, float64(countTokens(encCl, text)))
		ys = append(ys, float64(trueTokens+7)) // reported = true o200k + fixed template overhead
	}
	v := fingerprintGroup(o2, cl, ys)
	if v.Family != "o200k" {
		t.Fatalf("期望 o200k，得到 %s（slope=%.3f rmse=%.1f note=%s）", v.Family, v.Slope, v.RMSE, v.Note)
	}
}

func TestFingerprintInsufficient(t *testing.T) {
	v := fingerprintGroup([]float64{1, 2, 3}, []float64{1, 2, 3}, []float64{1, 2, 3})
	if v.Family != "insufficient" {
		t.Fatalf("期望 insufficient，得到 %s", v.Family)
	}
}

// Honest snapshot resolution (alias -> dated snapshot) must not count as a routing
// substitution. Regression for the false positive on normal OpenAI traffic.
func TestSnapshotNotSubstitution(t *testing.T) {
	samples := []Sample{
		{RequestedModel: "gpt-5", ReportedModel: "gpt-5-2025-08-01", InputTokens: 100, OutputTokens: 10},
		{RequestedModel: "gpt-4o", ReportedModel: "gpt-4o-2024-08-06", InputTokens: 100, OutputTokens: 10},
	}
	if got := buildReport(samples, "2026-09-22").RoutingSubstitutions; got != 0 {
		t.Fatalf("快照解析被误判为路由替换：得到 %d，期望 0", got)
	}
	// A genuine cross-family substitution must still be flagged.
	real := []Sample{{RequestedModel: "gpt-5", ReportedModel: "qwen-2.5-72b", InputTokens: 100, OutputTokens: 10}}
	if got := buildReport(real, "2026-09-22").RoutingSubstitutions; got != 1 {
		t.Fatalf("真实替换漏报：得到 %d，期望 1", got)
	}
}

// A non-stream JSON response whose text contains "data:" must still yield model+usage.
func TestParseUsageJSONWithDataURI(t *testing.T) {
	body := `{"model":"gpt-5","usage":{"input_tokens":321,"output_tokens":12},` +
		`"output":[{"content":[{"type":"output_text","text":"see data:image/png;base64,AAAA"}]}]}`
	model, in, out, _ := parseUsage([]byte(body))
	if model != "gpt-5" || in != 321 || out != 12 {
		t.Fatalf("含 data: 的非流式响应解析错误：model=%q in=%d out=%d", model, in, out)
	}
}

// Tool-call arguments and tool output are billed upstream and must be reconstructed.
func TestCountableTextIncludesToolItems(t *testing.T) {
	payload := map[string]any{
		"input": []any{
			map[string]any{"type": "function_call", "name": "run", "arguments": `{"cmd":"pytest -q"}`},
			map[string]any{"type": "function_call_output", "call_id": "c1", "output": "PASSED 42 tests in 3.1s"},
		},
	}
	text := countableText(payload)
	if !strings.Contains(text, "pytest -q") || !strings.Contains(text, "PASSED 42 tests") {
		t.Fatalf("工具调用/输出文本未纳入重建：%q", text)
	}
}

// The guard must never chain its upstream to itself (infinite proxy loop).
func TestSanitizeUpstreamNoSelfLoop(t *testing.T) {
	guard := "http://127.0.0.1:8899"
	if got := sanitizeUpstream(guard, guard); got != "" {
		t.Fatalf("自指上游未被清除：%q", got)
	}
	if got := sanitizeUpstream("http://127.0.0.1:7890", guard); got != "http://127.0.0.1:7890" {
		t.Fatalf("正常上游被误清：%q", got)
	}
}

// In-memory samples are capped so a long-running guard doesn't grow without limit.
func TestStoreTrimsToMaxSamples(t *testing.T) {
	old := maxSamples
	maxSamples = 10
	defer func() { maxSamples = old }()
	s := &Store{path: filepath.Join(t.TempDir(), "samples.jsonl")}
	for i := 0; i < 25; i++ {
		s.add(Sample{RequestedModel: "m", InputTokens: int64(i)})
	}
	if len(s.samples) != 10 {
		t.Fatalf("样本未按上限裁剪：len=%d，期望 10", len(s.samples))
	}
	if s.samples[len(s.samples)-1].InputTokens != 24 {
		t.Fatalf("裁剪丢了最新样本：末条 InputTokens=%d，期望 24", s.samples[len(s.samples)-1].InputTokens)
	}
}

// When tokenizers fail to load, the report must say so instead of silently producing nothing.
func TestReportSurfacesEncoderFailure(t *testing.T) {
	o2, cl := encO2, encCl
	encO2, encCl = nil, nil
	defer func() { encO2, encCl = o2, cl }()
	rep := buildReport(nil, "2026-09-22")
	if len(rep.Limitations) == 0 || !strings.Contains(rep.Limitations[0], "分词器未成功加载") {
		t.Fatalf("编码器失败未在报告中提示：%v", rep.Limitations)
	}
}
