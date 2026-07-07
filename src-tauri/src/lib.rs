mod clipboard;
mod commands;
mod db;
mod hotkey;
mod tray;

use commands::AppState;
use db::Database;
use std::sync::Arc;
use std::sync::RwLock;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            // macOS: 首次启动时提示用户授权辅助功能权限（仅未授权时弹窗）
            #[cfg(target_os = "macos")]
            {
                use core_foundation::base::TCFType;
                use core_foundation::boolean::CFBoolean;
                use core_foundation::dictionary::CFDictionary;
                use core_foundation::string::CFString;

                extern "C" {
                    fn AXIsProcessTrusted() -> bool;
                    fn AXIsProcessTrustedWithOptions(options: core_foundation::base::CFTypeRef) -> bool;
                }

                let trusted = unsafe { AXIsProcessTrusted() };
                if !trusted {
                    let key = CFString::new("AXTrustedCheckOptionPrompt");
                    let value = CFBoolean::true_value();
                    let options = CFDictionary::from_CFType_pairs(&[(key.as_CFType(), value.as_CFType())]);
                    unsafe {
                        AXIsProcessTrustedWithOptions(options.as_CFTypeRef());
                    }
                }
            }

            // 初始化数据库
            let app_data_dir = app
                .path()
                .app_data_dir()
                .map_err(|e| e.to_string())?;
            let db = Arc::new(
                Database::new(app_data_dir.clone())
                    .map_err(|e| format!("Failed to init database: {}", e))?
            );

            // 加载排除应用列表
            let excluded_apps_list: Vec<String> = db.get_config("excluded_apps")
                .ok()
                .flatten()
                .and_then(|json| serde_json::from_str(&json).ok())
                .unwrap_or_default();
            let excluded_apps = Arc::new(RwLock::new(excluded_apps_list));

            // 启动剩贴板监听
            let monitor = Arc::new(clipboard::monitor::ClipboardMonitor::new(
                db.clone(),
                app_data_dir.clone(),
                app.handle().clone(),
                excluded_apps.clone(),
            ));
            let skip_flag = monitor.skip_next.clone();
            monitor.start();
            
            // 启动后台过期清理线程（每小时执行一次）
            {
                let cleanup_db = db.clone();
                std::thread::spawn(move || {
                    loop {
                        // 启动时立即执行一次
                        if let Ok(deleted) = cleanup_db.cleanup_expired_items() {
                            if deleted > 0 {
                                eprintln!("[cleanup] Removed {} expired items", deleted);
                            }
                        }
                        std::thread::sleep(std::time::Duration::from_secs(3600));
                    }
                });
            }

            // 读取保存的快捷键配置
            let saved_shortcut = db.get_config("shortcut").ok().flatten();

            // 注册全局状态
            let blobs_dir = app_data_dir.join("blobs");
            app.manage(AppState { db, blobs_dir, skip_clipboard_check: skip_flag, previous_app: std::sync::Mutex::new(None), previous_window_hwnd: std::sync::Mutex::new(0), excluded_apps });

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
            commands::update_tag_rule_expire,
            commands::get_default_expire_days,
            commands::set_default_expire_days,
            commands::get_content_type_expire_days,
            commands::set_content_type_expire_days,
            commands::get_items_by_tag,
            commands::set_item_tag,
            commands::open_settings,
            commands::get_excluded_apps,
            commands::add_excluded_app,
            commands::remove_excluded_app,
            commands::get_running_apps,
            commands::get_excluded_apps_names,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
