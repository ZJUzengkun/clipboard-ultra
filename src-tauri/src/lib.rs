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
            let db = Arc::new(Database::new(app_data_dir.clone()).expect("Failed to init database"));

            // 启动剪贴板监听
            let monitor = Arc::new(clipboard::monitor::ClipboardMonitor::new(
                db.clone(),
                app_data_dir.clone(),
                app.handle().clone(),
            ));
            let skip_flag = monitor.skip_next.clone();
            monitor.start();

            // 读取保存的快捷键配置
            let saved_shortcut = db.get_config("shortcut").ok().flatten();

            // 注册全局状态
            let blobs_dir = app_data_dir.join("blobs");
            app.manage(AppState { db, blobs_dir, skip_clipboard_check: skip_flag });

            // 注册全局快捷键（使用已保存的或默认的）
            let handle = app.handle().clone();
            hotkey::register_shortcuts(&handle)
                .map_err(|e| e.to_string())?;
            // 如果有保存的快捷键，立刻用它替换默认的
            if let Some(ref shortcut_str) = saved_shortcut {
                if shortcut_str != hotkey::DEFAULT_SHORTCUT {
                    hotkey::re_register_shortcut(&handle, shortcut_str)
                        .map_err(|e| e.to_string())?;
                }
            }

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
            commands::get_blobs_dir,
            commands::get_shortcut,
            commands::set_shortcut,
            commands::get_tag_rules,
            commands::add_tag_rule,
            commands::delete_tag_rule,
            commands::get_items_by_tag,
            commands::set_item_tag,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
