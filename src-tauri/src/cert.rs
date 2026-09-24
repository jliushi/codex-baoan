//! 证书信任：把本机 CA 装入/移出「当前用户」信任库（certutil，隐藏窗口，无需管理员）。
use std::process::Command;

#[cfg(windows)]
fn hidden(cmd: &mut Command) -> &mut Command {
    use std::os::windows::process::CommandExt;
    cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW，避免弹黑框
    cmd
}
#[cfg(not(windows))]
fn hidden(cmd: &mut Command) -> &mut Command {
    cmd
}

/// CA 是否已在当前用户信任库（按 CN 匹配）。
pub fn ca_trusted() -> bool {
    let out = hidden(Command::new("certutil").args(["-user", "-store", "Root"])).output();
    match out {
        Ok(o) => String::from_utf8_lossy(&o.stdout).contains(crate::ca::CA_COMMON_NAME),
        Err(_) => false,
    }
}

/// 生成（若无）并信任 CA。
pub fn install_ca() -> Result<(), String> {
    crate::ca::ensure_ca()?;
    if ca_trusted() {
        return Ok(());
    }
    let cert = crate::paths::ca_cert_path();
    let out = hidden(Command::new("certutil").args(["-user", "-addstore", "Root"]).arg(&cert))
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).to_string());
    }
    Ok(())
}

/// 从信任库移除 CA（按 CN）。
pub fn uninstall_ca() -> Result<(), String> {
    let out = hidden(Command::new("certutil").args(["-user", "-delstore", "Root", crate::ca::CA_COMMON_NAME]))
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).to_string());
    }
    Ok(())
}
