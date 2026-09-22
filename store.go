package main

import (
	"bufio"
	"encoding/json"
	"os"
	"path/filepath"
	"sync"
	"time"
)

// Sample is one captured upstream request/response pair.
type Sample struct {
	Time            time.Time `json:"time"`
	Host            string    `json:"host"`
	Path            string    `json:"path"`
	RequestedModel  string    `json:"requested_model"`  // model field CC Switch sent upstream
	ReportedModel   string    `json:"reported_model"`   // upstream self-reported (response.completed)
	InputTokens     int64     `json:"input_tokens"`     // upstream-reported prompt tokens
	OutputTokens    int64     `json:"output_tokens"`
	ReasoningTokens int64     `json:"reasoning_tokens"`
	StatusCode      int       `json:"status_code"`
	PromptTokensO2  int       `json:"prompt_tokens_o200k"` // our o200k count of the exact sent text
	PromptTokensCl  int       `json:"prompt_tokens_cl100k"`
	TextChars       int       `json:"text_chars"`
}

type Store struct {
	mu       sync.RWMutex
	samples  []Sample
	path     string
	lastSeen time.Time
}

// maxSamples bounds the in-memory sample slice so a long-running guard doesn't grow
// without limit (the on-disk jsonl stays append-only). Kept high enough that browsing
// recent days is unaffected; only the oldest samples are dropped once exceeded. Var,
// not const, so tests can shrink it.
var maxSamples = 200000

// trimLocked drops the oldest samples once the slice exceeds maxSamples. Caller holds mu.
func (s *Store) trimLocked() {
	if len(s.samples) > maxSamples {
		s.samples = append(s.samples[:0], s.samples[len(s.samples)-maxSamples:]...)
	}
}

func (s *Store) LastSeen() time.Time {
	s.mu.RLock()
	defer s.mu.RUnlock()
	return s.lastSeen
}

func newStore() *Store {
	dir, _ := dataDir()
	return &Store{path: filepath.Join(dir, "samples.jsonl")}
}

func (s *Store) load() error {
	f, err := os.Open(s.path)
	if err != nil {
		if os.IsNotExist(err) {
			return nil
		}
		return err
	}
	defer f.Close()
	sc := bufio.NewScanner(f)
	sc.Buffer(make([]byte, 1024*1024), 8*1024*1024)
	for sc.Scan() {
		line := sc.Bytes()
		if len(line) == 0 {
			continue
		}
		var sample Sample
		if json.Unmarshal(line, &sample) == nil {
			s.samples = append(s.samples, sample)
		}
	}
	s.trimLocked()
	return sc.Err()
}

func (s *Store) add(sample Sample) {
	s.mu.Lock()
	s.samples = append(s.samples, sample)
	s.trimLocked()
	s.lastSeen = time.Now()
	s.mu.Unlock()
	// Append-only persistence; best effort.
	if f, err := os.OpenFile(s.path, os.O_CREATE|os.O_WRONLY|os.O_APPEND, 0o600); err == nil {
		if b, err := json.Marshal(sample); err == nil {
			f.Write(append(b, '\n'))
		}
		f.Close()
	}
}

// snapshot returns samples within the given local-day [start,end).
func (s *Store) snapshot(start, end time.Time) []Sample {
	s.mu.RLock()
	defer s.mu.RUnlock()
	out := make([]Sample, 0, len(s.samples))
	for _, sample := range s.samples {
		if !sample.Time.Before(start) && sample.Time.Before(end) {
			out = append(out, sample)
		}
	}
	return out
}
