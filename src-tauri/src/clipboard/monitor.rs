use clipboard_rs::{common::RustImage, Clipboard, ClipboardContext, RustImageData};
use image::GenericImageView;
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use uuid::Uuid;

use crate::db::Database;

/// 剪贴板监听器，后台轮询检测新复制的文本和图片
pub struct ClipboardMonitor {
    db: Arc<Database>,
    blobs_dir: PathBuf,
    app_handle: AppHandle,
    last_hash: std::sync::Mutex<String>,
}

impl ClipboardMonitor {
    pub fn new(db: Arc<Database>, app_data_dir: PathBuf, app_handle: AppHandle) -> Self {
        let blobs_dir = app_data_dir.join("blobs");
        std::fs::create_dir_all(&blobs_dir).ok();
        Self {
            db,
            blobs_dir,
            app_handle,
            last_hash: std::sync::Mutex::new(String::new()),
        }
    }

    /// 通知前端剪贴板内容已更新
    fn notify_update(&self) {
        let _ = self.app_handle.emit("clipboard-updated", ());
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
                // 优先检测图片
                if let Ok(image_data) = ctx.get_image() {
                    self.handle_image(image_data);
                } else if let Ok(text) = ctx.get_text() {
                    self.handle_text(&text);
                }
                std::thread::sleep(Duration::from_millis(500));
            }
        });
    }

    /// 处理文本剪贴板内容
    fn handle_text(&self, text: &str) {
        let text = text.trim().to_string();
        if text.is_empty() {
            return;
        }
        let hash = format!("{:x}", Sha256::digest(text.as_bytes()));
        let mut last = self.last_hash.lock().unwrap();
        if *last != hash {
            *last = hash;
            drop(last);
            match self.db.insert_text(&text) {
                Ok(_) => self.notify_update(),
                Err(e) => eprintln!("Failed to insert clipboard text: {}", e),
            }
        }
    }

    /// 处理图片剪贴板内容：保存原图 + 缩略图
    fn handle_image(&self, image_data: RustImageData) {
        let raw_bytes = match image_data.to_png() {
            Ok(buf) => buf,
            Err(_) => return,
        };
        let bytes = raw_bytes.get_bytes();
        if bytes.is_empty() {
            return;
        }

        // 计算图片数据哈希
        let hash = format!("{:x}", Sha256::digest(bytes));
        let mut last = self.last_hash.lock().unwrap();
        if *last == hash {
            return;
        }
        *last = hash.clone();
        drop(last);

        // 生成唯一文件名并保存原图
        let file_id = Uuid::new_v4().to_string();
        let original_path = self.blobs_dir.join(format!("{}.png", file_id));
        let thumb_path = self.blobs_dir.join(format!("{}_thumb.png", file_id));

        if std::fs::write(&original_path, bytes).is_err() {
            eprintln!("Failed to save clipboard image");
            return;
        }

        // 生成缩略图（宽度缩到 200px）
        if let Ok(img) = image::open(&original_path) {
            let (w, h) = img.dimensions();
            if w > 200 {
                let new_h = (200 * h) / w;
                let thumb = img.thumbnail(200, new_h);
                let _ = thumb.save(&thumb_path);
            } else {
                // 原图已经很小，直接复制作为缩略图
                let _ = std::fs::copy(&original_path, &thumb_path);
            }
        }

        // 存储到数据库，blob_path 保存相对路径（文件名）
        let blob_relative = format!("{}.png", file_id);
        match self.db.insert_image(&blob_relative, &hash) {
            Ok(_) => self.notify_update(),
            Err(e) => eprintln!("Failed to insert clipboard image: {}", e),
        }
    }
}
