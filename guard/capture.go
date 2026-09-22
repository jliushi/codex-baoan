package main

import (
	"bytes"
	"encoding/json"
	"io"
	"log"
	"net/http"
	"strings"
	"sync"
	"time"

	"github.com/elazarl/goproxy"
	"github.com/pkoukk/tiktoken-go"
	tiktokenLoader "github.com/pkoukk/tiktoken-go-loader"
)

var (
	encOnce sync.Once
	encO2   *tiktoken.Tiktoken
	encCl   *tiktoken.Tiktoken
)

func initEncoders() {
	encOnce.Do(func() {
		tiktoken.SetBpeLoader(tiktokenLoader.NewOfflineLoader())
		var err error
		if encO2, err = tiktoken.GetEncoding("o200k_base"); err != nil {
			log.Printf("o200k 分词器加载失败：%v", err)
		}
		if encCl, err = tiktoken.GetEncoding("cl100k_base"); err != nil {
			log.Printf("cl100k 分词器加载失败：%v", err)
		}
	})
}

func countTokens(enc *tiktoken.Tiktoken, text string) int {
	if enc == nil {
		return 0
	}
	return len(enc.Encode(text, nil, nil))
}

// pending carries request-side data from the OnRequest hook to the OnResponse hook.
type pending struct {
	host           string
	path           string
	requestedModel string
	text           string
}

// isTarget matches the LLM request paths we care about.
func isTarget(path string) bool {
	return strings.Contains(path, "/responses") || strings.Contains(path, "/chat/completions")
}

func installCapture(proxy *goproxy.ProxyHttpServer, store *Store) {
	initEncoders()

	proxy.OnRequest().DoFunc(func(req *http.Request, ctx *goproxy.ProxyCtx) (*http.Request, *http.Response) {
		if req.Body == nil || !isTarget(req.URL.Path) {
			return req, nil
		}
		body, err := io.ReadAll(req.Body)
		req.Body.Close()
		req.Body = io.NopCloser(bytes.NewReader(body))
		if err != nil {
			return req, nil
		}
		p := &pending{host: req.URL.Host, path: req.URL.Path}
		if req.URL.Host == "" {
			p.host = req.Host
		}
		var payload map[string]any
		if json.Unmarshal(body, &payload) == nil {
			p.requestedModel, _ = payload["model"].(string)
			p.text = countableText(payload)
		}
		ctx.UserData = p
		return req, nil
	})

	proxy.OnResponse().DoFunc(func(resp *http.Response, ctx *goproxy.ProxyCtx) *http.Response {
		p, ok := ctx.UserData.(*pending)
		if !ok || resp == nil || resp.Body == nil {
			return resp
		}
		status := resp.StatusCode
		orig := resp.Body
		buf := &bytes.Buffer{}
		resp.Body = &teeCloser{r: io.TeeReader(orig, buf), c: orig, onClose: func() {
			reportedModel, in, out, reasoning := parseUsage(buf.Bytes())
			sample := Sample{
				Time:            time.Now(),
				Host:            p.host,
				Path:            p.path,
				RequestedModel:  p.requestedModel,
				ReportedModel:   reportedModel,
				InputTokens:     in,
				OutputTokens:    out,
				ReasoningTokens: reasoning,
				StatusCode:      status,
				TextChars:       len(p.text),
			}
			if p.text != "" {
				sample.PromptTokensO2 = countTokens(encO2, p.text)
				sample.PromptTokensCl = countTokens(encCl, p.text)
			}
			store.add(sample)
		}}
		return resp
	})
}

// teeCloser tees reads into a buffer and fires onClose exactly once when the client
// finishes reading the streamed response (preserving SSE streaming to Codex).
type teeCloser struct {
	r       io.Reader
	c       io.Closer
	onClose func()
	once    sync.Once
}

func (t *teeCloser) Read(p []byte) (int, error) { return t.r.Read(p) }
func (t *teeCloser) Close() error {
	t.once.Do(t.onClose)
	return t.c.Close()
}

// countableText reconstructs the text the upstream tokenizes: instructions + input +
// messages + serialized tools. Same string basis used for every candidate tokenizer.
func countableText(payload map[string]any) string {
	var parts []string
	if s, ok := payload["instructions"].(string); ok && s != "" {
		parts = append(parts, s)
	}
	parts = append(parts, textOfField(payload["input"])...)
	parts = append(parts, textOfField(payload["messages"])...)
	if tools, ok := payload["tools"]; ok && tools != nil {
		if b, err := json.Marshal(tools); err == nil {
			parts = append(parts, string(b))
		}
	}
	return strings.Join(nonEmpty(parts), "\n")
}

func textOfField(field any) []string {
	switch v := field.(type) {
	case string:
		return []string{v}
	case []any:
		var out []string
		for _, item := range v {
			out = append(out, textOfItem(item))
		}
		return out
	default:
		return nil
	}
}

func textOfItem(item any) string {
	m, ok := item.(map[string]any)
	if !ok {
		if s, ok := item.(string); ok {
			return s
		}
		return ""
	}
	switch content := m["content"].(type) {
	case string:
		return content
	case []any:
		var parts []string
		for _, piece := range content {
			if pm, ok := piece.(map[string]any); ok {
				for _, key := range []string{"text", "input_text", "output_text"} {
					if s, ok := pm[key].(string); ok && s != "" {
						parts = append(parts, s)
						break
					}
				}
			} else if s, ok := piece.(string); ok {
				parts = append(parts, s)
			}
		}
		return strings.Join(parts, "\n")
	}
	for _, key := range []string{"text", "input_text"} {
		if s, ok := m[key].(string); ok {
			return s
		}
	}
	return ""
}

func nonEmpty(in []string) []string {
	out := in[:0]
	for _, s := range in {
		if strings.TrimSpace(s) != "" {
			out = append(out, s)
		}
	}
	return out
}

// parseUsage extracts the upstream self-reported model and token usage from a response
// body: SSE stream (terminal response.completed wins) or a single JSON object.
func parseUsage(body []byte) (model string, input, output, reasoning int64) {
	assign := func(obj map[string]any) {
		resp := obj
		if r, ok := obj["response"].(map[string]any); ok {
			resp = r
		}
		if m, ok := resp["model"].(string); ok && m != "" {
			model = m
		}
		usage, ok := resp["usage"].(map[string]any)
		if !ok {
			usage, _ = obj["usage"].(map[string]any)
		}
		if usage == nil {
			return
		}
		input = firstInt(usage, "input_tokens", "prompt_tokens", input)
		output = firstInt(usage, "output_tokens", "completion_tokens", output)
		if details, ok := usage["output_tokens_details"].(map[string]any); ok {
			reasoning = firstInt(details, "reasoning_tokens", "", reasoning)
		}
	}
	text := string(body)
	if strings.Contains(text, "data:") {
		for _, line := range strings.Split(text, "\n") {
			line = strings.TrimSpace(line)
			data, ok := strings.CutPrefix(line, "data:")
			if !ok {
				continue
			}
			data = strings.TrimSpace(data)
			if data == "" || data == "[DONE]" {
				continue
			}
			var obj map[string]any
			if json.Unmarshal([]byte(data), &obj) == nil {
				assign(obj)
			}
		}
		return
	}
	var obj map[string]any
	if json.Unmarshal(body, &obj) == nil {
		assign(obj)
	}
	return
}

func firstInt(m map[string]any, key, alt string, fallback int64) int64 {
	for _, k := range []string{key, alt} {
		if k == "" {
			continue
		}
		if f, ok := m[k].(float64); ok {
			return int64(f)
		}
	}
	return fallback
}
