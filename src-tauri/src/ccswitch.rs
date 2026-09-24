//! 只读 CC Switch：读它的出口代理与当前供应商用于状态显示。
//! 明确不写 CC Switch 的任何数据——不改动 CC Switch。
use rusqlite::{Connection, OpenFlags};
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

/// 只读 CC Switch 的全局出口代理（settings.global_proxy_url），无则空串。
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

/// CC Switch 当前出口是否已指向本 guard（只读判断；由用户自行在其代理层设置，本工具不写 CC Switch）。
pub fn routed_to(guard_url: &str) -> bool {
    get_global_proxy().map(|p| p == guard_url).unwrap_or(false)
}
