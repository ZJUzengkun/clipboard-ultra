mod clipboard;
mod commands;
mod db;
mod hotkey;
mod tray;

use commands::AppState;
use db::Database;
use std::sync::Arc;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // 初始化数据库
            let app_data_dir = app
                .path()
                .app_data_dir()
                .expect("Failed to get app data dir");
            let db = Arc::new(Database::new(app_data_dir).expect("Failed to init database"));

            // 启动剪贴板监听
            let monitor = Arc::new(clipboard::monitor::ClipboardMonitor::new(db.clone()));
            monitor.start();

            // 注册全局状态
            app.manage(AppState { db });

            // 注册全局快捷键
            let handle = app.handle().clone();
            hotkey::register_shortcuts(&handle)
                .map_err(|e| e.to_string())?;

            // 创建系统托盘
            tray::create_tray(&handle)
                .map_err(|e| e.to_string())?;

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_clipboard_items,
            commands::search_clipboard,
            commands::toggle_pin_item,
            commands::delete_clipboard_item,
            commands::paste_item,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
