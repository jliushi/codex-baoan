//! 模型审计日报：把「一天的日志」汇总成——总请求数、异常请求数（路由/替换、
//! Token 用量矛盾、接受无效模型、疑似降智）、以及异常请求实际是什么模型。
//!
//! 两条本地数据源，都不需要代理：
//!   - CC Switch 代理库 `cc-switch.db` 的 `proxy_request_logs`：能拿到上游在响应里
//!     自报的模型名（`model` 列，来自 `response.completed`），与请求名不一致即路由/替换，
//!     此时自报名就是实际模型。
//!   - Codex 会话 rollout 日志 `rollout-*.jsonl`：每轮的请求模型、推理档位与用量，
//!     用于疑似降智与 Token 矛盾（含绕过 CC Switch 的直连流量）。
//!
//! 不做的事：分词器指纹（识破「连名字都改干净」）留待后续更新；本模块只依据日志已有
//! 字段，绝不声称认证了模型权重身份。

use chrono::{DateTime, FixedOffset, NaiveDate, TimeZone, Utc};
use rusqlite::{Connection, OpenFlags};
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const TZ_OFFSET_SECS: i32 = 8 * 3600; // Asia/Shanghai, UTC+08:00
const MAX_ROWS: usize = 1000;

fn shanghai() -> FixedOffset {
    FixedOffset::east_opt(TZ_OFFSET_SECS).expect("valid offset")
}

/// [start, end) unix-second window for a YYYY-MM-DD date in Asia/Shanghai.
fn day_window(date: &str) -> Result<(i64, i64), String> {
    let day = NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .map_err(|_| format!("日期格式应为 YYYY-MM-DD：{date}"))?;
    let midnight = day.and_hms_opt(0, 0, 0).ok_or("无法构造当日零点")?;
    let start = shanghai()
        .from_local_datetime(&midnight)
        .single()
        .ok_or("时区换算失败")?
        .timestamp();
    Ok((start, start + 86_400))
}

fn today_shanghai() -> String {
    Utc::now()
        .with_timezone(&shanghai())
        .format("%Y-%m-%d")
        .to_string()
}

fn fmt_time(ts: i64) -> String {
    shanghai()
        .timestamp_opt(ts, 0)
        .single()
        .map(|dt| dt.format("%H:%M:%S").to_string())
        .unwrap_or_default()
}

// -------- unified record --------

#[derive(Debug, Clone)]
struct Record {
    id: String,
    source: &'static str, // "ccswitch" | "codex"
    ts: i64,
    requested_model: Option<String>,
    claimed_model: Option<String>, // 上游自报，仅 CC Switch 有
    provider: Option<String>,
    status_code: Option<i64>,
    input_tokens: Option<i64>,
    output_tokens: Option<i64>,
    cached: Option<i64>,
    cache_write: Option<i64>,
    reasoning_tokens: Option<i64>,
    reasoning_effort: Option<String>,
    issues: Vec<String>,
}

fn is_invalid_model(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.starts_with("audit-nonexistent") || lower.starts_with("audit-invalid")
}

fn is_error(status: Option<i64>) -> bool {
    matches!(status, Some(code) if code == 0 || code >= 400)
}

fn is_success(status: Option<i64>) -> bool {
    matches!(status, Some(code) if (200..300).contains(&code))
}

fn is_mismatch(record: &Record) -> bool {
    match (&record.requested_model, &record.claimed_model) {
        (Some(req), Some(claimed)) => !claimed.is_empty() && req != claimed,
        _ => false,
    }
}

// -------- CC Switch source --------

fn ccswitch_db_path() -> Option<PathBuf> {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let candidates = [
        dirs::data_dir().map(|p| p.join("cc-switch").join("cc-switch.db")),
        dirs::config_dir().map(|p| p.join("cc-switch").join("cc-switch.db")),
        Some(home.join(".cc-switch").join("cc-switch.db")),
    ];
    candidates.into_iter().flatten().find(|p| p.exists())
}

fn read_ccswitch(path: &Path, start: i64, end: i64) -> Result<Vec<Record>, String> {
    let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|e| e.to_string())?;

    // provider id -> name
    let mut names: BTreeMap<String, String> = BTreeMap::new();
    if let Ok(mut stmt) = conn.prepare("SELECT id, name FROM providers WHERE app_type='codex'") {
        if let Ok(rows) = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
        }) {
            for row in rows.flatten() {
                if let Some(name) = row.1 {
                    names.insert(row.0, name);
                }
            }
        }
    }

    let mut stmt = conn
        .prepare(
            "SELECT request_id, request_model, model, provider_id, input_tokens, output_tokens, \
             cache_read_tokens, cache_creation_tokens, input_token_semantics, status_code, created_at \
             FROM proxy_request_logs WHERE app_type='codex' AND created_at>=? AND created_at<? \
             ORDER BY created_at",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([start, end], |row| {
            Ok((
                row.get::<_, Option<String>>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<i64>>(4)?,
                row.get::<_, Option<i64>>(5)?,
                row.get::<_, Option<i64>>(6)?,
                row.get::<_, Option<i64>>(7)?,
                row.get::<_, Option<i64>>(8)?,
                row.get::<_, Option<i64>>(9)?,
                row.get::<_, Option<i64>>(10)?,
            ))
        })
        .map_err(|e| e.to_string())?;

    let mut out = Vec::new();
    for row in rows {
        let (
            req_id,
            request_model,
            model,
            provider_id,
            input,
            output,
            read,
            write,
            semantics,
            status,
            created,
        ) = row.map_err(|e| e.to_string())?;
        let read = read.unwrap_or(0);
        let write = write.unwrap_or(0);
        let semantics = semantics.unwrap_or(0);
        let raw_input = input.unwrap_or(0);
        let mut issues = Vec::new();
        if raw_input < 0 || output.unwrap_or(0) < 0 || read < 0 || write < 0 {
            issues.push("negative_token_claim".to_string());
        }
        // semantics: 1=total inclusive, 2=fresh, 0=legacy(includes cache-read only)
        let total_input = match semantics {
            2 => raw_input + read + write,
            1 => {
                if read + write > raw_input {
                    issues.push("cache_exceeds_input".to_string());
                }
                raw_input
            }
            _ => {
                if read > raw_input {
                    issues.push("cache_exceeds_input".to_string());
                }
                raw_input + write
            }
        };
        let provider = provider_id
            .as_ref()
            .and_then(|id| names.get(id).cloned())
            .or(provider_id.clone());
        out.push(Record {
            id: req_id.unwrap_or_else(|| format!("ccswitch:{}", created.unwrap_or(0))),
            source: "ccswitch",
            ts: created.unwrap_or(0),
            requested_model: request_model,
            claimed_model: model,
            provider,
            status_code: status,
            input_tokens: Some(total_input),
            output_tokens: output,
            cached: Some(read),
            cache_write: Some(write),
            reasoning_tokens: None,
            reasoning_effort: None,
            issues,
        });
    }
    Ok(out)
}

// -------- Codex rollout source --------

fn collect_rollouts(root: &Path, out: &mut Vec<PathBuf>) {
    let entries = match std::fs::read_dir(root) {
        Ok(entries) => entries,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_rollouts(&path, out);
        } else if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if name.starts_with("rollout-") && name.ends_with(".jsonl") {
                out.push(path);
            }
        }
    }
}

/// Filename embeds the session start date: rollout-YYYY-MM-DDT...jsonl
fn rollout_date_prefix(path: &Path) -> Option<String> {
    let name = path.file_name()?.to_str()?;
    let rest = name.strip_prefix("rollout-")?;
    rest.get(0..10).map(|s| s.to_string())
}

fn parse_ts(value: &Value) -> Option<i64> {
    let raw = value.get("timestamp")?.as_str()?;
    DateTime::parse_from_rfc3339(raw)
        .ok()
        .map(|dt| dt.with_timezone(&Utc).timestamp())
}

fn read_rollouts(codex_home: &Path, start: i64, end: i64) -> Vec<Record> {
    let mut files = Vec::new();
    collect_rollouts(&codex_home.join("sessions"), &mut files);
    collect_rollouts(&codex_home.join("archived_sessions"), &mut files);

    // Only files whose start date is within [day-1, day+1]; a session may span midnight.
    let lo = shanghai().timestamp_opt(start - 86_400, 0).single();
    let hi = shanghai().timestamp_opt(end + 86_400, 0).single();
    let (lo, hi) = match (lo, hi) {
        (Some(a), Some(b)) => (
            a.format("%Y-%m-%d").to_string(),
            b.format("%Y-%m-%d").to_string(),
        ),
        _ => return Vec::new(),
    };

    let mut out = Vec::new();
    for path in files {
        if let Some(prefix) = rollout_date_prefix(&path) {
            if prefix < lo || prefix > hi {
                continue;
            }
        }
        parse_rollout_file(&path, start, end, &mut out);
    }
    out
}

fn parse_rollout_file(path: &Path, start: i64, end: i64, out: &mut Vec<Record>) {
    let content = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(_) => return,
    };
    let session = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("rollout")
        .to_string();
    let mut model: Option<String> = None;
    let mut effort: Option<String> = None;
    let mut turn = 0usize;
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let value: Value = match serde_json::from_str(line) {
            Ok(value) => value,
            Err(_) => continue,
        };
        let kind = value.get("type").and_then(Value::as_str).unwrap_or("");
        let payload = value.get("payload").cloned().unwrap_or(Value::Null);
        match kind {
            "turn_context" => {
                if let Some(m) = payload.get("model").and_then(Value::as_str) {
                    model = Some(m.to_string());
                }
                effort = payload
                    .get("effort")
                    .and_then(Value::as_str)
                    .or_else(|| {
                        payload
                            .get("collaboration_mode")
                            .and_then(|c| c.get("settings"))
                            .and_then(|s| s.get("reasoning_effort"))
                            .and_then(Value::as_str)
                    })
                    .map(|s| s.to_string());
            }
            "event_msg" if payload.get("type").and_then(Value::as_str) == Some("token_count") => {
                let usage = payload
                    .get("info")
                    .and_then(|i| i.get("last_token_usage"))
                    .cloned();
                let usage = match usage {
                    Some(Value::Object(map)) if !map.is_empty() => Value::Object(map),
                    _ => continue, // empty terminal token_count carries no per-turn usage
                };
                let ts = parse_ts(&value).unwrap_or(0);
                if ts < start || ts >= end {
                    continue;
                }
                let get = |key: &str| usage.get(key).and_then(Value::as_i64);
                let input = get("input_tokens").unwrap_or(0);
                let read = get("cached_input_tokens").unwrap_or(0);
                let write = get("cache_write_input_tokens").unwrap_or(0);
                let output = get("output_tokens").unwrap_or(0);
                let reasoning = get("reasoning_output_tokens").unwrap_or(0);
                let mut issues = Vec::new();
                if read > input {
                    issues.push("cache_exceeds_input".to_string());
                }
                if reasoning > output && output > 0 {
                    issues.push("reasoning_exceeds_output".to_string());
                }
                turn += 1;
                out.push(Record {
                    id: format!("{session}#{turn}"),
                    source: "codex",
                    ts,
                    requested_model: model.clone(),
                    claimed_model: None,
                    provider: None,
                    status_code: Some(200),
                    input_tokens: Some(input),
                    output_tokens: Some(output),
                    cached: Some(read),
                    cache_write: Some(write),
                    reasoning_tokens: Some(reasoning),
                    reasoning_effort: effort.clone(),
                    issues,
                });
            }
            _ => {}
        }
    }
}

// -------- classification + report --------

#[derive(Serialize)]
pub struct SourceInfo {
    pub id: String,
    pub label: String,
    pub path: String,
    pub available: bool,
    pub records: usize,
}

#[derive(Serialize)]
pub struct AnomalyCategory {
    pub key: String,
    pub label: String,
    pub count: usize,
}

#[derive(Serialize)]
pub struct BreakdownRow {
    pub requested_model: String,
    pub actual_model: String,
    pub evidence_level: String, // self_reported | undetermined
    pub count: usize,
}

#[derive(Serialize)]
pub struct ReportRow {
    pub id: String,
    pub time: String,
    pub source: String,
    pub provider: String,
    pub requested_model: String,
    pub actual_model: String,
    pub evidence_level: String,
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub reasoning_tokens: Option<i64>,
    pub status_code: Option<i64>,
    pub categories: Vec<String>,
}

#[derive(Serialize)]
pub struct Tokens {
    pub input: i64,
    pub output: i64,
    pub total: i64,
}

#[derive(Serialize)]
pub struct DailyAudit {
    pub date: String,
    pub generated_at: String,
    pub sources: Vec<SourceInfo>,
    pub total_requests: usize,
    pub analyzed_requests: usize,
    pub errors: usize,
    pub anomaly_total: usize,
    pub clean_requests: usize,
    pub anomalies: Vec<AnomalyCategory>,
    pub actual_model_breakdown: Vec<BreakdownRow>,
    pub tokens: Tokens,
    pub rows: Vec<ReportRow>,
    pub limitations: Vec<String>,
}

fn actual_model(record: &Record) -> (String, &'static str) {
    if is_mismatch(record) {
        (
            record.claimed_model.clone().unwrap_or_default(),
            "self_reported",
        )
    } else {
        (
            "无法判定（分词器指纹将在后续更新补上）".to_string(),
            "undetermined",
        )
    }
}

fn build_report(date: String, sources: Vec<SourceInfo>, records: Vec<Record>) -> DailyAudit {
    let total = records.len();
    let errors = records.iter().filter(|r| is_error(r.status_code)).count();

    let mut routing = 0usize;
    let mut token_anomaly = 0usize;
    let mut invalid_model = 0usize;
    let mut degradation = 0usize;
    let mut anomalous_ids: std::collections::HashSet<&str> = std::collections::HashSet::new();
    let mut breakdown: BTreeMap<(String, String), (usize, &'static str)> = BTreeMap::new();
    let mut rows: Vec<ReportRow> = Vec::new();
    let (mut sum_in, mut sum_out) = (0i64, 0i64);

    for record in &records {
        sum_in += record.input_tokens.unwrap_or(0);
        sum_out += record.output_tokens.unwrap_or(0);

        let mut categories = Vec::new();
        if is_mismatch(record) {
            routing += 1;
            categories.push("routing_substitution".to_string());
        }
        if !record.issues.is_empty() {
            token_anomaly += 1;
            categories.push("token_anomaly".to_string());
        }
        if record
            .requested_model
            .as_deref()
            .map(is_invalid_model)
            .unwrap_or(false)
            && is_success(record.status_code)
        {
            invalid_model += 1;
            categories.push("invalid_model_accepted".to_string());
        }
        // 疑似降智：仅 rollout（有推理档位与推理 token），且只给线索
        if record.source == "codex"
            && matches!(
                record.reasoning_effort.as_deref(),
                Some("high") | Some("medium")
            )
            && record.reasoning_tokens == Some(0)
            && is_success(record.status_code)
        {
            degradation += 1;
            categories.push("degradation_suspect".to_string());
        }

        if categories.is_empty() {
            continue;
        }
        anomalous_ids.insert(record.id.as_str());
        let (actual, evidence) = actual_model(record);
        let key = (
            record
                .requested_model
                .clone()
                .unwrap_or_else(|| "未记录".to_string()),
            actual.clone(),
        );
        let entry = breakdown.entry(key).or_insert((0, evidence));
        entry.0 += 1;
        // 保留更强证据
        let rank = |e: &str| match e {
            "self_reported" => 1,
            _ => 0,
        };
        if rank(evidence) > rank(entry.1) {
            entry.1 = evidence;
        }

        if rows.len() < MAX_ROWS {
            rows.push(ReportRow {
                id: record.id.clone(),
                time: fmt_time(record.ts),
                source: record.source.to_string(),
                provider: record.provider.clone().unwrap_or_else(|| "—".to_string()),
                requested_model: record
                    .requested_model
                    .clone()
                    .unwrap_or_else(|| "未记录".to_string()),
                actual_model: actual,
                evidence_level: evidence.to_string(),
                input_tokens: record.input_tokens,
                output_tokens: record.output_tokens,
                reasoning_tokens: record.reasoning_tokens,
                status_code: record.status_code,
                categories,
            });
        }
    }

    rows.sort_by(|a, b| b.time.cmp(&a.time));

    let anomaly_total = anomalous_ids.len();
    let mut actual_model_breakdown: Vec<BreakdownRow> = breakdown
        .into_iter()
        .map(|((requested, actual), (count, evidence))| BreakdownRow {
            requested_model: requested,
            actual_model: actual,
            evidence_level: evidence.to_string(),
            count,
        })
        .collect();
    actual_model_breakdown.sort_by(|a, b| b.count.cmp(&a.count));

    let anomalies = vec![
        AnomalyCategory {
            key: "routing_substitution".into(),
            label: "路由 / 替换到其他模型".into(),
            count: routing,
        },
        AnomalyCategory {
            key: "token_anomaly".into(),
            label: "Token 用量矛盾".into(),
            count: token_anomaly,
        },
        AnomalyCategory {
            key: "invalid_model_accepted".into(),
            label: "接受了不存在 / 非法模型名".into(),
            count: invalid_model,
        },
        AnomalyCategory {
            key: "degradation_suspect".into(),
            label: "疑似模型降智（仅线索，非确诊）".into(),
            count: degradation,
        },
    ];

    DailyAudit {
        date,
        generated_at: Utc::now().to_rfc3339(),
        sources,
        total_requests: total,
        analyzed_requests: total,
        errors,
        anomaly_total,
        clean_requests: total.saturating_sub(anomaly_total),
        anomalies,
        actual_model_breakdown,
        tokens: Tokens {
            input: sum_in,
            output: sum_out,
            total: sum_in + sum_out,
        },
        rows,
        limitations: vec![
            "「实际模型」为中转在响应里的自报名（来自 response.completed），非经过认证的权重身份；自报名可被中转改写。".into(),
            "自报名不一致只覆盖中转诚实自报的情况；「连模型名都改干净」的同名偷换本页看不出，需后续的分词器指纹更新。".into(),
            "疑似降智仅为被动线索（请求高/中推理档位却无推理 Token），被动日志没有标准答案，不能确诊；确诊需主动配对核验。".into(),
            "CC Switch 全日记录与 Codex 会话日志两条源分别统计，同一次调用可能两边都有；未做无可靠关联 ID 的模糊去重。".into(),
        ],
    }
}

/// Tauri 命令：汇总某天（北京时间）的模型审计日报。
#[tauri::command]
pub(crate) fn audit_daily_report(date: Option<String>) -> Result<DailyAudit, String> {
    let date = date.unwrap_or_else(today_shanghai);
    let (start, end) = day_window(&date)?;

    let mut sources = Vec::new();
    let mut records = Vec::new();

    // CC Switch
    match ccswitch_db_path() {
        Some(path) => match read_ccswitch(&path, start, end) {
            Ok(mut rows) => {
                sources.push(SourceInfo {
                    id: "ccswitch".into(),
                    label: "CC Switch 全日记录".into(),
                    path: path.display().to_string(),
                    available: true,
                    records: rows.len(),
                });
                records.append(&mut rows);
            }
            Err(_error) => sources.push(SourceInfo {
                id: "ccswitch".into(),
                label: "CC Switch 全日记录".into(),
                path: path.display().to_string(),
                available: false,
                records: 0,
            }),
        },
        None => sources.push(SourceInfo {
            id: "ccswitch".into(),
            label: "CC Switch 全日记录".into(),
            path: "未找到 cc-switch.db".into(),
            available: false,
            records: 0,
        }),
    }

    // Codex rollout
    let codex_home = crate::codex_config::codex_home();
    let mut rollout = read_rollouts(&codex_home, start, end);
    sources.push(SourceInfo {
        id: "codex".into(),
        label: "Codex 会话日志".into(),
        path: codex_home.join("sessions").display().to_string(),
        available: true,
        records: rollout.len(),
    });
    records.append(&mut rollout);

    Ok(build_report(date, sources, records))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(source: &'static str, req: &str, claimed: Option<&str>, status: i64) -> Record {
        Record {
            id: format!("{source}:{req}:{status}"),
            source,
            ts: 1_000,
            requested_model: Some(req.to_string()),
            claimed_model: claimed.map(|s| s.to_string()),
            provider: Some("fr copy".into()),
            status_code: Some(status),
            input_tokens: Some(100),
            output_tokens: Some(10),
            cached: Some(0),
            cache_write: Some(0),
            reasoning_tokens: Some(5),
            reasoning_effort: Some("high".into()),
            issues: vec![],
        }
    }

    #[test]
    fn day_window_is_shanghai_midnight() {
        let (start, end) = day_window("2026-09-18").unwrap();
        assert_eq!(end - start, 86_400);
        // 2026-09-18 00:00 +08:00 == 2026-09-17 16:00 UTC
        assert_eq!(start % 86_400, (16 * 3600) % 86_400);
    }

    #[test]
    fn routing_substitution_and_actual_model() {
        let records = vec![
            rec("ccswitch", "gpt-6-astra", Some("gpt-5.6-luna"), 200),
            rec("ccswitch", "gpt-6-astra", Some("gpt-6-astra"), 200),
        ];
        let report = build_report("2026-09-18".into(), vec![], records);
        assert_eq!(report.anomaly_total, 1);
        assert_eq!(report.clean_requests, 1);
        let routing = report
            .anomalies
            .iter()
            .find(|c| c.key == "routing_substitution")
            .unwrap();
        assert_eq!(routing.count, 1);
        let top = &report.actual_model_breakdown[0];
        assert_eq!(top.actual_model, "gpt-5.6-luna");
        assert_eq!(top.evidence_level, "self_reported");
    }

    #[test]
    fn degradation_only_from_rollout() {
        let mut r = rec("codex", "gpt-6-astra", None, 200);
        r.reasoning_tokens = Some(0);
        let report = build_report("d".into(), vec![], vec![r]);
        let deg = report
            .anomalies
            .iter()
            .find(|c| c.key == "degradation_suspect")
            .unwrap();
        assert_eq!(deg.count, 1);
        assert_eq!(
            report.actual_model_breakdown[0].evidence_level,
            "undetermined"
        );
    }

    #[test]
    fn invalid_model_accepted_flagged() {
        let report = build_report(
            "d".into(),
            vec![],
            vec![rec("ccswitch", "audit-nonexistent-x", None, 200)],
        );
        let invalid = report
            .anomalies
            .iter()
            .find(|c| c.key == "invalid_model_accepted")
            .unwrap();
        assert_eq!(invalid.count, 1);
    }

    #[test]
    fn rollout_date_prefix_extracted() {
        let path = PathBuf::from("rollout-2026-09-18T13-07-51-uuid.jsonl");
        assert_eq!(rollout_date_prefix(&path).as_deref(), Some("2026-09-18"));
    }
}
