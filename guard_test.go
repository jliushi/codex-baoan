package main

import (
	"math/rand"
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
