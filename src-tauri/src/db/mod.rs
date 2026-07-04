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
                tag TEXT,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                is_pinned INTEGER DEFAULT 0
            );
            CREATE INDEX IF NOT EXISTS idx_content_hash ON clipboard_items(content_hash);
            CREATE INDEX IF NOT EXISTS idx_updated_at ON clipboard_items(updated_at DESC);
            CREATE INDEX IF NOT EXISTS idx_is_pinned ON clipboard_items(is_pinned);
            CREATE INDEX IF NOT EXISTS idx_tag ON clipboard_items(tag);

            CREATE TABLE IF NOT EXISTS tag_rules (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                pattern TEXT NOT NULL,
                color TEXT NOT NULL DEFAULT '#7c6df0',
                priority INTEGER NOT NULL DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS app_config (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            ",
        )?;

        // 兼容旧数据库：添加 tag 列（如果不存在）
        let _ = conn.execute("ALTER TABLE clipboard_items ADD COLUMN tag TEXT", []);

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// 获取配置值
    pub fn get_config(&self, key: &str) -> Result<Option<String>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let result = conn
            .query_row(
                "SELECT value FROM app_config WHERE key = ?1",
                [key],
                |row| row.get(0),
            )
            .ok();
        Ok(result)
    }

    /// 设置配置值
    pub fn set_config(&self, key: &str, value: &str) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT OR REPLACE INTO app_config (key, value) VALUES (?1, ?2)",
            rusqlite::params![key, value],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }
}
