use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

/// 默认快捷键
pub const DEFAULT_SHORTCUT: &str = "CommandOrControl+Shift+V";

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
                            // 先定位到屏幕中央，再显示并聚焦
                            let _ = window.center();
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
