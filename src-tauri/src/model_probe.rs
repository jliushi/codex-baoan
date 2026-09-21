//! 模型路由/替换检测。借用 sub2api 的 upstreamResponseModelObserver 思路：
//! 只发一次探测请求，读上游「自报的真实 model」和请求的模型名比对。
//! 终结事件(response.completed 等)的自报优先于中间帧；出现互相矛盾则标 conflict。
//! 不做题库、不做行为指纹——纯读上游招供的模型名。

use serde::Serialize;
use serde_json::Value;
use std::time::Duration;

const MAX_MODEL_LEN: usize = 200;

/// 观察器：镜像 sub2api 的 first/terminal/conflict 三字段。
#[derive(Default)]
struct ModelObserver {
    first: Option<String>,
    terminal: Option<String>,
    conflict: bool,
    seen: Vec<String>,
}

impl ModelObserver {
    fn observe(&mut self, model: &str, terminal: bool) {
        let model = model.trim();
        if model.is_empty() {
            return;
        }
        let model: String = model.chars().take(MAX_MODEL_LEN).collect();
        if let Some(current) = self.resolved() {
            if !current.eq_ignore_ascii_case(&model) {
                self.conflict = true;
            }
        }
        if !self.seen.iter().any(|m| m.eq_ignore_ascii_case(&model)) {
            self.seen.push(model.clone());
        }
        if terminal {
            self.terminal = Some(model);
        } else if self.first.is_none() {
            self.first = Some(model);
        }
    }

    /// 终帧优先，否则取首个声明。
    fn resolved(&self) -> Option<String> {
        self.terminal.clone().or_else(|| self.first.clone())
    }
}

fn is_terminal_event(event: &str) -> bool {
    matches!(
        event.trim(),
        "response.completed"
            | "response.done"
            | "response.failed"
            | "response.incomplete"
            | "response.cancelled"
            | "response.canceled"
    )
}

/// 从一个 JSON 事件里抠 model：优先 response.model，回退顶层 model。
fn model_from_json(value: &Value) -> Option<&str> {
    value
        .get("response")
        .and_then(|r| r.get("model"))
        .and_then(Value::as_str)
        .or_else(|| value.get("model").and_then(Value::as_str))
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

/// 把响应体喂进观察器。SSE 流式逐事件解析；非流式当作单个终结 JSON。
fn observe_body(body: &str, observer: &mut ModelObserver) {
    if body.contains("data:") {
        let mut cur_event = String::new();
        for line in body.lines() {
            if let Some(rest) = line.strip_prefix("event:") {
                cur_event = rest.trim().to_string();
                continue;
            }
            let Some(data) = line.strip_prefix("data:") else {
                continue;
            };
            let data = data.trim();
            if data.is_empty() || data == "[DONE]" {
                continue;
            }
            let Ok(json) = serde_json::from_str::<Value>(data) else {
                continue;
            };
            let event = json
                .get("type")
                .and_then(Value::as_str)
                .unwrap_or(&cur_event);
            if let Some(model) = model_from_json(&json) {
                observer.observe(model, is_terminal_event(event));
            }
        }
    } else if let Ok(json) = serde_json::from_str::<Value>(body) {
        if let Some(model) = model_from_json(&json) {
            observer.observe(model, true);
        }
    }
}

/// 检测结果，返回给前端。
#[derive(Serialize)]
pub struct ModelRoutingResult {
    /// 请求的模型名
    pub requested_model: String,
    /// 上游自报的真实模型（终帧优先）；None = 上游没自报
    pub reported_model: Option<String>,
    /// match=一致 / mismatch=被路由或替换 / unknown=上游没自报 / error=请求失败
    pub verdict: String,
    /// 中间帧与终帧自报不一致
    pub conflict: bool,
    /// 出现过的所有 model 值（去重），供前端展示
    pub observed: Vec<String>,
    /// 探测用的端点（脱敏后）
    pub endpoint: String,
    /// HTTP 状态码（有请求时）
    pub http_status: Option<u16>,
    /// 人类可读说明
    pub detail: String,
}

fn verdict_for(requested: &str, reported: &Option<String>) -> (String, String) {
    match reported {
        None => (
            "unknown".into(),
            "上游未在响应里自报模型，无法判断（sub2api 同款盲区）。".into(),
        ),
        Some(real) if real.eq_ignore_ascii_case(requested.trim()) => (
            "match".into(),
            "上游自报模型与请求一致。".into(),
        ),
        Some(real) => (
            "mismatch".into(),
            format!(
                "上游返回的模型名 {real} 与请求模型 {requested} 不一致 → 疑似被路由/替换。\
                 注意：模型别名或日期版本(如快照名)也可能造成名称差异，不能仅凭此警示认定模型被替换。"
            ),
        ),
    }
}

/// 核心探测：向 {base_url}/responses 发一次最小流式请求，读自报模型。
pub fn probe(base_url: &str, api_key: &str, model: &str) -> ModelRoutingResult {
    let endpoint = crate::codex_config::redact_url(base_url);
    let url = format!("{}/responses", base_url.trim_end_matches('/'));
    let mut result = ModelRoutingResult {
        requested_model: model.to_string(),
        reported_model: None,
        verdict: "error".into(),
        conflict: false,
        observed: vec![],
        endpoint,
        http_status: None,
        detail: String::new(),
    };

    let client = match reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
    {
        Ok(client) => client,
        Err(error) => {
            result.detail = format!("HTTP 客户端初始化失败：{error}");
            return result;
        }
    };

    let payload = serde_json::json!({
        "model": model,
        "input": "hi",
        "stream": true,
        "store": false,
    });

    let response = client
        .post(&url)
        .bearer_auth(api_key)
        .header("accept", "text/event-stream")
        .json(&payload)
        .send();

    let response = match response {
        Ok(response) => response,
        Err(error) => {
            // reqwest 的 Display 不含 bearer，安全；但仍保守不回显 url 查询串
            result.detail = format!("请求发送失败：{error}");
            return result;
        }
    };

    let status = response.status();
    result.http_status = Some(status.as_u16());
    let body = response.text().unwrap_or_default();

    if !status.is_success() {
        result.detail = format!("上游返回 HTTP {}：{}", status.as_u16(), truncate(&body, 200));
        return result;
    }

    let mut observer = ModelObserver::default();
    observe_body(&body, &mut observer);
    result.reported_model = observer.resolved();
    result.conflict = observer.conflict;
    result.observed = observer.seen;
    let (verdict, mut detail) = verdict_for(model, &result.reported_model);
    result.verdict = verdict;
    if result.conflict {
        detail.push_str(" 另：中间帧与终帧自报不一致（conflict）。");
    }
    result.detail = detail;
    result
}

fn truncate(text: &str, max: usize) -> String {
    let text = text.trim();
    if text.chars().count() <= max {
        text.to_string()
    } else {
        text.chars().take(max).collect::<String>() + "…"
    }
}

/// Tauri 命令：不传参数时读本机 Codex 配置(base_url + 明文 key + model)自动探测；
/// 也可显式传 base_url/api_key/model 覆盖。api_key 始终留在后端，不回传前端。
#[tauri::command]
pub(crate) fn detect_model_routing(
    model: Option<String>,
    base_url: Option<String>,
    api_key: Option<String>,
) -> Result<ModelRoutingResult, String> {
    // 缺哪个补哪个：从本机 Codex 配置解析
    let (base_url, api_key, model) = if base_url.is_some() && api_key.is_some() && model.is_some() {
        (base_url.unwrap(), api_key.unwrap(), model.unwrap())
    } else {
        let config = crate::codex_config::resolve_active()?;
        let base_url = base_url
            .or(config.base_url)
            .ok_or("未能确定上游地址")?;
        // 凭据回退顺序：显式传入 → 解析出的 API Key → ChatGPT 订阅 OAuth token
        let api_key = api_key
            .or(config.api_key)
            .or_else(crate::codex_config::active_bearer_token)
            .ok_or("未能取得凭据（API Key 或 ChatGPT 登录态）")?;
        let model = model.or(config.model).ok_or("未配置要探测的模型名")?;
        (base_url, api_key, model)
    };
    Ok(probe(&base_url, &api_key, &model))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_declaration_wins_over_intermediate() {
        let mut o = ModelObserver::default();
        o.observe("gpt-6", false); // 中间帧回显请求名
        o.observe("gpt-5.6-luna", true); // 终帧才是真身
        assert_eq!(o.resolved().as_deref(), Some("gpt-5.6-luna"));
        assert!(o.conflict);
    }

    #[test]
    fn consistent_declarations_have_no_conflict() {
        let mut o = ModelObserver::default();
        o.observe("gpt-6-astra", false);
        o.observe("gpt-6-astra", true);
        assert_eq!(o.resolved().as_deref(), Some("gpt-6-astra"));
        assert!(!o.conflict);
    }

    #[test]
    fn sse_stream_extracts_terminal_model() {
        let body = "event: response.created\ndata: {\"type\":\"response.created\",\"response\":{\"model\":\"gpt-6\"}}\n\nevent: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{\"model\":\"gpt-5.6-luna\"}}\n\n";
        let mut o = ModelObserver::default();
        observe_body(body, &mut o);
        assert_eq!(o.resolved().as_deref(), Some("gpt-5.6-luna"));
        assert!(o.conflict);
    }

    #[test]
    fn non_streaming_single_json() {
        let body = "{\"model\":\"gpt-5.5\",\"usage\":{}}";
        let mut o = ModelObserver::default();
        observe_body(body, &mut o);
        assert_eq!(o.resolved().as_deref(), Some("gpt-5.5"));
    }

    #[test]
    fn verdict_detects_mismatch_case_insensitively() {
        let (v, _) = verdict_for("GPT-6", &Some("gpt-5.6-luna".into()));
        assert_eq!(v, "mismatch");
        let (v, _) = verdict_for("gpt-6-astra", &Some("GPT-6-Astra".into()));
        assert_eq!(v, "match");
        let (v, _) = verdict_for("gpt-6", &None);
        assert_eq!(v, "unknown");
    }
}
