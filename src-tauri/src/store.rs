//! 抓取样本存储：内存 + 追加式 jsonl（数据留在本机）。
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;

/// 一条经 MITM 观察到的上游请求/响应配对。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sample {
    pub ts: i64,
    pub host: String,
    pub requested_model: String,       // 请求体里的 model
    pub reported_model: String,        // 上游自报（response.completed）
    pub input_tokens: i64,             // 上游上报的 prompt tokens
    pub output_tokens: i64,
    pub reasoning_tokens: i64,
    pub status: u16,
    pub prompt_tokens_o200k: i64,      // 我们对发出正文的 o200k 计数
    pub prompt_tokens_cl100k: i64,
    pub text_chars: i64,
}

pub struct Store {
    samples: Mutex<Vec<Sample>>,
    path: PathBuf,
}

impl Store {
    pub fn new() -> Self {
        let dir = crate::paths::data_dir();
        let path = dir.join("samples.jsonl");
        let mut samples = Vec::new();
        if let Ok(text) = std::fs::read_to_string(&path) {
            for line in text.lines() {
                if line.trim().is_empty() {
                    continue;
                }
                if let Ok(s) = serde_json::from_str::<Sample>(line) {
                    samples.push(s);
                }
            }
        }
        Store { samples: Mutex::new(samples), path }
    }

    pub fn add(&self, sample: Sample) {
        if let Ok(line) = serde_json::to_string(&sample) {
            use std::io::Write;
            if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&self.path) {
                let _ = writeln!(f, "{line}");
            }
        }
        if let Ok(mut v) = self.samples.lock() {
            v.push(sample);
        }
    }

    /// 某天 [start,end) 的样本快照。
    pub fn snapshot(&self, start: i64, end: i64) -> Vec<Sample> {
        self.samples
            .lock()
            .map(|v| v.iter().filter(|s| s.ts >= start && s.ts < end).cloned().collect())
            .unwrap_or_default()
    }
}
