pub mod operations;

use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::Mutex;

/// 数据库管理结构体，封装 SQLite 连接
pub struct Database {
    pub conn: Mutex<Connection>,
}

impl Database {
    /// 初始化数据库：创建文件和表结构
    pub fn new(app_data_dir: PathBuf) -> Result<Self, rusqlite::Error> {
        std::fs::create_dir_all(&app_data_dir).ok();
        let db_path = app_data_dir.join("clipboard.db");
        let conn = Connection::open(db_path)?;

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS clipboard_items (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                content_type TEXT NOT NULL DEFAULT 'text',
                content TEXT NOT NULL,
                content_hash TEXT NOT NULL,
                blob_path TEXT,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                is_pinned INTEGER DEFAULT 0
            );
            CREATE INDEX IF NOT EXISTS idx_content_hash ON clipboard_items(content_hash);
            CREATE INDEX IF NOT EXISTS idx_updated_at ON clipboard_items(updated_at DESC);
            CREATE INDEX IF NOT EXISTS idx_is_pinned ON clipboard_items(is_pinned);
            ",
        )?;

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }
}
