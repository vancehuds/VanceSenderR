//! Startup port occupancy guard utilities.
//!
//! Checks if the configured port is available before starting the HTTP server.
//! If occupied, identifies the occupying process and optionally force-kills it.

use std::net::TcpListener;
use std::process::Command;

use crate::error::{AppError, AppResult};

/// Information about a process occupying a port.
#[derive(Debug, Clone)]
pub struct PortOccupier {
    pub pid: u32,
    pub process_name: Option<String>,
    pub local_address: Option<String>,
}

/// Return true if the target host:port can be bound right now.
pub fn is_port_bindable(host: &str, port: u16) -> bool {
    match TcpListener::bind((host, port)) {
        Ok(_listener) => true,
        Err(_) => false,
    }
}

/// Find the process occupying the given port (Windows only).
#[cfg(windows)]
fn find_port_occupier(port: u16) -> Option<PortOccupier> {
    // Use netstat to find the PID listening on this port
    let output = Command::new("netstat")
        .args(["-ano", "-p", "TCP"])
        .output()
        .ok()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let port_str = format!(":{port}");

    for line in stdout.lines() {
        let line = line.trim();
        if !line.contains("LISTENING") {
            continue;
        }

        // Parse: TCP    0.0.0.0:8730    0.0.0.0:0    LISTENING    12345
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 5 {
            continue;
        }

        let local_addr = parts[1];
        if !local_addr.ends_with(&port_str) {
            continue;
        }

        let pid: u32 = match parts[4].parse() {
            Ok(p) => p,
            Err(_) => continue,
        };

        // Look up process name via tasklist
        let process_name = lookup_process_name(pid);

        return Some(PortOccupier {
            pid,
            process_name,
            local_address: Some(local_addr.to_string()),
        });
    }

    None
}

#[cfg(not(windows))]
fn find_port_occupier(_port: u16) -> Option<PortOccupier> {
    None
}

/// Get process name for pid via tasklist on Windows.
#[cfg(windows)]
fn lookup_process_name(pid: u32) -> Option<String> {
    let output = Command::new("tasklist")
        .args(["/FI", &format!("PID eq {pid}"), "/FO", "CSV", "/NH"])
        .output()
        .ok()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout.lines().next()?.trim();

    if line.starts_with('"') {
        // CSV format: "process.exe","12345",...
        let name = line.split(',').next()?;
        let name = name.trim_matches('"');
        if !name.is_empty() && name != "INFO:" {
            return Some(name.to_string());
        }
    }

    None
}

/// Force terminate a Windows process tree by pid.
#[cfg(windows)]
fn force_kill_pid(pid: u32) -> AppResult<()> {
    let status = Command::new("taskkill")
        .args(["/F", "/T", "/PID", &pid.to_string()])
        .status()
        .map_err(|e| AppError::Internal(format!("无法执行 taskkill: {e}")))?;

    if status.success() {
        Ok(())
    } else {
        Err(AppError::Internal(format!(
            "taskkill 退出码: {}",
            status.code().unwrap_or(-1)
        )))
    }
}

#[cfg(not(windows))]
fn force_kill_pid(_pid: u32) -> AppResult<()> {
    Err(AppError::Internal("不支持在非 Windows 平台终止进程".into()))
}

/// Wait until host:port is bindable after process termination.
fn wait_for_port_release(host: &str, port: u16, timeout_seconds: f64) -> bool {
    let deadline = std::time::Instant::now()
        + std::time::Duration::from_secs_f64(timeout_seconds);

    while std::time::Instant::now() < deadline {
        if is_port_bindable(host, port) {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
    false
}

/// Ensure startup target port is available, optionally force-closing the occupier.
///
/// Returns Ok(()) if the port is available (or was freed), Err if we can't proceed.
pub fn ensure_startup_port_available(host: &str, port: u16) -> AppResult<()> {
    if is_port_bindable(host, port) {
        tracing::info!("端口 {host}:{port} 可用");
        return Ok(());
    }

    tracing::warn!("端口 {host}:{port} 已被占用");

    // Try to identify the occupier
    let occupier = find_port_occupier(port);

    if let Some(ref occ) = occupier {
        let proc_name = occ.process_name.as_deref().unwrap_or("未知进程");
        let addr = occ.local_address.as_deref().unwrap_or("?");
        tracing::warn!(
            "端口 {port} 被 PID {pid} ({proc_name}, {addr}) 占用",
            pid = occ.pid
        );

        // Check if the occupying process is another VanceSender instance
        let is_self = proc_name.to_lowercase().contains("vancesender")
            || proc_name.to_lowercase().contains("vance_sender");

        if is_self {
            tracing::info!("检测到另一个 VanceSender 实例 (PID {}), 尝试终止...", occ.pid);
            if let Err(e) = force_kill_pid(occ.pid) {
                tracing::error!("终止旧实例失败: {e}");
                return Err(AppError::Internal(format!(
                    "端口 {port} 被另一个 VanceSender (PID {}) 占用，且无法终止: {e}",
                    occ.pid
                )));
            }

            // Wait for port release
            if wait_for_port_release(host, port, 8.0) {
                tracing::info!("旧实例已终止，端口 {port} 已释放");
                return Ok(());
            } else {
                return Err(AppError::Internal(format!(
                    "已终止旧实例 (PID {}) 但端口 {port} 仍未释放",
                    occ.pid
                )));
            }
        }

        // For non-self processes, show Win32 dialog asking if we should kill
        #[cfg(windows)]
        {
            let message = format!(
                "VanceSender 需要的端口 {} 已被 {} (PID {}) 占用。\n\n是否强制关闭该进程？",
                port, proc_name, occ.pid
            );
            let result = show_windows_yes_no_dialog(&message);
            if result {
                tracing::info!("用户选择终止 PID {}", occ.pid);
                if let Err(e) = force_kill_pid(occ.pid) {
                    tracing::error!("终止进程失败: {e}");
                    return Err(AppError::Internal(format!(
                        "无法终止占用端口的进程 {} (PID {}): {e}",
                        proc_name, occ.pid
                    )));
                }
                if wait_for_port_release(host, port, 8.0) {
                    tracing::info!("进程已终止，端口 {port} 已释放");
                    Ok(())
                } else {
                    Err(AppError::Internal(format!(
                        "已终止进程但端口 {port} 仍未释放"
                    )))
                }
            } else {
                tracing::info!("用户取消终止进程");
                Err(AppError::Internal(format!(
                    "端口 {port} 被占用，用户取消终止"
                )))
            }
        }

        #[cfg(not(windows))]
        {
            return Err(AppError::Internal(format!(
                "端口 {port} 被 {proc_name} (PID {}) 占用",
                occ.pid
            )));
        }
    } else {
        // Could not identify occupier
        Err(AppError::Internal(format!(
            "端口 {host}:{port} 已被占用（无法识别占用进程）"
        )))
    }
}

/// Show a native Win32 Yes/No dialog.
#[cfg(windows)]
fn show_windows_yes_no_dialog(message: &str) -> bool {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;

    let wide_msg: Vec<u16> = OsStr::new(message)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let wide_title: Vec<u16> = OsStr::new("VanceSender")
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    // MB_YESNO | MB_ICONWARNING | MB_TOPMOST
    let style: u32 = 0x00000004 | 0x00000030 | 0x00040000;

    let result = unsafe {
        windows::Win32::UI::WindowsAndMessaging::MessageBoxW(
            None,
            windows::core::PCWSTR(wide_msg.as_ptr()),
            windows::core::PCWSTR(wide_title.as_ptr()),
            windows::Win32::UI::WindowsAndMessaging::MESSAGEBOX_STYLE(style),
        )
    };

    // IDYES = 6
    result.0 == 6
}
