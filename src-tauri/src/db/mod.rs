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

        // 创建基础表（不含新增列的索引，避免旧数据库兼容问题）
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

            CREATE TABLE IF NOT EXISTS boards (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                color TEXT NOT NULL DEFAULT '#f5c518',
                sort_order INTEGER NOT NULL DEFAULT 0,
                is_builtin INTEGER NOT NULL DEFAULT 0,
                created_at INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS board_items (
                board_id INTEGER NOT NULL,
                item_id INTEGER NOT NULL,
                added_at INTEGER NOT NULL,
                PRIMARY KEY (board_id, item_id),
                FOREIGN KEY (board_id) REFERENCES boards(id) ON DELETE CASCADE,
                FOREIGN KEY (item_id) REFERENCES clipboard_items(id) ON DELETE CASCADE
            );
            ",
        )?;

        // 外键级联需显式开启
        let _ = conn.execute("PRAGMA foreign_keys = ON", []);

        // 兼容旧数据库：添加 tag 列（如果不存在）— 必须在创建索引之前
        let _ = conn.execute("ALTER TABLE clipboard_items ADD COLUMN tag TEXT", []);

        // 兼容旧数据库：添加 expire_days 列到 tag_rules
        let _ = conn.execute("ALTER TABLE tag_rules ADD COLUMN expire_days INTEGER NOT NULL DEFAULT 0", []);

        // 创建索引（此时 tag 列已确保存在）
        conn.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_content_hash ON clipboard_items(content_hash);
            CREATE INDEX IF NOT EXISTS idx_updated_at ON clipboard_items(updated_at DESC);
            CREATE INDEX IF NOT EXISTS idx_is_pinned ON clipboard_items(is_pinned);
            CREATE INDEX IF NOT EXISTS idx_tag ON clipboard_items(tag);
            CREATE INDEX IF NOT EXISTS idx_board_items_item ON board_items(item_id);
            ",
        )?;

        // 迁移：确保存在内置"收藏"板，并把历史 is_pinned=1 的条目并入收藏板（幂等）
        Self::ensure_favorites_board(&conn);

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// 确保内置"收藏"板存在，并把旧的 is_pinned=1 条目迁入（幂等）
    fn ensure_favorites_board(conn: &Connection) {
        let now = chrono::Utc::now().timestamp();
        // 内置收藏板不存在则创建（sort_order=0 置于最前）
        let fav_id: Option<i64> = conn
            .query_row(
                "SELECT id FROM boards WHERE is_builtin = 1 ORDER BY id LIMIT 1",
                [],
                |row| row.get(0),
            )
            .ok();
        let fav_id = match fav_id {
            Some(id) => {
                // 历史版本名为"收藏"，统一改名"收藏夹"（幂等）
                let _ = conn.execute(
                    "UPDATE boards SET name = '收藏夹' WHERE is_builtin = 1 AND name = '收藏'",
                    [],
                );
                id
            }
            None => {
                let _ = conn.execute(
                    "INSERT INTO boards (name, color, sort_order, is_builtin, created_at)
                     VALUES ('收藏夹', '#f5c518', 0, 1, ?1)",
                    [now],
                );
                conn.last_insert_rowid()
            }
        };
        // 把历史收藏条目并入收藏板（INSERT OR IGNORE 保证幂等）
        let _ = conn.execute(
            "INSERT OR IGNORE INTO board_items (board_id, item_id, added_at)
             SELECT ?1, id, ?2 FROM clipboard_items WHERE is_pinned = 1",
            [fav_id, now],
        );
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
