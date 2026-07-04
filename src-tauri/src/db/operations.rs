use crate::db::Database;
use chrono::Utc;
use rusqlite;
use serde::Serialize;
use sha2::{Digest, Sha256};

/// 剪贴板条目数据结构，用于前后端传输
#[derive(Debug, Serialize, Clone)]
pub struct ClipboardItem {
    pub id: i64,
    pub content_type: String,
    pub content: String,
    pub content_hash: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub is_pinned: bool,
}

impl Database {
    /// 插入新文本，自动去重（相同内容置顶）
    pub fn insert_text(&self, content: &str) -> Result<(), String> {
        let hash = format!("{:x}", Sha256::digest(content.as_bytes()));
        let now = Utc::now().timestamp();
        let conn = self.conn.lock().map_err(|e| e.to_string())?;

        // 检查是否已存在相同内容
        let existing: Option<i64> = conn
            .query_row(
                "SELECT id FROM clipboard_items WHERE content_hash = ?1",
                [&hash],
                |row| row.get(0),
            )
            .ok();

        if let Some(id) = existing {
            // 已存在：更新时间戳（置顶）
            conn.execute(
                "UPDATE clipboard_items SET updated_at = ?1 WHERE id = ?2",
                rusqlite::params![now, id],
            )
            .map_err(|e| e.to_string())?;
        } else {
            // 不存在：插入新记录
            conn.execute(
                "INSERT INTO clipboard_items (content_type, content, content_hash, created_at, updated_at)
                 VALUES ('text', ?1, ?2, ?3, ?4)",
                rusqlite::params![content, hash, now, now],
            )
            .map_err(|e| e.to_string())?;
        }

        // 清理超限记录
        self.cleanup_old_items(&conn)?;
        Ok(())
    }

    /// 搜索历史记录（模糊匹配）
    pub fn search(&self, keyword: &str) -> Result<Vec<ClipboardItem>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let query = format!("%{}%", keyword);
        let mut stmt = conn
            .prepare(
                "SELECT id, content_type, content, content_hash, created_at, updated_at, is_pinned
                 FROM clipboard_items
                 WHERE content LIKE ?1
                 ORDER BY is_pinned DESC, updated_at DESC
                 LIMIT 50",
            )
            .map_err(|e| e.to_string())?;

        let items = stmt
            .query_map([&query], |row| {
                Ok(ClipboardItem {
                    id: row.get(0)?,
                    content_type: row.get(1)?,
                    content: row.get(2)?,
                    content_hash: row.get(3)?,
                    created_at: row.get(4)?,
                    updated_at: row.get(5)?,
                    is_pinned: row.get::<_, i32>(6)? == 1,
                })
            })
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();

        Ok(items)
    }

    /// 获取最近的历史列表
    pub fn get_recent(&self, limit: u32) -> Result<Vec<ClipboardItem>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare(
                "SELECT id, content_type, content, content_hash, created_at, updated_at, is_pinned
                 FROM clipboard_items
                 ORDER BY is_pinned DESC, updated_at DESC
                 LIMIT ?1",
            )
            .map_err(|e| e.to_string())?;

        let items = stmt
            .query_map([limit], |row| {
                Ok(ClipboardItem {
                    id: row.get(0)?,
                    content_type: row.get(1)?,
                    content: row.get(2)?,
                    content_hash: row.get(3)?,
                    created_at: row.get(4)?,
                    updated_at: row.get(5)?,
                    is_pinned: row.get::<_, i32>(6)? == 1,
                })
            })
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();

        Ok(items)
    }

    /// 切换收藏状态
    pub fn toggle_pin(&self, id: i64) -> Result<bool, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "UPDATE clipboard_items SET is_pinned = CASE WHEN is_pinned = 1 THEN 0 ELSE 1 END WHERE id = ?1",
            [id],
        )
        .map_err(|e| e.to_string())?;

        let is_pinned: i32 = conn
            .query_row(
                "SELECT is_pinned FROM clipboard_items WHERE id = ?1",
                [id],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;

        Ok(is_pinned == 1)
    }

    /// 删除单条记录
    pub fn delete_item(&self, id: i64) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute("DELETE FROM clipboard_items WHERE id = ?1", [id])
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// 清理超限记录（保留最近 1000 条未收藏的）
    fn cleanup_old_items(&self, conn: &rusqlite::Connection) -> Result<(), String> {
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM clipboard_items WHERE is_pinned = 0",
                [],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;

        if count > 1000 {
            conn.execute(
                "DELETE FROM clipboard_items WHERE id IN (
                    SELECT id FROM clipboard_items WHERE is_pinned = 0
                    ORDER BY updated_at ASC LIMIT ?1
                )",
                [count - 1000],
            )
            .map_err(|e| e.to_string())?;
        }
        Ok(())
    }
}
