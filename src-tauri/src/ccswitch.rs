//! CC Switch 出口代理的读写与「临时接入」生命周期（对标原 Go guard 的自愈接入）。
use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

fn db_path() -> Option<PathBuf> {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let candidates = [
        dirs::data_dir().map(|p| p.join("cc-switch").join("cc-switch.db")),
        dirs::config_dir().map(|p| p.join("cc-switch").join("cc-switch.db")),
        Some(home.join(".cc-switch").join("cc-switch.db")),
    ];
    candidates.into_iter().flatten().find(|p| p.exists())
}

pub fn found() -> bool {
    db_path().is_some()
}

/// 读 CC Switch 的全局出口代理（settings.global_proxy_url），无则空串。
pub fn get_global_proxy() -> Result<String, String> {
    let path = db_path().ok_or("未找到 cc-switch.db")?;
    let conn = Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(|e| e.to_string())?;
    let v: Result<String, _> =
        conn.query_row("SELECT value FROM settings WHERE key='global_proxy_url'", [], |r| r.get(0));
    match v {
        Ok(s) => Ok(s),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(String::new()),
        Err(e) => Err(e.to_string()),
    }
}

/// 写 CC Switch 的全局出口代理；空串则清除（直连）。CC Switch 下次启动读取生效。
pub fn set_global_proxy(url: &str) -> Result<(), String> {
    let path = db_path().ok_or("未找到 cc-switch.db")?;
    let conn = Connection::open(&path).map_err(|e| e.to_string())?;
    if url.is_empty() {
        conn.execute("DELETE FROM settings WHERE key='global_proxy_url'", []).map_err(|e| e.to_string())?;
    } else {
        conn.execute(
            "INSERT INTO settings(key,value) VALUES('global_proxy_url',?1) ON CONFLICT(key) DO UPDATE SET value=?1",
            [url],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn current_provider_name() -> String {
    if let Some(path) = db_path() {
        if let Ok(conn) = Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_ONLY) {
            if let Ok(n) = conn.query_row(
                "SELECT name FROM providers WHERE app_type='codex' AND is_current=1",
                [],
                |r| r.get::<_, String>(0),
            ) {
                return n;
            }
        }
    }
    String::new()
}

// --- 临时接入生命周期：记录原出口 + PID，关闭自动恢复，崩溃后下次启动清理 ---

#[derive(Serialize, Deserialize, Default)]
pub struct Attachment {
    pub original_proxy: String, // 接入前 CC Switch 的出口（可能是 Clash 7890 或空）
    pub guard_url: String,      // 我们指过去的地址
    pub owner_pid: u32,         // 建立接入的进程
}

fn attach_path() -> PathBuf {
    crate::paths::data_dir().join("attachment.json")
}

pub fn load_attachment() -> Option<Attachment> {
    std::fs::read_to_string(attach_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
}

fn save_attachment(a: &Attachment) {
    if let Ok(s) = serde_json::to_string_pretty(a) {
        let _ = std::fs::write(attach_path(), s);
    }
}

fn clear_attachment() {
    let _ = std::fs::remove_file(attach_path());
}

/// 是否已接入到本 guard。
pub fn is_attached(guard_url: &str) -> bool {
    get_global_proxy().map(|p| p == guard_url).unwrap_or(false)
}

/// 接入：记录原出口，把 CC Switch 出口指向 guard。
pub fn attach(guard_url: &str) -> Result<(), String> {
    let prev = get_global_proxy().unwrap_or_default();
    if prev != guard_url {
        save_attachment(&Attachment {
            original_proxy: prev,
            guard_url: guard_url.to_string(),
            owner_pid: std::process::id(),
        });
    }
    set_global_proxy(guard_url)
}

/// 断开：恢复原出口，清记录。若用户手动改成了别的（非 guard），不覆盖。
pub fn detach(guard_url: &str) -> Result<(), String> {
    if let Some(a) = load_attachment() {
        let current = get_global_proxy().unwrap_or_default();
        if current == guard_url {
            set_global_proxy(&a.original_proxy)?;
        }
        clear_attachment();
    }
    Ok(())
}

/// 启动时清理崩溃残留：若记录仍指向 guard 且建立进程已不在，则恢复原出口。
pub fn recover_stale(guard_url: &str) {
    if let Some(a) = load_attachment() {
        let current = get_global_proxy().unwrap_or_default();
        if current == guard_url {
            let _ = set_global_proxy(&a.original_proxy);
        }
        clear_attachment();
    }
}
