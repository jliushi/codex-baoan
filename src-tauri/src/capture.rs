//! MITM 抓取处理器：读发往上游的请求正文（model + 正文文本，本地 o200k/cl100k 计数），
//! 并串流透传响应的同时累积 SSE，在响应结束时解析上游自报模型与 input_tokens，落一条样本。

use crate::store::{Sample, Store};
use chrono::Utc;
use hudsucker::hyper::body::Bytes;
use hudsucker::hyper::{Request, Response};
use hudsucker::{Body, HttpContext, HttpHandler, RequestOrResponse};
use http_body_util::{BodyExt, BodyStream, Full};
use serde_json::Value;
use std::sync::{Arc, OnceLock};
use tiktoken_rs::{cl100k_base, o200k_base, CoreBPE};

fn enc_o200k() -> Option<&'static CoreBPE> {
    static E: OnceLock<Option<CoreBPE>> = OnceLock::new();
    E.get_or_init(|| o200k_base().ok()).as_ref()
}
fn enc_cl100k() -> Option<&'static CoreBPE> {
    static E: OnceLock<Option<CoreBPE>> = OnceLock::new();
    E.get_or_init(|| cl100k_base().ok()).as_ref()
}
fn count(enc: Option<&CoreBPE>, text: &str) -> i64 {
    enc.map(|e| e.encode_ordinary(text).len() as i64).unwrap_or(0)
}

#[derive(Clone, Default)]
struct Pending {
    host: String,
    requested_model: String,
    o200k: i64,
    cl100k: i64,
    text_chars: i64,
    is_target: bool,
}

#[derive(Clone)]
pub struct CaptureHandler {
    store: Arc<Store>,
    pending: Pending,
}

impl CaptureHandler {
    pub fn new(store: Arc<Store>) -> Self {
        CaptureHandler { store, pending: Pending::default() }
    }
}

fn is_target_path(path: &str) -> bool {
    path.contains("/responses") || path.contains("/chat/completions")
}

/// 拼出上游会分词的文本：instructions + input + messages + tools。
fn countable_text(v: &Value) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(s) = v.get("instructions").and_then(Value::as_str) {
        parts.push(s.to_string());
    }
    for key in ["input", "messages"] {
        match v.get(key) {
            Some(Value::String(s)) => parts.push(s.clone()),
            Some(Value::Array(items)) => {
                for it in items {
                    parts.push(text_of_item(it));
                }
            }
            _ => {}
        }
    }
    if let Some(tools) = v.get("tools") {
        if !tools.is_null() {
            parts.push(tools.to_string());
        }
    }
    parts.retain(|p| !p.trim().is_empty());
    parts.join("\n")
}

fn text_of_item(item: &Value) -> String {
    if let Some(s) = item.as_str() {
        return s.to_string();
    }
    let obj = match item.as_object() {
        Some(o) => o,
        None => return String::new(),
    };
    match obj.get("content") {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Array(pieces)) => {
            let mut out = Vec::new();
            for p in pieces {
                if let Some(s) = p.as_str() {
                    out.push(s.to_string());
                } else if let Some(po) = p.as_object() {
                    for k in ["text", "input_text", "output_text"] {
                        if let Some(s) = po.get(k).and_then(Value::as_str) {
                            out.push(s.to_string());
                            break;
                        }
                    }
                }
            }
            out.join("\n")
        }
        _ => obj
            .get("text")
            .or_else(|| obj.get("input_text"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
    }
}

/// 从响应体（SSE 或单个 JSON）解析上游自报模型与用量。
fn parse_usage(body: &str) -> (String, i64, i64, i64) {
    let (mut model, mut input, mut output, mut reasoning) = (String::new(), 0i64, 0i64, 0i64);
    let mut assign = |obj: &Value| {
        let resp = obj.get("response").unwrap_or(obj);
        if let Some(m) = resp.get("model").and_then(Value::as_str) {
            if !m.is_empty() {
                model = m.to_string();
            }
        }
        let usage = resp.get("usage").or_else(|| obj.get("usage"));
        if let Some(u) = usage {
            if let Some(x) = u.get("input_tokens").or_else(|| u.get("prompt_tokens")).and_then(Value::as_i64) {
                input = x;
            }
            if let Some(x) = u.get("output_tokens").or_else(|| u.get("completion_tokens")).and_then(Value::as_i64) {
                output = x;
            }
            if let Some(d) = u.get("output_tokens_details") {
                if let Some(x) = d.get("reasoning_tokens").and_then(Value::as_i64) {
                    reasoning = x;
                }
            }
        }
    };
    if body.contains("data:") {
        for line in body.lines() {
            let line = line.trim();
            if let Some(rest) = line.strip_prefix("data:") {
                let data = rest.trim();
                if data.is_empty() || data == "[DONE]" {
                    continue;
                }
                if let Ok(v) = serde_json::from_str::<Value>(data) {
                    assign(&v);
                }
            }
        }
    } else if let Ok(v) = serde_json::from_str::<Value>(body) {
        assign(&v);
    }
    (model, input, output, reasoning)
}

impl HttpHandler for CaptureHandler {
    async fn handle_request(&mut self, _ctx: &HttpContext, req: Request<Body>) -> RequestOrResponse {
        let path = req.uri().path().to_string();
        let host = req
            .uri()
            .host()
            .map(|h| h.to_string())
            .or_else(|| req.headers().get("host").and_then(|v| v.to_str().ok()).map(|s| s.to_string()))
            .unwrap_or_default();
        if !is_target_path(&path) {
            self.pending = Pending::default();
            return req.into();
        }
        let (parts, body) = req.into_parts();
        let bytes = match body.collect().await {
            Ok(c) => c.to_bytes(),
            Err(_) => Bytes::new(),
        };
        let mut p = Pending { host, is_target: true, ..Default::default() };
        if let Ok(v) = serde_json::from_slice::<Value>(&bytes) {
            p.requested_model = v.get("model").and_then(Value::as_str).unwrap_or("").to_string();
            let text = countable_text(&v);
            p.text_chars = text.chars().count() as i64;
            p.o200k = count(enc_o200k(), &text);
            p.cl100k = count(enc_cl100k(), &text);
        }
        self.pending = p;
        RequestOrResponse::Request(Request::from_parts(parts, Body::from(Full::new(bytes))))
    }

    async fn handle_response(&mut self, _ctx: &HttpContext, res: Response<Body>) -> Response<Body> {
        if !self.pending.is_target {
            return res;
        }
        let pending = std::mem::take(&mut self.pending);
        let store = self.store.clone();
        let (parts, body) = res.into_parts();
        let status = parts.status.as_u16();
        Response::from_parts(parts, Body::from_stream(tee_and_record(body, pending, store, status)))
    }
}

/// 串流透传响应，同时把 SSE 累积下来，在结束时解析用量并落样本。
/// 显式返回类型固定错误类型，避免 `Body::from_stream` 的类型推断歧义。
fn tee_and_record(
    body: Body,
    pending: Pending,
    store: Arc<Store>,
    status: u16,
) -> impl futures::Stream<Item = Result<Bytes, hudsucker::Error>> {
    let acc = Arc::new(std::sync::Mutex::new(Vec::<u8>::new()));
    async_stream::try_stream! {
        use futures::StreamExt;
        let mut s = std::pin::pin!(BodyStream::new(body));
        while let Some(frame) = s.next().await {
            let frame = frame?;
            if let Ok(data) = frame.into_data() {
                acc.lock().unwrap().extend_from_slice(&data);
                yield data;
            }
        }
        let text = { let b = acc.lock().unwrap(); String::from_utf8_lossy(&b).into_owned() };
        let (model, input, output, reasoning) = parse_usage(&text);
        store.add(Sample {
            ts: Utc::now().timestamp(),
            host: pending.host.clone(),
            requested_model: pending.requested_model.clone(),
            reported_model: model,
            input_tokens: input,
            output_tokens: output,
            reasoning_tokens: reasoning,
            status,
            prompt_tokens_o200k: pending.o200k,
            prompt_tokens_cl100k: pending.cl100k,
            text_chars: pending.text_chars,
        });
    }
}

