use crate::db::{operations::{ClipboardItem, TagRule, Board}, Database};
use clipboard_rs::{common::RustImage, Clipboard, ClipboardContext};
use base64::{Engine, engine::general_purpose::STANDARD};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::sync::RwLock;
use tauri::{Manager, State, Emitter};
use serde::Serialize;

/// macOS: 用 CGEvent 直接发送 Cmd+V（只需 Accessibility 权限）
#[cfg(target_os = "macos")]
fn simulate_paste_macos() {
    use core_graphics::event::{CGEvent, CGEventFlags, CGEventTapLocation};
    use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};

    // 检查辅助功能权限
    extern "C" {
        fn AXIsProcessTrusted() -> bool;
    }
    let trusted = unsafe { AXIsProcessTrusted() };
    if !trusted {
        // Accessibility 未授权，fallback 到 System Events 按键
        let _ = std::process::Command::new("osascript")
            .args(["-e", "tell application \"System Events\" to keystroke \"v\" using command down"])
            .output();
        return;
    }

    let source = match CGEventSource::new(CGEventSourceStateID::HIDSystemState) {
        Ok(s) => s,
        Err(_) => return,
    };

    // 'v' key code = 9 on macOS
    let key_down = match CGEvent::new_keyboard_event(source.clone(), 9, true) {
        Ok(e) => e,
        Err(_) => return,
    };
    key_down.set_flags(CGEventFlags::CGEventFlagCommand);
    key_down.post(CGEventTapLocation::HID);

    let key_up = match CGEvent::new_keyboard_event(source, 9, false) {
        Ok(e) => e,
        Err(_) => return,
    };
    key_up.set_flags(CGEventFlags::CGEventFlagCommand);
    key_up.post(CGEventTapLocation::HID);
}

/// 应用全局状态，存储数据库引用和 blob 目录
pub struct AppState {
    pub db: Arc<Database>,
    pub blobs_dir: PathBuf,
    /// 粘贴时设置此标志，让 monitor 跳过下一次检测
    pub skip_clipboard_check: Arc<AtomicBool>,
    /// 记录面板显示前的前台应用 bundle ID（macOS）
    #[allow(dead_code)]
    pub previous_app: Mutex<Option<String>>,
    /// 记录面板显示前的前台窗口句柄（Windows）
    #[allow(dead_code)]
    pub previous_window_hwnd: Mutex<usize>,
    /// 排除应用列表（与 ClipboardMonitor 共享）
    pub excluded_apps: Arc<RwLock<Vec<String>>>,
}

/// 获取剪贴板历史列表（支持分页）
#[tauri::command]
pub fn get_clipboard_items(
    state: State<AppState>,
    limit: Option<u32>,
    offset: Option<u32>,
) -> Result<Vec<ClipboardItem>, String> {
    state.db.get_recent(limit.unwrap_or(50), offset.unwrap_or(0))
}

/// 获取历史条目总数
#[tauri::command]
pub fn count_items(state: State<AppState>) -> Result<i64, String> {
    state.db.count_items()
}

/// 获取收藏（置顶）条目列表
#[tauri::command]
pub fn get_pinned_items(
    state: State<AppState>,
    limit: Option<u32>,
) -> Result<Vec<ClipboardItem>, String> {
    state.db.get_pinned(limit.unwrap_or(200))
}

// ========== 收藏板（Boards）命令 ==========

#[tauri::command]
pub fn list_boards(state: State<AppState>) -> Result<Vec<Board>, String> {
    state.db.list_boards()
}

#[tauri::command]
pub fn create_board(state: State<AppState>, name: String, color: String) -> Result<Board, String> {
    state.db.create_board(&name, &color)
}

#[tauri::command]
pub fn rename_board(state: State<AppState>, id: i64, name: String) -> Result<(), String> {
    state.db.rename_board(id, &name)
}

#[tauri::command]
pub fn recolor_board(state: State<AppState>, id: i64, color: String) -> Result<(), String> {
    state.db.recolor_board(id, &color)
}

#[tauri::command]
pub fn delete_board(state: State<AppState>, id: i64) -> Result<(), String> {
    state.db.delete_board(id)
}

#[tauri::command]
pub fn reorder_boards(state: State<AppState>, ordered_ids: Vec<i64>) -> Result<(), String> {
    state.db.reorder_boards(&ordered_ids)
}

#[tauri::command]
pub fn add_item_to_board(state: State<AppState>, board_id: i64, item_id: i64) -> Result<(), String> {
    state.db.add_item_to_board(board_id, item_id)
}

#[tauri::command]
pub fn remove_item_from_board(state: State<AppState>, board_id: i64, item_id: i64) -> Result<(), String> {
    state.db.remove_item_from_board(board_id, item_id)
}

#[tauri::command]
pub fn get_board_ids_for_item(state: State<AppState>, item_id: i64) -> Result<Vec<i64>, String> {
    state.db.get_board_ids_for_item(item_id)
}

#[tauri::command]
pub fn get_items_in_board(state: State<AppState>, board_id: i64, limit: Option<u32>) -> Result<Vec<ClipboardItem>, String> {
    state.db.get_items_in_board(board_id, limit.unwrap_or(200))
}

/// 搜索剪贴板历史
#[tauri::command]
pub fn search_clipboard(
    state: State<AppState>,
    keyword: String,
) -> Result<Vec<ClipboardItem>, String> {
    state.db.search(&keyword)
}

/// 切换收藏/置顶
#[tauri::command]
pub fn toggle_pin_item(state: State<AppState>, id: i64) -> Result<bool, String> {
    state.db.toggle_pin(id)
}

/// 删除一条记录
#[tauri::command]
pub fn delete_clipboard_item(state: State<AppState>, id: i64) -> Result<(), String> {
    state.db.delete_item(id)
}

/// 粘贴指定条目：写入剪贴板 → 隐藏窗口 → 模拟 Ctrl+V/Cmd+V
#[tauri::command]
pub fn paste_item(
    state: State<AppState>,
    id: i64,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    // 获取目标条目
    let item = state
        .db
        .get_item_by_id(id)?
        .ok_or_else(|| "Item not found".to_string())?;

    // 刷新 updated_at，防止常用条目被过期清理
    state.db.touch_item(id)?;

    // 写入系统剪贴板
    // 设置跳过标志，防止 monitor 回环检测到自己写入的内容
    state.skip_clipboard_check.store(true, Ordering::SeqCst);
    let ctx = ClipboardContext::new().map_err(|e| e.to_string())?;

    if item.content_type == "image" {
        // 图片类型：从 blob 文件读取 PNG 并写入剪贴板
        if let Some(ref blob_name) = item.blob_path {
            let blob_file = state.blobs_dir.join(blob_name);
            if blob_file.exists() {
                let png_bytes = std::fs::read(&blob_file).map_err(|e| e.to_string())?;
                let rust_img = clipboard_rs::RustImageData::from_bytes(&png_bytes)
                    .map_err(|e| e.to_string())?;
                ctx.set_image(rust_img).map_err(|e| e.to_string())?;
            } else {
                return Err("Image blob file not found".to_string());
            }
        } else {
            return Err("Image item has no blob path".to_string());
        }
    } else {
        // 文本类型
        ctx.set_text(item.content.clone())
            .map_err(|e| e.to_string())?;
    }

    // 隐藏窗口
    if let Some(window) = app_handle.get_webview_window("main") {
        let _ = window.hide();
    }

    // 模拟粘贴快捷键
    #[cfg(target_os = "macos")]
    {
        // macOS: osascript 恢复焦点 + CGEvent 发送 Cmd+V
        // CGEvent 只需 Accessibility 权限，不需要主线程、不需要 Automation 权限
        let prev = state.previous_app.lock().unwrap().take();
        std::thread::spawn(move || {
            if let Some(bundle_id) = prev {
                // 用 osascript 激活前台应用（同步，返回时焦点已切换）
                let _ = std::process::Command::new("osascript")
                    .args(["-e", &format!(
                        "tell application id \"{}\" to activate", bundle_id
                    )])
                    .output();
                std::thread::sleep(std::time::Duration::from_millis(30));
            } else {
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            simulate_paste_macos();
        });
    }

    #[cfg(not(target_os = "macos"))]
    {
        // Windows/Linux: 恢复焦点 + enigo 模拟 Ctrl+V
        #[cfg(target_os = "windows")]
        let prev_hwnd = {
            *state.previous_window_hwnd.lock().unwrap()
        };
        let handle = app_handle.clone();
        std::thread::spawn(move || {
            // Windows: 先恢复焦点到之前的窗口
            #[cfg(target_os = "windows")]
            {
                crate::clipboard::restore_foreground_window(prev_hwnd);
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            #[cfg(not(target_os = "windows"))]
            {
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            let _ = handle.run_on_main_thread(move || {
                if let Ok(mut enigo) = enigo::Enigo::new(&enigo::Settings::default()) {
                    use enigo::{Direction, Key, Keyboard};
                    let _ = enigo.key(Key::Control, Direction::Press);
                    let _ = enigo.key(Key::Unicode('v'), Direction::Click);
                    let _ = enigo.key(Key::Control, Direction::Release);
                }
            });
        });
    }

    Ok(())
}

/// 获取 blobs 目录路径（前端用于构建图片 URL）
#[tauri::command]
pub fn get_blobs_dir(state: State<AppState>) -> Result<String, String> {
    Ok(state.blobs_dir.to_string_lossy().to_string())
}

/// 获取当前快捷键配置
#[tauri::command]
pub fn get_shortcut(state: State<AppState>) -> Result<String, String> {
    let shortcut = state
        .db
        .get_config("shortcut")?
        .unwrap_or_else(|| crate::hotkey::DEFAULT_SHORTCUT.to_string());
    Ok(shortcut)
}

/// 设置并应用新快捷键
#[tauri::command]
pub fn set_shortcut(
    state: State<AppState>,
    shortcut: String,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    // 先尝试注册新快捷键
    crate::hotkey::re_register_shortcut(&app_handle, &shortcut)?;
    // 注册成功后保存到数据库
    state.db.set_config("shortcut", &shortcut)?;
    Ok(())
}

// ========== 标签规则命令 ==========

/// 获取所有标签规则
#[tauri::command]
pub fn get_tag_rules(state: State<AppState>) -> Result<Vec<TagRule>, String> {
    state.db.get_tag_rules()
}

/// 添加标签规则
#[tauri::command]
pub fn add_tag_rule(
    state: State<AppState>,
    name: String,
    pattern: String,
    color: String,
    priority: i64,
    expire_days: Option<i64>,
) -> Result<TagRule, String> {
    state.db.add_tag_rule(&name, &pattern, &color, priority, expire_days.unwrap_or(0))
}

/// 更新标签规则的过期天数
#[tauri::command]
pub fn update_tag_rule_expire(
    state: State<AppState>,
    id: i64,
    expire_days: i64,
) -> Result<(), String> {
    state.db.update_tag_rule_expire(id, expire_days)
}

/// 获取全局默认过期天数
#[tauri::command]
pub fn get_default_expire_days(state: State<AppState>) -> Result<i64, String> {
    let val = state.db.get_config("default_expire_days")?
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    Ok(val)
}

/// 设置全局默认过期天数
#[tauri::command]
pub fn set_default_expire_days(
    state: State<AppState>,
    days: i64,
) -> Result<(), String> {
    state.db.set_config("default_expire_days", &days.to_string())
}

/// 获取最大保存数量（默认 1000，-1 表示不限制）
#[tauri::command]
pub fn get_max_items(state: State<AppState>) -> Result<i64, String> {
    let val = state.db.get_config("max_items")?
        .and_then(|v| v.parse().ok())
        .unwrap_or(1000);
    Ok(val)
}

/// 设置最大保存数量（-1 表示不限制）
#[tauri::command]
pub fn set_max_items(
    state: State<AppState>,
    count: i64,
) -> Result<(), String> {
    state.db.set_config("max_items", &count.to_string())
}

/// 获取指定内容类型的过期天数
#[tauri::command]
pub fn get_content_type_expire_days(
    state: State<AppState>,
    content_type: String,
) -> Result<i64, String> {
    let key = format!("expire_days_{}", content_type);
    let val = state.db.get_config(&key)?
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    Ok(val)
}

/// 设置指定内容类型的过期天数
#[tauri::command]
pub fn set_content_type_expire_days(
    state: State<AppState>,
    content_type: String,
    days: i64,
) -> Result<(), String> {
    let key = format!("expire_days_{}", content_type);
    state.db.set_config(&key, &days.to_string())
}

/// 删除标签规则
#[tauri::command]
pub fn delete_tag_rule(state: State<AppState>, id: i64) -> Result<(), String> {
    state.db.delete_tag_rule(id)
}

/// 按标签筛选条目
#[tauri::command]
pub fn get_items_by_tag(
    state: State<AppState>,
    tag: String,
    limit: Option<u32>,
) -> Result<Vec<ClipboardItem>, String> {
    state.db.get_by_tag(&tag, limit.unwrap_or(50))
}

/// 手动给条目打标签
#[tauri::command]
pub fn set_item_tag(
    state: State<AppState>,
    id: i64,
    tag: String,
) -> Result<(), String> {
    let tag_opt = if tag.is_empty() { None } else { Some(tag.as_str()) };
    state.db.set_item_tag(id, tag_opt)
}

/// 更新条目文本内容（仅文本类），编辑后标签/板归属保留
#[tauri::command]
pub fn update_item_content(state: State<AppState>, id: i64, content: String) -> Result<(), String> {
    state.db.update_item_content(id, &content)
}

/// 导出全部数据到指定 JSON 文件（图片以 base64 内嵌，自包含备份），返回导出条目数
#[tauri::command]
pub fn export_data(state: State<AppState>, path: String) -> Result<usize, String> {
    let mut items = state.db.export_items()?;
    // 图片条目：读取原图并 base64 内嵌
    for it in items.iter_mut() {
        if it.content_type == "image" {
            if let Some(ref blob) = it.blob_path {
                let full = state.blobs_dir.join(blob);
                if let Ok(bytes) = std::fs::read(&full) {
                    it.image_base64 = Some(STANDARD.encode(bytes));
                }
            }
        }
    }
    let count = items.len();
    let data = crate::db::operations::ExportData {
        version: 1,
        exported_at: chrono::Utc::now().timestamp(),
        items,
        tag_rules: state.db.export_tag_rules()?,
        boards: state.db.export_boards()?,
    };
    let json = serde_json::to_string_pretty(&data).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())?;
    Ok(count)
}

/// 从指定 JSON 文件导入数据（合并，去重跳过），返回新增条目数
#[tauri::command]
pub fn import_data(app: tauri::AppHandle, state: State<AppState>, path: String) -> Result<usize, String> {
    use image::GenericImageView;
    let json = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let mut data: crate::db::operations::ExportData =
        serde_json::from_str(&json).map_err(|_| "文件格式无法识别".to_string())?;
    std::fs::create_dir_all(&state.blobs_dir).ok();
    // 图片条目：解码 base64 写回 blob + 生成缩略图
    for it in data.items.iter_mut() {
        if it.content_type == "image" {
            if let Some(ref b64) = it.image_base64 {
                if let Ok(bytes) = STANDARD.decode(b64) {
                    let file_id = uuid::Uuid::new_v4().to_string();
                    let original = state.blobs_dir.join(format!("{}.png", file_id));
                    if std::fs::write(&original, &bytes).is_ok() {
                        let thumb = state.blobs_dir.join(format!("{}_thumb.png", file_id));
                        if let Ok(img) = image::load_from_memory(&bytes) {
                            let (w, h) = img.dimensions();
                            if w > 200 {
                                let new_h = (200 * h) / w;
                                let _ = img.thumbnail(200, new_h).save(&thumb);
                            } else {
                                let _ = std::fs::copy(&original, &thumb);
                            }
                        }
                        it.blob_path = Some(format!("{}.png", file_id));
                    }
                }
            }
        }
    }
    let imported = state.db.import_all(&data.items, &data.tag_rules, &data.boards)?;
    // 通知主窗口刷新列表
    let _ = app.emit("clipboard-updated", ());
    Ok(imported)
}

/// 打开独立的设置窗口（使用 tauri.conf.json 中预定义的 settings 窗口）
#[tauri::command]
pub fn open_settings(app_handle: tauri::AppHandle) -> Result<(), String> {
    // 获取预定义的 settings 窗口并显示
    if let Some(window) = app_handle.get_webview_window("settings") {
        let _ = window.show();
        let _ = window.set_focus();
        Ok(())
    } else {
        Err("Settings window not found".to_string())
    }
}

// ========== 权限自检与通用配置 ==========

/// 检测辅助功能权限（macOS 自动粘贴依赖；其他平台无此概念，恒为已授权）
#[tauri::command]
pub fn check_accessibility() -> bool {
    #[cfg(target_os = "macos")]
    {
        extern "C" {
            fn AXIsProcessTrusted() -> bool;
        }
        unsafe { AXIsProcessTrusted() }
    }
    #[cfg(not(target_os = "macos"))]
    {
        true
    }
}

/// 跳转系统设置的辅助功能授权页（仅 macOS）
#[tauri::command]
pub fn open_accessibility_settings() {
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open")
            .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
            .spawn();
    }
}

/// 通用 KV 配置读取（app_config 表），供前端轻量开关使用
#[tauri::command]
pub fn get_app_config(state: State<AppState>, key: String) -> Result<Option<String>, String> {
    state.db.get_config(&key)
}

/// 通用 KV 配置写入（app_config 表）
#[tauri::command]
pub fn set_app_config(state: State<AppState>, key: String, value: String) -> Result<(), String> {
    state.db.set_config(&key, &value)
}

// ========== 排除应用命令 ==========

/// 运行中的应用信息
#[derive(Debug, Serialize, Clone)]
pub struct RunningApp {
    pub bundle_id: String,
    pub name: String,
}

/// 获取排除应用列表
#[tauri::command]
pub fn get_excluded_apps(state: State<AppState>) -> Result<Vec<String>, String> {
    let list = state.excluded_apps.read().map_err(|e| e.to_string())?;
    Ok(list.clone())
}

/// 添加排除应用
#[tauri::command]
pub fn add_excluded_app(
    state: State<AppState>,
    bundle_id: String,
    app_name: String,
) -> Result<(), String> {
    // 更新内存中的列表
    {
        let mut list = state.excluded_apps.write().map_err(|e| e.to_string())?;
        if !list.contains(&bundle_id) {
            list.push(bundle_id.clone());
        }
    }
    // 持久化到数据库
    let list = state.excluded_apps.read().map_err(|e| e.to_string())?;
    let json = serde_json::to_string(&*list).map_err(|e| e.to_string())?;
    state.db.set_config("excluded_apps", &json)?;
    // 同时保存应用名称映射（用于前端展示）
    let names_json = state.db.get_config("excluded_apps_names")?
        .unwrap_or_else(|| "{}".to_string());
    let mut names: serde_json::Map<String, serde_json::Value> = serde_json::from_str(&names_json).unwrap_or_default();
    names.insert(bundle_id, serde_json::Value::String(app_name));
    let names_str = serde_json::to_string(&names).map_err(|e| e.to_string())?;
    state.db.set_config("excluded_apps_names", &names_str)?;
    Ok(())
}

/// 移除排除应用
#[tauri::command]
pub fn remove_excluded_app(
    state: State<AppState>,
    bundle_id: String,
) -> Result<(), String> {
    {
        let mut list = state.excluded_apps.write().map_err(|e| e.to_string())?;
        list.retain(|id| id != &bundle_id);
    }
    let list = state.excluded_apps.read().map_err(|e| e.to_string())?;
    let json = serde_json::to_string(&*list).map_err(|e| e.to_string())?;
    state.db.set_config("excluded_apps", &json)?;
    // 也从名称映射中移除
    let names_json = state.db.get_config("excluded_apps_names")?
        .unwrap_or_else(|| "{}".to_string());
    let mut names: serde_json::Map<String, serde_json::Value> = serde_json::from_str(&names_json).unwrap_or_default();
    names.remove(&bundle_id);
    let names_str = serde_json::to_string(&names).map_err(|e| e.to_string())?;
    state.db.set_config("excluded_apps_names", &names_str)?;
    Ok(())
}

/// 获取当前正在运行的 GUI 应用列表（macOS）
/// 仅返回 Foreground 类型的用户应用，排除系统守护进程、WebKit 子进程和自身
#[tauri::command]
pub fn get_running_apps() -> Result<Vec<RunningApp>, String> {
    #[cfg(target_os = "macos")]
    {
        let output = std::process::Command::new("lsappinfo")
            .args(["list"])
            .output()
            .map_err(|e| e.to_string())?;
        let text = String::from_utf8_lossy(&output.stdout);
        let mut apps: Vec<RunningApp> = Vec::new();
        let mut current_name: Option<String> = None;
        let mut current_bundle: Option<String> = None;
        let mut current_is_foreground = false;

        for line in text.lines() {
            let trimmed = line.trim();
            // 新应用条目: 以数字开头，包含应用名
            if trimmed.starts_with(|c: char| c.is_ascii_digit()) && trimmed.contains('"') {
                // 保存上一个（仅 Foreground 类型）
                if current_is_foreground {
                    if let (Some(name), Some(bundle)) = (current_name.take(), current_bundle.take()) {
                        if !bundle.is_empty() && !name.is_empty() {
                            apps.push(RunningApp { bundle_id: bundle, name });
                        }
                    }
                }
                current_name = None;
                current_bundle = None;
                current_is_foreground = false;
                // 提取名称: 形如 0) "Finder" ASN:...
                if let Some(start) = trimmed.find('"') {
                    if let Some(end) = trimmed[start+1..].find('"') {
                        current_name = Some(trimmed[start+1..start+1+end].to_string());
                    }
                }
            }
            // 检测应用类型：仅保留 Foreground 类型
            if trimmed.contains("type=\"Foreground\"") {
                current_is_foreground = true;
            }
            // 解析 bundle ID
            if trimmed.starts_with("\"CFBundleIdentifier\"=") || trimmed.starts_with("bundleID=") {
                if let Some(val) = trimmed.split('"').nth(3).or_else(|| trimmed.split('=').last()) {
                    let clean = val.trim_matches('"').to_string();
                    if !clean.is_empty() && clean != "[ NULL ]" {
                        current_bundle = Some(clean);
                    }
                }
            }
        }
        // 最后一个
        if current_is_foreground {
            if let (Some(name), Some(bundle)) = (current_name, current_bundle) {
                if !bundle.is_empty() && !name.is_empty() {
                    apps.push(RunningApp { bundle_id: bundle, name });
                }
            }
        }

        // 过滤掉自身应用
        apps.retain(|app| {
            !app.bundle_id.contains("clipboard-ultra")
                && !app.bundle_id.contains("clipboard-pro")
        });

        // 去重并排序
        apps.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        apps.dedup_by(|a, b| a.bundle_id == b.bundle_id);
        Ok(apps)
    }

    #[cfg(not(target_os = "macos"))]
    {
        #[cfg(target_os = "windows")]
        {
            use windows_sys::Win32::UI::WindowsAndMessaging::EnumWindows;

            let mut apps: Vec<RunningApp> = Vec::new();

            unsafe extern "system" fn enum_callback(
                hwnd: windows_sys::Win32::Foundation::HWND,
                lparam: isize,
            ) -> windows_sys::Win32::Foundation::BOOL {
                use windows_sys::Win32::UI::WindowsAndMessaging::{
                    GetWindowTextW, GetWindowTextLengthW, IsWindowVisible,
                    GetWindowThreadProcessId,
                };
                use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
                use windows_sys::Win32::System::ProcessStatus::GetModuleFileNameExW;
                use windows_sys::Win32::Foundation::CloseHandle;

                if IsWindowVisible(hwnd) == 0 {
                    return 1; // 继续枚举
                }
                let title_len = GetWindowTextLengthW(hwnd);
                if title_len == 0 {
                    return 1;
                }
                // 获取窗口标题
                let mut title_buf = vec![0u16; (title_len + 1) as usize];
                GetWindowTextW(hwnd, title_buf.as_mut_ptr(), title_buf.len() as i32);
                let title = String::from_utf16_lossy(&title_buf[..title_len as usize]);

                // 获取进程 exe
                let mut pid: u32 = 0;
                GetWindowThreadProcessId(hwnd, &mut pid);
                if pid == 0 {
                    return 1;
                }
                let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
                if process.is_null() {
                    return 1;
                }
                let mut buf = [0u16; 260];
                let len = GetModuleFileNameExW(process, std::ptr::null_mut(), buf.as_mut_ptr(), 260);
                CloseHandle(process);
                if len == 0 {
                    return 1;
                }
                let path = String::from_utf16_lossy(&buf[..len as usize]);
                let exe_name = path.rsplit('\\').next()
                    .or_else(|| path.rsplit('/').next())
                    .unwrap_or(&path)
                    .to_lowercase();

                let apps_vec = &mut *(lparam as *mut Vec<(String, String)>);
                apps_vec.push((exe_name, title));
                1 // 继续枚举
            }

            let mut raw_apps: Vec<(String, String)> = Vec::new();
            unsafe {
                EnumWindows(Some(enum_callback), &mut raw_apps as *mut _ as isize);
            }

            // 去重：同一 exe 只保留一个
            let mut seen = std::collections::HashSet::new();
            for (exe, title) in raw_apps {
                if exe.contains("clipboard-ultra") || exe.contains("clipboard-pro") {
                    continue;
                }
                if seen.insert(exe.clone()) {
                    apps.push(RunningApp {
                        bundle_id: exe.clone(),
                        name: title,
                    });
                }
            }
            apps.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
            Ok(apps)
        }

        #[cfg(not(target_os = "windows"))]
        {
            Ok(Vec::new())
        }
    }
}

/// 获取排除应用的名称映射
#[tauri::command]
pub fn get_excluded_apps_names(state: State<AppState>) -> Result<std::collections::HashMap<String, String>, String> {
    let names_json = state.db.get_config("excluded_apps_names")?
        .unwrap_or_else(|| "{}".to_string());
    let names: std::collections::HashMap<String, String> = serde_json::from_str(&names_json).unwrap_or_default();
    Ok(names)
}
