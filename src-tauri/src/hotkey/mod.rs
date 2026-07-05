use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

/// 默认快捷键
pub const DEFAULT_SHORTCUT: &str = "CommandOrControl+Shift+V";

/// 获取当前前台应用的 bundle ID（macOS）
/// 使用 lsappinfo 命令，无需 Automation 权限
#[cfg(target_os = "macos")]
fn get_frontmost_app_bundle_id() -> Option<String> {
    crate::clipboard::get_frontmost_app_bundle_id()
}

/// 将窗口定位到屏幕底部，宽度铺满屏幕
fn position_at_bottom(window: &tauri::WebviewWindow) {
    if let Ok(monitor) = window.current_monitor() {
        if let Some(monitor) = monitor {
            let screen_size = monitor.size();
            let screen_pos = monitor.position();
            let scale = monitor.scale_factor();

            let screen_w = screen_size.width as f64 / scale;
            let screen_h = screen_size.height as f64 / scale;
            let offset_x = screen_pos.x as f64 / scale;
            let offset_y = screen_pos.y as f64 / scale;

            let panel_height = 360.0;
            let dock_margin = 5.0;

            // 设置窗口大小为屏幕宽度 × 面板高度
            let _ = window.set_size(tauri::LogicalSize::new(screen_w, panel_height));

            // 定位到屏幕最底部（留一点间距给 Dock）
            let x = offset_x;
            let y = offset_y + screen_h - panel_height - dock_margin;

            let _ = window.set_position(tauri::LogicalPosition::new(x, y));
        }
    }
}

/// 注册全局快捷键来 toggle 窗口
pub fn register_shortcuts(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let shortcut: Shortcut = DEFAULT_SHORTCUT.parse()?;

    app.plugin(
        tauri_plugin_global_shortcut::Builder::new()
            .with_handler(move |app, _shortcut, event| {
                if event.state() == ShortcutState::Pressed {
                    if let Some(window) = app.get_webview_window("main") {
                        if window.is_visible().unwrap_or(false) {
                            let _ = window.hide();
                        } else {
                            // macOS: 记录当前前台应用，粘贴时恢复焦点
                            #[cfg(target_os = "macos")]
                            {
                                if let Some(id) = get_frontmost_app_bundle_id() {
                                    let state = app.state::<crate::commands::AppState>();
                                    *state.previous_app.lock().unwrap() = Some(id);
                                }
                            }
                            // Windows: 记录当前前台窗口句柄，粘贴时恢复焦点
                            #[cfg(target_os = "windows")]
                            {
                                let hwnd = crate::clipboard::get_foreground_window_handle();
                                let state = app.state::<crate::commands::AppState>();
                                *state.previous_window_hwnd.lock().unwrap() = hwnd;
                            }
                            position_at_bottom(&window);
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                }
            })
            .build(),
    )?;

    app.global_shortcut().register(shortcut)?;
    Ok(())
}

/// 重新注册快捷键（先取消所有，再注册新的）
pub fn re_register_shortcut(app: &AppHandle, shortcut_str: &str) -> Result<(), String> {
    let shortcut: Shortcut = shortcut_str.parse().map_err(|e| format!("{:?}", e))?;

    // 取消所有已注册快捷键
    app.global_shortcut()
        .unregister_all()
        .map_err(|e| e.to_string())?;

    // 注册新快捷键
    app.global_shortcut()
        .register(shortcut)
        .map_err(|e| e.to_string())?;

    Ok(())
}
