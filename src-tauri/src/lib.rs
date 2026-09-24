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
        "routed": ccswitch::routed_to(&url),
        "ccswitch_proxy": ccswitch::get_global_proxy().unwrap_or_default(),
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

    // 生成本机 CA；启动 MITM 代理。本工具不读写、不改动 CC Switch 的任何配置。
    let _ = ca::ensure_ca();
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
            fingerprint_report
        ])
        .run(tauri::generate_context!())
        .expect("运行 Tauri 应用出错");
}
