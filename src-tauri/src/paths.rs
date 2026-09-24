//! 本机数据目录与 CA 文件路径。
use std::path::PathBuf;

pub fn data_dir() -> PathBuf {
    let base = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    let dir = base.join("codex-baoan");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

pub fn ca_cert_path() -> PathBuf {
    data_dir().join("codex-baoan-ca.crt")
}

pub fn ca_key_path() -> PathBuf {
    data_dir().join("codex-baoan-ca.key")
}
