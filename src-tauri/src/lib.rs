mod ca;
mod capture;
mod cert;
mod ccswitch;
mod fingerprint;
mod model_audit;
mod paths;
mod proxy;
mod store;

use std::sync::Arc;
use tauri::Manager;

pub struct AppState {
    pub store: Arc<store::Store>,
    pub proxy_port: u16,
}

impl AppState {
    fn guard_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.proxy_port)
    }
}

#[tauri::command]
fn guard_status(state: tauri::State<AppState>) -> serde_json::Value {
    let url = state.guard_url();
    serde_json::json!({
        "cert_trusted": cert::ca_trusted(),
        "ccswitch_found": ccswitch::found(),
        "attached": ccswitch::is_attached(&url),
        "provider": ccswitch::current_provider_name(),
        "proxy_url": url,
    })
}

#[tauri::command]
fn install_cert() -> Result<bool, String> {
    cert::install_ca()?;
    Ok(cert::ca_trusted())
}

#[tauri::command]
fn uninstall_cert() -> Result<bool, String> {
    cert::uninstall_ca()?;
    Ok(cert::ca_trusted())
}

#[tauri::command]
fn attach_ccswitch(state: tauri::State<AppState>) -> Result<serde_json::Value, String> {
    ccswitch::attach(&state.guard_url())?;
    Ok(serde_json::json!({ "ok": true, "restart_required": true,
        "message": "已把 CC Switch 出口指向本工具，请重启一次 CC Switch 生效；关闭本工具会自动恢复。" }))
}

#[tauri::command]
fn detach_ccswitch(state: tauri::State<AppState>) -> Result<serde_json::Value, String> {
    ccswitch::detach(&state.guard_url())?;
    Ok(serde_json::json!({ "ok": true, "restart_required": true,
        "message": "已恢复 CC Switch 原出口，请重启一次 CC Switch 生效。" }))
}

#[tauri::command]
fn fingerprint_report(
    date: Option<String>,
    state: tauri::State<AppState>,
) -> Result<Vec<fingerprint::GroupVerdict>, String> {
    let d = date.unwrap_or_else(model_audit::today_shanghai);
    let (start, end) = model_audit::day_window(&d)?;
    let samples = state.store.snapshot(start, end);
    Ok(fingerprint::fingerprint_samples(&samples))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let proxy_port: u16 = 8899;
    let guard_url = format!("http://127.0.0.1:{proxy_port}");

    // 生成 CA；清理上次崩溃残留的接入；启动 MITM 代理。
    let _ = ca::ensure_ca();
    ccswitch::recover_stale(&guard_url);
    let store = Arc::new(store::Store::new());
    let _shutdown = proxy::spawn(proxy_port, store.clone());

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_process::init())
        .manage(AppState { store, proxy_port })
        .invoke_handler(tauri::generate_handler![
            model_audit::audit_report,
            guard_status,
            install_cert,
            uninstall_cert,
            attach_ccswitch,
            detach_ccswitch,
            fingerprint_report
        ])
        .on_window_event(move |window, event| {
            // 关闭窗口时自动恢复 CC Switch 原出口（临时接入自愈）。
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                let url = format!(
                    "http://127.0.0.1:{}",
                    window.state::<AppState>().proxy_port
                );
                let _ = ccswitch::detach(&url);
            }
        })
        .run(tauri::generate_context!())
        .expect("运行 Tauri 应用出错");
}
