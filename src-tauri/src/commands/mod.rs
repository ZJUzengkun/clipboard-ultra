use crate::db::{operations::{ClipboardItem, TagRule}, Database};
use clipboard_rs::{common::RustImage, Clipboard, ClipboardContext};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use tauri::{Manager, State, WebviewUrl, WebviewWindowBuilder};

/// 应用全局状态，存储数据库引用和 blob 目录
pub struct AppState {
    pub db: Arc<Database>,
    pub blobs_dir: PathBuf,
    /// 粘贴时设置此标志，让 monitor 跳过下一次检测
    pub skip_clipboard_check: Arc<AtomicBool>,
    /// 记录面板显示前的前台应用 bundle ID（macOS）
    #[allow(dead_code)]
    pub previous_app: Mutex<Option<String>>,
}

/// 获取剪贴板历史列表
#[tauri::command]
pub fn get_clipboard_items(
    state: State<AppState>,
    limit: Option<u32>,
) -> Result<Vec<ClipboardItem>, String> {
    state.db.get_recent(limit.unwrap_or(50))
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
        // macOS: osascript 恢复焦点 + enigo 发送 Cmd+V
        // System Events keystroke 需要 Automation 权限（用户可能未授予），
        // 而 enigo 走 Accessibility 权限（已确认开启），所以分开处理更可靠
        let prev = state.previous_app.lock().unwrap().take();
        let handle = app_handle.clone();
        std::thread::spawn(move || {
            // 1. 用 osascript 激活前台应用
            if let Some(bundle_id) = prev {
                let _ = std::process::Command::new("osascript")
                    .args(["-e", &format!(
                        "tell application id \"{}\" to activate", bundle_id
                    )])
                    .output();
            }
            // 2. 等待焦点稳定后用 enigo 发送 Cmd+V
            std::thread::sleep(std::time::Duration::from_millis(100));
            let _ = handle.run_on_main_thread(move || {
                if let Ok(mut enigo) = enigo::Enigo::new(&enigo::Settings::default()) {
                    use enigo::{Direction, Key, Keyboard};
                    let _ = enigo.key(Key::Meta, Direction::Press);
                    let _ = enigo.key(Key::Unicode('v'), Direction::Click);
                    let _ = enigo.key(Key::Meta, Direction::Release);
                }
            });
        });
    }

    #[cfg(not(target_os = "macos"))]
    {
        // Windows/Linux: 用 enigo 模拟 Ctrl+V
        let handle = app_handle.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(100));
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
) -> Result<TagRule, String> {
    state.db.add_tag_rule(&name, &pattern, &color, priority)
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

/// 打开独立的设置窗口
#[tauri::command]
pub fn open_settings(app_handle: tauri::AppHandle) -> Result<(), String> {
    // 如果设置窗口已经存在，直接聚焦
    if let Some(window) = app_handle.get_webview_window("settings") {
        let _ = window.set_focus();
        return Ok(());
    }

    // 创建新的设置窗口
    let url = WebviewUrl::App("settings.html".into());
    WebviewWindowBuilder::new(&app_handle, "settings", url)
        .title("偏好设置")
        .inner_size(580.0, 520.0)
        .min_inner_size(480.0, 400.0)
        .resizable(true)
        .center()
        .focused(true)
        .visible(true)
        .decorations(false)
        .build()
        .map_err(|e| e.to_string())?;

    Ok(())
}
