//! 模型审计：读 CC Switch 代理库 `proxy_request_logs`，汇总某天（北京时间）的
//! 总请求、异常（路由/替换、Token 矛盾、接受无效模型），以及异常请求的实际模型
//! （上游在 response.completed 里的自报名）。分词器指纹在 MITM 模块接入后补充。

use chrono::{FixedOffset, NaiveDate, TimeZone, Utc};
use rusqlite::{Connection, OpenFlags};
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::PathBuf;

const TZ_OFFSET_SECS: i32 = 8 * 3600; // Asia/Shanghai

fn shanghai() -> FixedOffset {
    FixedOffset::east_opt(TZ_OFFSET_SECS).expect("valid offset")
}

pub fn day_window(date: &str) -> Result<(i64, i64), String> {
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

pub fn today_shanghai() -> String {
    Utc::now().with_timezone(&shanghai()).format("%Y-%m-%d").to_string()
}

fn ccswitch_db_path() -> Option<PathBuf> {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let candidates = [
        dirs::data_dir().map(|p| p.join("cc-switch").join("cc-switch.db")),
        dirs::config_dir().map(|p| p.join("cc-switch").join("cc-switch.db")),
        Some(home.join(".cc-switch").join("cc-switch.db")),
    ];
    candidates.into_iter().flatten().find(|p| p.exists())
}

#[derive(Debug, Clone)]
struct Row {
    id: String,
    ts: i64,
    requested: Option<String>,
    reported: Option<String>, // 上游自报（model 列）
    provider: Option<String>,
    status: Option<i64>,
    input: i64,
    cache_read: i64,
    issues: Vec<String>,
}

fn is_error(s: Option<i64>) -> bool {
    matches!(s, Some(c) if c == 0 || c >= 400)
}
fn is_success(s: Option<i64>) -> bool {
    matches!(s, Some(c) if (200..300).contains(&c))
}
fn is_invalid_model(name: &str) -> bool {
    let l = name.to_ascii_lowercase();
    l.starts_with("audit-nonexistent") || l.starts_with("audit-invalid")
}
fn is_mismatch(r: &Row) -> bool {
    match (&r.requested, &r.reported) {
        (Some(a), Some(b)) => !b.is_empty() && a != b,
        _ => false,
    }
}

fn read_rows(start: i64, end: i64) -> Result<(Vec<Row>, BTreeMap<String, String>), String> {
    let path = ccswitch_db_path().ok_or("未找到 cc-switch.db")?;
    let conn = Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|e| e.to_string())?;
    let mut names = BTreeMap::new();
    if let Ok(mut stmt) = conn.prepare("SELECT id,name FROM providers WHERE app_type='codex'") {
        if let Ok(rows) = stmt.query_map([], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?))
        }) {
            for row in rows.flatten() {
                if let Some(n) = row.1 {
                    names.insert(row.0, n);
                }
            }
        }
    }
    let mut stmt = conn
        .prepare(
            "SELECT request_id,request_model,model,provider_id,input_tokens,cache_read_tokens,\
             cache_creation_tokens,input_token_semantics,status_code,created_at \
             FROM proxy_request_logs WHERE app_type='codex' AND created_at>=? AND created_at<? ORDER BY created_at",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([start, end], |r| {
            Ok((
                r.get::<_, Option<String>>(0)?,
                r.get::<_, Option<String>>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, Option<String>>(3)?,
                r.get::<_, Option<i64>>(4)?,
                r.get::<_, Option<i64>>(5)?,
                r.get::<_, Option<i64>>(6)?,
                r.get::<_, Option<i64>>(7)?,
                r.get::<_, Option<i64>>(8)?,
                r.get::<_, Option<i64>>(9)?,
            ))
        })
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for row in rows {
        let (id, req, model, pid, input, read, write, sem, status, created) =
            row.map_err(|e| e.to_string())?;
        let (read, write, sem, input) = (read.unwrap_or(0), write.unwrap_or(0), sem.unwrap_or(0), input.unwrap_or(0));
        let mut issues = Vec::new();
        // semantics: 1=total inclusive, 2=fresh, 0=legacy
        match sem {
            1 => {
                if read + write > input {
                    issues.push("cache_exceeds_input".into());
                }
            }
            0 => {
                if read > input {
                    issues.push("cache_exceeds_input".into());
                }
            }
            _ => {}
        }
        let provider = pid.as_ref().and_then(|p| names.get(p).cloned()).or(pid);
        out.push(Row {
            id: id.unwrap_or_else(|| format!("cc:{created:?}")),
            ts: created.unwrap_or(0),
            requested: req,
            reported: model,
            provider,
            status,
            input,
            cache_read: read,
            issues,
        });
    }
    Ok((out, names))
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
    pub evidence_level: String,
    pub count: usize,
}

#[derive(Serialize)]
pub struct ReportRow {
    pub id: String,
    pub time: String,
    pub provider: String,
    pub requested_model: String,
    pub actual_model: String,
    pub evidence_level: String,
    pub input_tokens: i64,
    pub cache_read: i64,
    pub status_code: Option<i64>,
    pub categories: Vec<String>,
}

#[derive(Serialize)]
pub struct Report {
    pub date: String,
    pub generated_at: String,
    pub source_available: bool,
    pub total_requests: usize,
    pub errors: usize,
    pub anomaly_total: usize,
    pub clean_requests: usize,
    pub anomalies: Vec<AnomalyCategory>,
    pub actual_model_breakdown: Vec<BreakdownRow>,
    pub rows: Vec<ReportRow>,
    pub limitations: Vec<String>,
}

fn fmt_time(ts: i64) -> String {
    shanghai()
        .timestamp_opt(ts, 0)
        .single()
        .map(|d| d.format("%H:%M:%S").to_string())
        .unwrap_or_default()
}

/// Tauri command: build the audit report for a day (defaults to today, Asia/Shanghai).
#[tauri::command]
pub fn audit_report(date: Option<String>) -> Result<Report, String> {
    let date = date.unwrap_or_else(today_shanghai);
    let (start, end) = day_window(&date)?;
    let (rows, _names) = match read_rows(start, end) {
        Ok(v) => v,
        Err(e) => {
            return Ok(Report {
                date,
                generated_at: Utc::now().to_rfc3339(),
                source_available: false,
                total_requests: 0,
                errors: 0,
                anomaly_total: 0,
                clean_requests: 0,
                anomalies: vec![],
                actual_model_breakdown: vec![],
                rows: vec![],
                limitations: vec![format!("未能读取 CC Switch 数据：{e}")],
            })
        }
    };

    let (mut routing, mut token_anomaly, mut invalid_model) = (0usize, 0usize, 0usize);
    let mut errors = 0usize;
    let mut anomalous: std::collections::HashSet<&str> = std::collections::HashSet::new();
    let mut breakdown: BTreeMap<(String, String), (usize, &'static str)> = BTreeMap::new();
    let mut out_rows: Vec<ReportRow> = Vec::new();

    for r in &rows {
        if is_error(r.status) {
            errors += 1;
        }
        let mut cats = Vec::new();
        if is_mismatch(r) {
            routing += 1;
            cats.push("routing_substitution".to_string());
        }
        if !r.issues.is_empty() {
            token_anomaly += 1;
            cats.push("token_anomaly".to_string());
        }
        if r.requested.as_deref().map(is_invalid_model).unwrap_or(false) && is_success(r.status) {
            invalid_model += 1;
            cats.push("invalid_model_accepted".to_string());
        }
        if cats.is_empty() {
            continue;
        }
        anomalous.insert(r.id.as_str());
        let (actual, evidence) = if is_mismatch(r) {
            (r.reported.clone().unwrap_or_default(), "self_reported")
        } else {
            ("无法判定（需 MITM 分词器指纹）".to_string(), "undetermined")
        };
        let key = (r.requested.clone().unwrap_or_else(|| "未记录".into()), actual.clone());
        let e = breakdown.entry(key).or_insert((0, evidence));
        e.0 += 1;
        if out_rows.len() < 1000 {
            out_rows.push(ReportRow {
                id: r.id.clone(),
                time: fmt_time(r.ts),
                provider: r.provider.clone().unwrap_or_else(|| "—".into()),
                requested_model: r.requested.clone().unwrap_or_else(|| "未记录".into()),
                actual_model: actual,
                evidence_level: evidence.into(),
                input_tokens: r.input,
                cache_read: r.cache_read,
                status_code: r.status,
                categories: cats,
            });
        }
    }
    out_rows.sort_by(|a, b| b.time.cmp(&a.time));

    let mut actual_model_breakdown: Vec<BreakdownRow> = breakdown
        .into_iter()
        .map(|((req, act), (count, ev))| BreakdownRow {
            requested_model: req,
            actual_model: act,
            evidence_level: ev.into(),
            count,
        })
        .collect();
    actual_model_breakdown.sort_by(|a, b| b.count.cmp(&a.count));

    let anomaly_total = anomalous.len();
    let total = rows.len();
    Ok(Report {
        date,
        generated_at: Utc::now().to_rfc3339(),
        source_available: true,
        total_requests: total,
        errors,
        anomaly_total,
        clean_requests: total.saturating_sub(anomaly_total),
        anomalies: vec![
            AnomalyCategory { key: "routing_substitution".into(), label: "路由 / 替换到其他模型".into(), count: routing },
            AnomalyCategory { key: "token_anomaly".into(), label: "Token 用量矛盾".into(), count: token_anomaly },
            AnomalyCategory { key: "invalid_model_accepted".into(), label: "接受了不存在 / 非法模型名".into(), count: invalid_model },
        ],
        actual_model_breakdown,
        rows: out_rows,
        limitations: vec![
            "「实际模型」为上游在 response.completed 里的自报名，非权重认证；自报名可被中转改写。".into(),
            "同名偷换（不改名字）本页看不出，需接入 MITM 分词器指纹（后续接入）。".into(),
            "仅覆盖经 CC Switch 记录的请求；数据留在本机，北京时间。".into(),
        ],
    })
}

