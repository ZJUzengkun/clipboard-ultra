use crate::db::{operations::ClipboardItem, Database};
use clipboard_rs::{common::RustImage, Clipboard, ClipboardContext};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{Manager, State};

/// 应用全局状态，存储数据库引用和 blob 目录
pub struct AppState {
    pub db: Arc<Database>,
    pub blobs_dir: PathBuf,
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
    std::thread::spawn(|| {
        std::thread::sleep(std::time::Duration::from_millis(100));
        if let Ok(mut enigo) = enigo::Enigo::new(&enigo::Settings::default()) {
            use enigo::{Direction, Key, Keyboard};
            #[cfg(target_os = "macos")]
            {
                let _ = enigo.key(Key::Meta, Direction::Press);
                let _ = enigo.key(Key::Unicode('v'), Direction::Click);
                let _ = enigo.key(Key::Meta, Direction::Release);
            }
            #[cfg(not(target_os = "macos"))]
            {
                let _ = enigo.key(Key::Control, Direction::Press);
                let _ = enigo.key(Key::Unicode('v'), Direction::Click);
                let _ = enigo.key(Key::Control, Direction::Release);
            }
        }
    });

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
