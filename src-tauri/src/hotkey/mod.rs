use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

/// 默认快捷键
pub const DEFAULT_SHORTCUT: &str = "CommandOrControl+Shift+V";

/// 将窗口定位到屏幕底部居中
fn position_at_bottom(window: &tauri::WebviewWindow) {
    if let Ok(monitor) = window.current_monitor() {
        if let Some(monitor) = monitor {
            let screen_size = monitor.size();
            let screen_pos = monitor.position();
            let scale = monitor.scale_factor();

            // 窗口实际像素尺寸
            if let Ok(win_size) = window.outer_size() {
                let screen_w = screen_size.width as f64 / scale;
                let screen_h = screen_size.height as f64 / scale;
                let win_w = win_size.width as f64 / scale;
                let win_h = win_size.height as f64 / scale;
                let offset_x = screen_pos.x as f64 / scale;
                let offset_y = screen_pos.y as f64 / scale;

                // 底部居中，留 40px 间距给 Dock/Taskbar
                let x = offset_x + (screen_w - win_w) / 2.0;
                let y = offset_y + screen_h - win_h - 40.0;

                let _ = window.set_position(tauri::LogicalPosition::new(x, y));
            }
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
