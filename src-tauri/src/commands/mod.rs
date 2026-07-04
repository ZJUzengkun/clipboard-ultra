use crate::db::{operations::ClipboardItem, Database};
use clipboard_rs::{Clipboard, ClipboardContext};
use std::sync::Arc;
use tauri::{Manager, State};

/// 应用全局状态，存储数据库引用
pub struct AppState {
    pub db: Arc<Database>,
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
    // 获取目标条目内容
    let items = state.db.get_recent(1000)?;
    let item = items
        .iter()
        .find(|i| i.id == id)
        .ok_or_else(|| "Item not found".to_string())?;

    // 写入系统剪贴板
    let ctx = ClipboardContext::new().map_err(|e| e.to_string())?;
    ctx.set_text(item.content.clone())
        .map_err(|e| e.to_string())?;

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
