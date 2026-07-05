use clipboard_rs::{common::RustImage, Clipboard, ClipboardContext, RustImageData};
use image::GenericImageView;
use sha2::{Digest, Sha256};
use std::panic;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::RwLock;
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
    /// 标志位：当我们自己写入剪贴板时临时跳过检测，避免回环
    pub skip_next: Arc<AtomicBool>,
    /// 排除应用列表（bundle ID），来自这些应用的复制不会被记录
    excluded_apps: Arc<RwLock<Vec<String>>>,
}

impl ClipboardMonitor {
    pub fn new(db: Arc<Database>, app_data_dir: PathBuf, app_handle: AppHandle, excluded_apps: Arc<RwLock<Vec<String>>>) -> Self {
        let blobs_dir = app_data_dir.join("blobs");
        std::fs::create_dir_all(&blobs_dir).ok();
        Self {
            db,
            blobs_dir,
            app_handle,
            last_hash: std::sync::Mutex::new(String::new()),
            skip_next: Arc::new(AtomicBool::new(false)),
            excluded_apps,
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
                // 如果是我们自己写入的，跳过本轮检测
                if self.skip_next.swap(false, Ordering::SeqCst) {
                    std::thread::sleep(Duration::from_millis(500));
                    continue;
                }

                // 检查当前前台应用是否在排除列表中
                let mut is_excluded = false;
                #[cfg(target_os = "macos")]
                {
                    if let Some(bundle_id) = crate::clipboard::get_frontmost_app_bundle_id() {
                        let excluded = self.excluded_apps.read().unwrap_or_else(|e| e.into_inner());
                        if excluded.iter().any(|id| id == &bundle_id) {
                            is_excluded = true;
                        }
                    }
                }
                #[cfg(target_os = "windows")]
                {
                    if let Some(exe_name) = crate::clipboard::get_frontmost_app_exe() {
                        let excluded = self.excluded_apps.read().unwrap_or_else(|e| e.into_inner());
                        if excluded.iter().any(|id| id.to_lowercase() == exe_name) {
                            is_excluded = true;
                        }
                    }
                }

                // 使用 catch_unwind 保护，防止 clipboard-rs 内部 panic 导致进程崩溃
                let monitor = self.clone();
                let result = panic::catch_unwind(panic::AssertUnwindSafe(|| {
                    if is_excluded {
                        // 排除应用：只更新 hash 不入库，防止切换应用后误触发保存
                        if let Ok(image_data) = ctx.get_image() {
                            if let Ok(buf) = image_data.to_png() {
                                let bytes = buf.get_bytes();
                                if !bytes.is_empty() {
                                    let hash = format!("{:x}", Sha256::digest(bytes));
                                    let mut last = monitor.last_hash.lock().unwrap();
                                    *last = hash;
                                }
                            }
                        } else if let Ok(text) = ctx.get_text() {
                            let text = text.trim().to_string();
                            if !text.is_empty() {
                                let hash = format!("{:x}", Sha256::digest(text.as_bytes()));
                                let mut last = monitor.last_hash.lock().unwrap();
                                *last = hash;
                            }
                        }
                    } else {
                        // 正常流程：检测并保存
                        if let Ok(image_data) = ctx.get_image() {
                            monitor.handle_image(image_data);
                        } else if let Ok(text) = ctx.get_text() {
                            monitor.handle_text(&text);
                        }
                    }
                }));

                if let Err(e) = result {
                    eprintln!("Clipboard poll panic caught: {:?}", e);
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
