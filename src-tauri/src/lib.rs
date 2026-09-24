mod model_audit;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_process::init())
        .invoke_handler(tauri::generate_handler![model_audit::audit_report])
        .run(tauri::generate_context!())
        .expect("运行 Tauri 应用出错");
}
