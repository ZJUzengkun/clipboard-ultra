use crate::db::Database;
use chrono::Utc;
use regex::Regex;
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
    pub blob_path: Option<String>,
    pub tag: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub is_pinned: bool,
}

/// 标签规则数据结构
#[derive(Debug, Serialize, Clone)]
pub struct TagRule {
    pub id: i64,
    pub name: String,
    pub pattern: String,
    pub color: String,
    pub priority: i64,
    pub expire_days: i64,
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
            // 自动匹配标签
            let tag = Self::match_tag_with_conn(&conn, content);

            // 不存在：插入新记录
            conn.execute(
                "INSERT INTO clipboard_items (content_type, content, content_hash, tag, created_at, updated_at)
                 VALUES ('text', ?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![content, hash, tag, now, now],
            )
            .map_err(|e| e.to_string())?;
        }

        // 清理超限记录
        self.cleanup_old_items(&conn)?;
        Ok(())
    }

    /// 插入图片记录，存储 blob 路径
    pub fn insert_image(&self, blob_path: &str, content_hash: &str) -> Result<(), String> {
        let now = Utc::now().timestamp();
        let conn = self.conn.lock().map_err(|e| e.to_string())?;

        // 检查是否已存在相同哈希的图片
        let existing: Option<i64> = conn
            .query_row(
                "SELECT id FROM clipboard_items WHERE content_hash = ?1",
                [content_hash],
                |row| row.get(0),
            )
            .ok();

        if let Some(id) = existing {
            // 已存在：更新时间戳
            conn.execute(
                "UPDATE clipboard_items SET updated_at = ?1 WHERE id = ?2",
                rusqlite::params![now, id],
            )
            .map_err(|e| e.to_string())?;
        } else {
            // 不存在：插入新记录
            conn.execute(
                "INSERT INTO clipboard_items (content_type, content, content_hash, blob_path, created_at, updated_at)
                 VALUES ('image', '[图片]', ?1, ?2, ?3, ?4)",
                rusqlite::params![content_hash, blob_path, now, now],
            )
            .map_err(|e| e.to_string())?;
        }

        self.cleanup_old_items(&conn)?;
        Ok(())
    }

    /// 根据 ID 获取单条记录
    pub fn get_item_by_id(&self, id: i64) -> Result<Option<ClipboardItem>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let result = conn
            .query_row(
                "SELECT id, content_type, content, content_hash, blob_path, tag, created_at, updated_at, is_pinned
                 FROM clipboard_items WHERE id = ?1",
                [id],
                |row| {
                    Ok(ClipboardItem {
                        id: row.get(0)?,
                        content_type: row.get(1)?,
                        content: row.get(2)?,
                        content_hash: row.get(3)?,
                        blob_path: row.get(4)?,
                        tag: row.get(5)?,
                        created_at: row.get(6)?,
                        updated_at: row.get(7)?,
                        is_pinned: row.get::<_, i32>(8)? == 1,
                    })
                },
            )
            .ok();
        Ok(result)
    }

    /// 搜索历史记录（模糊匹配）
    pub fn search(&self, keyword: &str) -> Result<Vec<ClipboardItem>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let query = format!("%{}%", keyword);
        let mut stmt = conn
            .prepare(
                "SELECT id, content_type, content, content_hash, blob_path, tag, created_at, updated_at, is_pinned
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
                    blob_path: row.get(4)?,
                    tag: row.get(5)?,
                    created_at: row.get(6)?,
                    updated_at: row.get(7)?,
                    is_pinned: row.get::<_, i32>(8)? == 1,
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
                "SELECT id, content_type, content, content_hash, blob_path, tag, created_at, updated_at, is_pinned
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
                    blob_path: row.get(4)?,
                    tag: row.get(5)?,
                    created_at: row.get(6)?,
                    updated_at: row.get(7)?,
                    is_pinned: row.get::<_, i32>(8)? == 1,
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

    /// 刷新条目的 updated_at 为当前时间（用于粘贴使用时延长过期时间）
    pub fn touch_item(&self, id: i64) -> Result<(), String> {
        let now = Utc::now().timestamp();
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "UPDATE clipboard_items SET updated_at = ?1 WHERE id = ?2",
            rusqlite::params![now, id],
        )
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

    // ========== 标签规则相关方法 ==========

    /// 内部方法：用已锁定的连接匹配标签
    fn match_tag_with_conn(conn: &rusqlite::Connection, content: &str) -> Option<String> {
        let mut stmt = conn
            .prepare("SELECT name, pattern FROM tag_rules ORDER BY priority DESC")
            .ok()?;

        let rules: Vec<(String, String)> = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .ok()?
            .filter_map(|r| r.ok())
            .collect();

        for (name, pattern) in rules {
            // 空正则表示手动标签，不参与自动匹配（空正则会匹配一切，必须跳过）
            if pattern.is_empty() {
                continue;
            }
            if let Ok(re) = Regex::new(&pattern) {
                if re.is_match(content) {
                    return Some(name);
                }
            }
        }
        None
    }

    /// 获取所有标签规则（按优先级降序）
    pub fn get_tag_rules(&self) -> Result<Vec<TagRule>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare("SELECT id, name, pattern, color, priority, expire_days FROM tag_rules ORDER BY priority DESC")
            .map_err(|e| e.to_string())?;

        let rules = stmt
            .query_map([], |row| {
                Ok(TagRule {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    pattern: row.get(2)?,
                    color: row.get(3)?,
                    priority: row.get(4)?,
                    expire_days: row.get(5)?,
                })
            })
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();

        Ok(rules)
    }

    /// 新增标签规则
    pub fn add_tag_rule(&self, name: &str, pattern: &str, color: &str, priority: i64, expire_days: i64) -> Result<TagRule, String> {
        // 验证正则合法性（空正则表示手动标签，跳过校验）
        if !pattern.is_empty() {
            Regex::new(pattern).map_err(|e| format!("Invalid regex: {}", e))?;
        }

        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT INTO tag_rules (name, pattern, color, priority, expire_days) VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![name, pattern, color, priority, expire_days],
        )
        .map_err(|e| e.to_string())?;

        let id = conn.last_insert_rowid();
        Ok(TagRule {
            id,
            name: name.to_string(),
            pattern: pattern.to_string(),
            color: color.to_string(),
            priority,
            expire_days,
        })
    }

    /// 删除标签规则
    pub fn delete_tag_rule(&self, id: i64) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute("DELETE FROM tag_rules WHERE id = ?1", [id])
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// 更新标签规则的过期天数
    pub fn update_tag_rule_expire(&self, id: i64, expire_days: i64) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "UPDATE tag_rules SET expire_days = ?1 WHERE id = ?2",
            rusqlite::params![expire_days, id],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// 清理过期条目
    /// 1. 置顶条目永不过期
    /// 2. 有标签的条目按对应规则的 expire_days 清理
    /// 3. 无标签条目按全局默认过期天数清理
    pub fn cleanup_expired_items(&self) -> Result<u64, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let now = Utc::now().timestamp();
        let mut total_deleted: u64 = 0;

        // 获取所有标签规则及其过期天数
        let mut stmt = conn
            .prepare("SELECT name, expire_days FROM tag_rules WHERE expire_days > 0")
            .map_err(|e| e.to_string())?;
        let rules: Vec<(String, i64)> = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();

        // 按标签规则清理
        for (tag_name, expire_days) in &rules {
            let cutoff = now - expire_days * 86400;
            let deleted = conn
                .execute(
                    "DELETE FROM clipboard_items WHERE tag = ?1 AND is_pinned = 0 AND updated_at < ?2",
                    rusqlite::params![tag_name, cutoff],
                )
                .map_err(|e| e.to_string())?;
            total_deleted += deleted as u64;
        }

        // 按内容类型清理无标签条目
        let known_types = ["image", "text"];
        let mut configured_types: Vec<String> = Vec::new();

        for ct in &known_types {
            let key = format!("expire_days_{}", ct);
            let expire: i64 = conn
                .query_row(
                    "SELECT value FROM app_config WHERE key = ?1",
                    rusqlite::params![key],
                    |row| row.get::<_, String>(0),
                )
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(-1); // -1 表示未配置，跟随默认

            if expire >= 0 {
                configured_types.push(ct.to_string());
                if expire > 0 {
                    let cutoff = now - expire * 86400;
                    let deleted = conn
                        .execute(
                            "DELETE FROM clipboard_items WHERE tag IS NULL AND content_type = ?1 AND is_pinned = 0 AND updated_at < ?2",
                            rusqlite::params![*ct, cutoff],
                        )
                        .map_err(|e| e.to_string())?;
                    total_deleted += deleted as u64;
                }
                // expire == 0 表示永不过期，不清理
            }
        }

        // 默认过期策略：覆盖未单独配置的内容类型
        let default_expire: i64 = conn
            .query_row(
                "SELECT value FROM app_config WHERE key = 'default_expire_days'",
                [],
                |row| row.get::<_, String>(0),
            )
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);

        if default_expire > 0 {
            let cutoff = now - default_expire * 86400;
            if configured_types.is_empty() {
                // 没有任何类型单独配置，全部无标签条目用默认
                let deleted = conn
                    .execute(
                        "DELETE FROM clipboard_items WHERE tag IS NULL AND is_pinned = 0 AND updated_at < ?1",
                        rusqlite::params![cutoff],
                    )
                    .map_err(|e| e.to_string())?;
                total_deleted += deleted as u64;
            } else {
                // 只清理未单独配置的类型
                let placeholders: Vec<String> = configured_types.iter().enumerate().map(|(i, _)| format!("?{}", i + 2)).collect();
                let sql = format!(
                    "DELETE FROM clipboard_items WHERE tag IS NULL AND content_type NOT IN ({}) AND is_pinned = 0 AND updated_at < ?1",
                    placeholders.join(", ")
                );
                let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
                let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
                params.push(Box::new(cutoff));
                for ct in &configured_types {
                    params.push(Box::new(ct.clone()));
                }
                let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
                let deleted = stmt.execute(param_refs.as_slice()).map_err(|e| e.to_string())?;
                total_deleted += deleted as u64;
            }
        }

        Ok(total_deleted)
    }

    /// 手动设置/清除条目的标签
    pub fn set_item_tag(&self, id: i64, tag: Option<&str>) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "UPDATE clipboard_items SET tag = ?1 WHERE id = ?2",
            rusqlite::params![tag, id],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// 按标签筛选条目
    pub fn get_by_tag(&self, tag: &str, limit: u32) -> Result<Vec<ClipboardItem>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare(
                "SELECT id, content_type, content, content_hash, blob_path, tag, created_at, updated_at, is_pinned
                 FROM clipboard_items
                 WHERE tag = ?1
                 ORDER BY is_pinned DESC, updated_at DESC
                 LIMIT ?2",
            )
            .map_err(|e| e.to_string())?;

        let items = stmt
            .query_map(rusqlite::params![tag, limit], |row| {
                Ok(ClipboardItem {
                    id: row.get(0)?,
                    content_type: row.get(1)?,
                    content: row.get(2)?,
                    content_hash: row.get(3)?,
                    blob_path: row.get(4)?,
                    tag: row.get(5)?,
                    created_at: row.get(6)?,
                    updated_at: row.get(7)?,
                    is_pinned: row.get::<_, i32>(8)? == 1,
                })
            })
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();

        Ok(items)
    }
}
