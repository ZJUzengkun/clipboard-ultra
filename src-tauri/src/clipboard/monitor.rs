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
    /// 来源应用显示名缓存（bundle ID / exe 名 → 显示名），避免每次复制都 spawn 进程解析
    app_names: std::sync::Mutex<std::collections::HashMap<String, String>>,
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
            app_names: std::sync::Mutex::new(std::collections::HashMap::new()),
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

                // 采集当前前台应用（一份数据两用：排除判断 + 来源应用记录）
                #[cfg(target_os = "macos")]
                let source_app: Option<String> = crate::clipboard::get_frontmost_app_bundle_id();
                #[cfg(target_os = "windows")]
                let source_app: Option<String> = crate::clipboard::get_frontmost_app_exe();
                #[cfg(not(any(target_os = "macos", target_os = "windows")))]
                let source_app: Option<String> = None;

                // 检查当前前台应用是否在排除列表中
                let mut is_excluded = false;
                if let Some(ref app_id) = source_app {
                    let excluded = self.excluded_apps.read().unwrap_or_else(|e| e.into_inner());
                    // Windows 侧 exe 名已统一小写，排除列表按小写比对
                    #[cfg(target_os = "windows")]
                    {
                        is_excluded = excluded.iter().any(|id| id.to_lowercase() == *app_id);
                    }
                    #[cfg(not(target_os = "windows"))]
                    {
                        is_excluded = excluded.iter().any(|id| id == app_id);
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
                            monitor.handle_image(image_data, source_app.as_deref());
                        } else if let Ok(text) = ctx.get_text() {
                            monitor.handle_text(&text, source_app.as_deref());
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

    /// 解析来源应用显示名（带缓存）：macOS 用 lsappinfo 查 bundle ID 对应名称，Windows 去掉 .exe 后缀
    fn resolve_app_name(&self, app_id: &str) -> Option<String> {
        if let Some(name) = self.app_names.lock().unwrap().get(app_id) {
            return Some(name.clone());
        }
        #[cfg(target_os = "macos")]
        let name: Option<String> = std::process::Command::new("lsappinfo")
            .args(["info", "-only", "name", app_id])
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            // 输出形如 "LSDisplayName"="Google Chrome"，取第二对引号内容
            .and_then(|s| s.split('"').nth(3).map(|n| n.to_string()))
            .filter(|n| !n.is_empty())
            // 取不到时退化为 bundle ID 末段（如 com.google.Chrome → Chrome）
            .or_else(|| app_id.rsplit('.').next().map(|n| n.to_string()));
        #[cfg(target_os = "windows")]
        let name: Option<String> = Some(app_id.trim_end_matches(".exe").to_string());
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        let name: Option<String> = None;

        if let Some(ref n) = name {
            self.app_names.lock().unwrap().insert(app_id.to_string(), n.clone());
        }
        name
    }

    /// 处理文本剪贴板内容
    fn handle_text(&self, text: &str, source_app: Option<&str>) {
        let text = text.trim().to_string();
        if text.is_empty() {
            return;
        }
        let hash = format!("{:x}", Sha256::digest(text.as_bytes()));
        let mut last = self.last_hash.lock().unwrap();
        if *last != hash {
            *last = hash;
            drop(last);
            // hash 确认变化后才解析显示名，避免每轮轮询都 spawn 进程
            let source_name = source_app.and_then(|id| self.resolve_app_name(id));
            match self.db.insert_text(&text, source_app, source_name.as_deref()) {
                Ok(_) => self.notify_update(),
                Err(e) => eprintln!("Failed to insert clipboard text: {}", e),
            }
        }
    }

    /// 处理图片剪贴板内容：保存原图 + 缩略图
    fn handle_image(&self, image_data: RustImageData, source_app: Option<&str>) {
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
        let source_name = source_app.and_then(|id| self.resolve_app_name(id));
        match self.db.insert_image(&blob_relative, &hash, source_app, source_name.as_deref()) {
            Ok(_) => self.notify_update(),
            Err(e) => eprintln!("Failed to insert clipboard image: {}", e),
        }
    }
}
