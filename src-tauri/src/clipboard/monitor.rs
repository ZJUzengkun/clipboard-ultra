use clipboard_rs::{Clipboard, ClipboardContext};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use std::time::Duration;

use crate::db::Database;

/// 剪贴板监听器，后台轮询检测新复制的文本
pub struct ClipboardMonitor {
    db: Arc<Database>,
    last_hash: std::sync::Mutex<String>,
}

impl ClipboardMonitor {
    pub fn new(db: Arc<Database>) -> Self {
        Self {
            db,
            last_hash: std::sync::Mutex::new(String::new()),
        }
    }

    /// 启动后台轮询线程，每 500ms 检测一次剪贴板变化
    pub fn start(self: Arc<Self>) {
        std::thread::spawn(move || {
            let ctx = match ClipboardContext::new() {
                Ok(ctx) => ctx,
                Err(e) => {
                    eprintln!("Failed to create clipboard context: {}", e);
                    return;
                }
            };

            loop {
                if let Ok(text) = ctx.get_text() {
                    let text = text.trim().to_string();
                    if !text.is_empty() {
                        let hash = format!("{:x}", Sha256::digest(text.as_bytes()));
                        let mut last = self.last_hash.lock().unwrap();
                        if *last != hash {
                            *last = hash;
                            drop(last); // 释放锁再操作数据库
                            if let Err(e) = self.db.insert_text(&text) {
                                eprintln!("Failed to insert clipboard text: {}", e);
                            }
                        }
                    }
                }
                std::thread::sleep(Duration::from_millis(500));
            }
        });
    }
}
