pub mod monitor;

/// 获取当前前台应用的 bundle ID（macOS）
/// 使用 lsappinfo 命令，无需 Automation 权限
#[cfg(target_os = "macos")]
pub fn get_frontmost_app_bundle_id() -> Option<String> {
    let front_output = std::process::Command::new("lsappinfo")
        .args(["front"])
        .output()
        .ok()?;
    let asn = String::from_utf8_lossy(&front_output.stdout).trim().to_string();
    if asn.is_empty() { return None; }

    let info_output = std::process::Command::new("lsappinfo")
        .args(["info", "-only", "bundleid", &asn])
        .output()
        .ok()?;
    let info = String::from_utf8_lossy(&info_output.stdout).trim().to_string();
    // 解析格式: "CFBundleIdentifier"="com.example.App"
    info.split('"').nth(3).map(|s| s.to_string())
}

/// 获取当前前台窗口的可执行文件名（Windows）
/// 返回小写的 exe 名称（如 "idea64.exe"），用作应用标识
#[cfg(target_os = "windows")]
pub fn get_frontmost_app_exe() -> Option<String> {
    use windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
    use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
    use windows_sys::Win32::System::ProcessStatus::GetModuleFileNameExW;
    use windows_sys::Win32::Foundation::CloseHandle;

    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_null() {
            return None;
        }
        let mut pid: u32 = 0;
        windows_sys::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId(hwnd, &mut pid);
        if pid == 0 {
            return None;
        }
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if process.is_null() {
            return None;
        }
        let mut buf = [0u16; 260];
        let len = GetModuleFileNameExW(process, std::ptr::null_mut(), buf.as_mut_ptr(), 260);
        CloseHandle(process);
        if len == 0 {
            return None;
        }
        let path = String::from_utf16_lossy(&buf[..len as usize]);
        // 取文件名部分，转小写
        path.rsplit('\\').next()
            .or_else(|| path.rsplit('/').next())
            .map(|s| s.to_lowercase())
    }
}

/// 获取当前前台窗口句柄（Windows），用于粘贴后恢复焦点
#[cfg(target_os = "windows")]
pub fn get_foreground_window_handle() -> usize {
    unsafe {
        windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow() as usize
    }
}

/// 恢复窗口焦点（Windows）
#[cfg(target_os = "windows")]
pub fn restore_foreground_window(hwnd: usize) {
    if hwnd == 0 { return; }
    unsafe {
        windows_sys::Win32::UI::WindowsAndMessaging::SetForegroundWindow(hwnd as _);
    }
}
