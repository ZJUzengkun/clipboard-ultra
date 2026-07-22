use crate::db::Database;
use chrono::Utc;
use regex::Regex;
use rusqlite;
use serde::{Serialize, Deserialize};
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
    pub use_count: i64,
    pub last_used_at: Option<i64>,
    pub source_app: Option<String>,
    pub source_app_name: Option<String>,
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

/// 收藏板数据结构
#[derive(Debug, Serialize, Clone)]
pub struct Board {
    pub id: i64,
    pub name: String,
    pub color: String,
    pub sort_order: i64,
    pub is_builtin: bool,
}

// ========== 导入/导出数据结构 ==========

/// 导出条目：图片以 base64 内嵌，boards 记录所属板名（按名关联，避免 id 漂移）
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ExportItem {
    pub content_type: String,
    pub content: String,
    pub content_hash: String,
    pub tag: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub is_pinned: bool,
    pub use_count: i64,
    pub last_used_at: Option<i64>,
    pub source_app: Option<String>,
    pub source_app_name: Option<String>,
    #[serde(default)]
    pub image_base64: Option<String>,
    #[serde(default)]
    pub boards: Vec<String>,
    /// 工作字段：导出时为原始 blob 相对路径、导入时为新写入路径，不参与 JSON
    #[serde(skip)]
    pub blob_path: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ExportTagRule {
    pub name: String,
    pub pattern: String,
    pub color: String,
    pub priority: i64,
    pub expire_days: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ExportBoard {
    pub name: String,
    pub color: String,
    pub sort_order: i64,
    pub is_builtin: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ExportData {
    pub version: u32,
    pub exported_at: i64,
    pub items: Vec<ExportItem>,
    pub tag_rules: Vec<ExportTagRule>,
    pub boards: Vec<ExportBoard>,
}

impl Database {
    /// 条目查询的统一列清单与行映射（新增列时同步维护这两处）
    const ITEM_COLS: &'static str = "id, content_type, content, content_hash, blob_path, tag, created_at, updated_at, is_pinned, use_count, last_used_at, source_app, source_app_name";

    fn map_item(row: &rusqlite::Row) -> rusqlite::Result<ClipboardItem> {
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
            use_count: row.get(9)?,
            last_used_at: row.get(10)?,
            source_app: row.get(11)?,
            source_app_name: row.get(12)?,
        })
    }

    /// 插入新文本，自动去重（相同内容置顶并累加使用次数）
    pub fn insert_text(&self, content: &str, source_app: Option<&str>, source_app_name: Option<&str>) -> Result<(), String> {
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
            // 已存在：更新时间戳（置顶），重复复制计入使用次数
            conn.execute(
                "UPDATE clipboard_items SET updated_at = ?1, use_count = use_count + 1, last_used_at = ?1 WHERE id = ?2",
                rusqlite::params![now, id],
            )
            .map_err(|e| e.to_string())?;
        } else {
            // 自动匹配标签
            let tag = Self::match_tag_with_conn(&conn, content);

            // 不存在：插入新记录
            conn.execute(
                "INSERT INTO clipboard_items (content_type, content, content_hash, tag, created_at, updated_at, source_app, source_app_name)
                 VALUES ('text', ?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                rusqlite::params![content, hash, tag, now, now, source_app, source_app_name],
            )
            .map_err(|e| e.to_string())?;
        }

        // 清理超限记录
        self.cleanup_old_items(&conn)?;
        Ok(())
    }

    /// 插入图片记录，存储 blob 路径
    pub fn insert_image(&self, blob_path: &str, content_hash: &str, source_app: Option<&str>, source_app_name: Option<&str>) -> Result<(), String> {
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
            // 已存在：更新时间戳，重复复制计入使用次数
            conn.execute(
                "UPDATE clipboard_items SET updated_at = ?1, use_count = use_count + 1, last_used_at = ?1 WHERE id = ?2",
                rusqlite::params![now, id],
            )
            .map_err(|e| e.to_string())?;
        } else {
            // 不存在：插入新记录
            conn.execute(
                "INSERT INTO clipboard_items (content_type, content, content_hash, blob_path, created_at, updated_at, source_app, source_app_name)
                 VALUES ('image', '[图片]', ?1, ?2, ?3, ?4, ?5, ?6)",
                rusqlite::params![content_hash, blob_path, now, now, source_app, source_app_name],
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
                &format!("SELECT {} FROM clipboard_items WHERE id = ?1", Self::ITEM_COLS),
                [id],
                Self::map_item,
            )
            .ok();
        Ok(result)
    }

    /// 搜索历史记录（模糊匹配）
    pub fn search(&self, keyword: &str) -> Result<Vec<ClipboardItem>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let query = format!("%{}%", keyword);
        let mut stmt = conn
            .prepare(&format!(
                "SELECT {} FROM clipboard_items WHERE content LIKE ?1 ORDER BY updated_at DESC LIMIT 50",
                Self::ITEM_COLS
            ))
            .map_err(|e| e.to_string())?;

        let items = stmt
            .query_map([&query], Self::map_item)
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();

        Ok(items)
    }

    /// 获取最近的历史列表（支持分页 offset）
    pub fn get_recent(&self, limit: u32, offset: u32) -> Result<Vec<ClipboardItem>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare(&format!(
                "SELECT {} FROM clipboard_items ORDER BY updated_at DESC LIMIT ?1 OFFSET ?2",
                Self::ITEM_COLS
            ))
            .map_err(|e| e.to_string())?;

        let items = stmt
            .query_map([limit, offset], Self::map_item)
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();

        Ok(items)
    }

    /// 获取历史条目总数
    pub fn count_items(&self) -> Result<i64, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.query_row("SELECT COUNT(*) FROM clipboard_items", [], |row| row.get(0))
            .map_err(|e| e.to_string())
    }

    /// 获取收藏（置顶）条目列表
    pub fn get_pinned(&self, limit: u32) -> Result<Vec<ClipboardItem>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare(&format!(
                "SELECT {} FROM clipboard_items WHERE is_pinned = 1 ORDER BY updated_at DESC LIMIT ?1",
                Self::ITEM_COLS
            ))
            .map_err(|e| e.to_string())?;

        let items = stmt
            .query_map([limit], Self::map_item)
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();

        Ok(items)
    }

    /// 切换收藏状态（同步收藏板成员）
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

        // 同步内置"收藏"板成员：is_pinned 是其镜像
        if let Some(fav_id) = Self::favorites_board_id(&conn) {
            if is_pinned == 1 {
                let now = Utc::now().timestamp();
                let _ = conn.execute(
                    "INSERT OR IGNORE INTO board_items (board_id, item_id, added_at) VALUES (?1, ?2, ?3)",
                    rusqlite::params![fav_id, id, now],
                );
            } else {
                let _ = conn.execute(
                    "DELETE FROM board_items WHERE board_id = ?1 AND item_id = ?2",
                    rusqlite::params![fav_id, id],
                );
            }
        }

        Ok(is_pinned == 1)
    }

    /// 获取内置"收藏"板 id
    fn favorites_board_id(conn: &rusqlite::Connection) -> Option<i64> {
        conn.query_row(
            "SELECT id FROM boards WHERE is_builtin = 1 ORDER BY id LIMIT 1",
            [],
            |row| row.get(0),
        )
        .ok()
    }

    /// 删除单条记录
    pub fn delete_item(&self, id: i64) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute("DELETE FROM clipboard_items WHERE id = ?1", [id])
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// 刷新条目的 updated_at 为当前时间（用于粘贴使用时延长过期时间），同时累加使用次数
    pub fn touch_item(&self, id: i64) -> Result<(), String> {
        let now = Utc::now().timestamp();
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "UPDATE clipboard_items SET updated_at = ?1, use_count = use_count + 1, last_used_at = ?1 WHERE id = ?2",
            rusqlite::params![now, id],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// 更新条目文本内容（仅文本类）：重算 hash，标签/板归属/收藏状态保留不动
    pub fn update_item_content(&self, id: i64, content: &str) -> Result<(), String> {
        let content = content.trim();
        if content.is_empty() {
            return Err("内容不能为空".to_string());
        }
        let hash = format!("{:x}", Sha256::digest(content.as_bytes()));
        let now = Utc::now().timestamp();
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let updated = conn
            .execute(
                "UPDATE clipboard_items SET content = ?1, content_hash = ?2, updated_at = ?3 WHERE id = ?4 AND content_type = 'text'",
                rusqlite::params![content, hash, now, id],
            )
            .map_err(|e| e.to_string())?;
        if updated == 0 {
            return Err("条目不存在或非文本类型".to_string());
        }
        Ok(())
    }

    /// 清理超限记录（保留最近 N 条未收藏的，N 来自 app_config.max_items，默认 1000，负数表示不限制）
    fn cleanup_old_items(&self, conn: &rusqlite::Connection) -> Result<(), String> {
        // 读取用户配置的最大保存数量，默认 1000
        let max_items: i64 = conn
            .query_row(
                "SELECT value FROM app_config WHERE key = 'max_items'",
                [],
                |row| row.get::<_, String>(0),
            )
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(1000);

        // 负数表示不限制，跳过清理
        if max_items < 0 {
            return Ok(());
        }

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM clipboard_items WHERE id NOT IN (SELECT item_id FROM board_items)",
                [],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;

        if count > max_items {
            conn.execute(
                "DELETE FROM clipboard_items WHERE id IN (
                    SELECT id FROM clipboard_items WHERE id NOT IN (SELECT item_id FROM board_items)
                    ORDER BY updated_at ASC LIMIT ?1
                )",
                [count - max_items],
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

        let trimmed = content.trim();
        for (name, pattern) in rules {
            // 空正则表示手动标签，不参与自动匹配（空正则会匹配一切，必须跳过）
            if pattern.is_empty() {
                continue;
            }
            // builtin: 前缀走内置检测器（解析器/校验位算法，比正则更准）
            if let Some(kind) = pattern.strip_prefix("builtin:") {
                if builtin_detect(kind, trimmed) {
                    return Some(name);
                }
                continue;
            }
            // 全串匹配：内容整体命中正则才打标，避免 JSON 等长文本因包含邮箱片段被误标
            if let Ok(re) = Regex::new(&format!("^(?:{})$", pattern)) {
                if re.is_match(trimmed) {
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

    // ========== 导入/导出 ==========

    /// 导出全部条目（含所属板名），blob_path 为原始相对路径供命令层读取图片
    pub fn export_items(&self) -> Result<Vec<ExportItem>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        // 板成员映射：item_id -> Vec<board_name>
        let mut membership: std::collections::HashMap<i64, Vec<String>> = std::collections::HashMap::new();
        {
            let mut stmt = conn
                .prepare("SELECT bi.item_id, b.name FROM board_items bi JOIN boards b ON b.id = bi.board_id")
                .map_err(|e| e.to_string())?;
            let rows = stmt
                .query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)))
                .map_err(|e| e.to_string())?;
            for r in rows {
                if let Ok((item_id, name)) = r {
                    membership.entry(item_id).or_default().push(name);
                }
            }
        }
        let sql = format!("SELECT {} FROM clipboard_items ORDER BY id ASC", Self::ITEM_COLS);
        let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
        let rows = stmt.query_map([], Self::map_item).map_err(|e| e.to_string())?;
        let mut out = Vec::new();
        for r in rows {
            let it = r.map_err(|e| e.to_string())?;
            let boards = membership.remove(&it.id).unwrap_or_default();
            out.push(ExportItem {
                content_type: it.content_type,
                content: it.content,
                content_hash: it.content_hash,
                tag: it.tag,
                created_at: it.created_at,
                updated_at: it.updated_at,
                is_pinned: it.is_pinned,
                use_count: it.use_count,
                last_used_at: it.last_used_at,
                source_app: it.source_app,
                source_app_name: it.source_app_name,
                image_base64: None,
                boards,
                blob_path: it.blob_path,
            });
        }
        Ok(out)
    }

    /// 导出标签规则
    pub fn export_tag_rules(&self) -> Result<Vec<ExportTagRule>, String> {
        let rules = self.get_tag_rules()?;
        Ok(rules
            .into_iter()
            .map(|r| ExportTagRule {
                name: r.name,
                pattern: r.pattern,
                color: r.color,
                priority: r.priority,
                expire_days: r.expire_days,
            })
            .collect())
    }

    /// 导出收藏板
    pub fn export_boards(&self) -> Result<Vec<ExportBoard>, String> {
        let boards = self.list_boards()?;
        Ok(boards
            .into_iter()
            .map(|b| ExportBoard {
                name: b.name,
                color: b.color,
                sort_order: b.sort_order,
                is_builtin: b.is_builtin,
            })
            .collect())
    }

    /// 导入数据（合并策略）：条目按 content_hash 去重跳过；标签规则/板按名去重；恢复板归属。返回新导入条目数
    pub fn import_all(
        &self,
        items: &[ExportItem],
        tag_rules: &[ExportTagRule],
        boards: &[ExportBoard],
    ) -> Result<usize, String> {
        let mut conn = self.conn.lock().map_err(|e| e.to_string())?;
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        let now = Utc::now().timestamp();

        // 1. 标签规则：按名去重
        for r in tag_rules {
            let exists: bool = tx
                .query_row("SELECT 1 FROM tag_rules WHERE name = ?1 LIMIT 1", [&r.name], |_| Ok(true))
                .unwrap_or(false);
            if !exists {
                let _ = tx.execute(
                    "INSERT INTO tag_rules (name, pattern, color, priority, expire_days) VALUES (?1,?2,?3,?4,?5)",
                    rusqlite::params![r.name, r.pattern, r.color, r.priority, r.expire_days],
                );
            }
        }

        // 2. 板：按名去重（内置板已由迁移保证，仅补非内置）
        for b in boards {
            if b.is_builtin {
                continue;
            }
            let exists: bool = tx
                .query_row("SELECT 1 FROM boards WHERE name = ?1 LIMIT 1", [&b.name], |_| Ok(true))
                .unwrap_or(false);
            if !exists {
                let _ = tx.execute(
                    "INSERT INTO boards (name, color, sort_order, is_builtin, created_at) VALUES (?1,?2,?3,0,?4)",
                    rusqlite::params![b.name, b.color, b.sort_order, now],
                );
            }
        }

        // 板名 -> id 映射
        let mut board_ids: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
        {
            let mut stmt = tx.prepare("SELECT name, id FROM boards").map_err(|e| e.to_string())?;
            let rows = stmt
                .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)))
                .map_err(|e| e.to_string())?;
            for r in rows {
                if let Ok((n, id)) = r {
                    board_ids.insert(n, id);
                }
            }
        }

        // 3. 条目：按 content_hash 去重
        let mut imported = 0usize;
        for it in items {
            let existing: Option<i64> = tx
                .query_row("SELECT id FROM clipboard_items WHERE content_hash = ?1", [&it.content_hash], |row| row.get(0))
                .ok();
            let item_id = if let Some(id) = existing {
                id
            } else {
                tx.execute(
                    "INSERT INTO clipboard_items (content_type, content, content_hash, blob_path, tag, created_at, updated_at, is_pinned, use_count, last_used_at, source_app, source_app_name)
                     VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
                    rusqlite::params![
                        it.content_type, it.content, it.content_hash, it.blob_path, it.tag,
                        it.created_at, it.updated_at, it.is_pinned as i32, it.use_count,
                        it.last_used_at, it.source_app, it.source_app_name
                    ],
                )
                .map_err(|e| e.to_string())?;
                imported += 1;
                tx.last_insert_rowid()
            };
            // 恢复板归属
            for bname in &it.boards {
                if let Some(&bid) = board_ids.get(bname) {
                    let _ = tx.execute(
                        "INSERT OR IGNORE INTO board_items (board_id, item_id, added_at) VALUES (?1,?2,?3)",
                        rusqlite::params![bid, item_id, now],
                    );
                }
            }
        }

        tx.commit().map_err(|e| e.to_string())?;
        Ok(imported)
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
                    "DELETE FROM clipboard_items WHERE tag = ?1 AND id NOT IN (SELECT item_id FROM board_items) AND updated_at < ?2",
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
                            "DELETE FROM clipboard_items WHERE tag IS NULL AND content_type = ?1 AND id NOT IN (SELECT item_id FROM board_items) AND updated_at < ?2",
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
                        "DELETE FROM clipboard_items WHERE tag IS NULL AND id NOT IN (SELECT item_id FROM board_items) AND updated_at < ?1",
                        rusqlite::params![cutoff],
                    )
                    .map_err(|e| e.to_string())?;
                total_deleted += deleted as u64;
            } else {
                // 只清理未单独配置的类型
                let placeholders: Vec<String> = configured_types.iter().enumerate().map(|(i, _)| format!("?{}", i + 2)).collect();
                let sql = format!(
                    "DELETE FROM clipboard_items WHERE tag IS NULL AND content_type NOT IN ({}) AND id NOT IN (SELECT item_id FROM board_items) AND updated_at < ?1",
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
            .prepare(&format!(
                "SELECT {} FROM clipboard_items WHERE tag = ?1 ORDER BY updated_at DESC LIMIT ?2",
                Self::ITEM_COLS
            ))
            .map_err(|e| e.to_string())?;

        let items = stmt
            .query_map(rusqlite::params![tag, limit], Self::map_item)
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();

        Ok(items)
    }

    // ========== 收藏板（Boards）相关方法 ==========

    /// 列出所有板（内置在前，同级按 sort_order）
    pub fn list_boards(&self) -> Result<Vec<Board>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare(
                "SELECT id, name, color, sort_order, is_builtin FROM boards
                 ORDER BY is_builtin DESC, sort_order ASC, id ASC",
            )
            .map_err(|e| e.to_string())?;
        let boards = stmt
            .query_map([], |row| {
                Ok(Board {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    color: row.get(2)?,
                    sort_order: row.get(3)?,
                    is_builtin: row.get::<_, i32>(4)? == 1,
                })
            })
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();
        Ok(boards)
    }

    /// 新建板（非内置）
    pub fn create_board(&self, name: &str, color: &str) -> Result<Board, String> {
        let name = name.trim();
        if name.is_empty() {
            return Err("板名称不能为空".to_string());
        }
        let now = Utc::now().timestamp();
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        // 新板排在末尾
        let next_order: i64 = conn
            .query_row("SELECT COALESCE(MAX(sort_order), 0) + 1 FROM boards WHERE is_builtin = 0", [], |row| row.get(0))
            .unwrap_or(1);
        conn.execute(
            "INSERT INTO boards (name, color, sort_order, is_builtin, created_at) VALUES (?1, ?2, ?3, 0, ?4)",
            rusqlite::params![name, color, next_order, now],
        )
        .map_err(|e| e.to_string())?;
        let id = conn.last_insert_rowid();
        Ok(Board { id, name: name.to_string(), color: color.to_string(), sort_order: next_order, is_builtin: false })
    }

    /// 重命名板
    pub fn rename_board(&self, id: i64, name: &str) -> Result<(), String> {
        let name = name.trim();
        if name.is_empty() {
            return Err("板名称不能为空".to_string());
        }
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute("UPDATE boards SET name = ?1 WHERE id = ?2 AND is_builtin = 0", rusqlite::params![name, id])
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// 修改板颜色
    pub fn recolor_board(&self, id: i64, color: &str) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute("UPDATE boards SET color = ?1 WHERE id = ?2", rusqlite::params![color, id])
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// 删除板（不删条目；内置板不可删）
    pub fn delete_board(&self, id: i64) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute("DELETE FROM boards WHERE id = ?1 AND is_builtin = 0", [id])
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// 批量重排板顺序
    pub fn reorder_boards(&self, ordered_ids: &[i64]) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        for (idx, id) in ordered_ids.iter().enumerate() {
            conn.execute(
                "UPDATE boards SET sort_order = ?1 WHERE id = ?2 AND is_builtin = 0",
                rusqlite::params![idx as i64 + 1, id],
            )
            .map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    /// 将条目加入板；若为内置收藏板则同步 is_pinned
    pub fn add_item_to_board(&self, board_id: i64, item_id: i64) -> Result<(), String> {
        let now = Utc::now().timestamp();
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT OR IGNORE INTO board_items (board_id, item_id, added_at) VALUES (?1, ?2, ?3)",
            rusqlite::params![board_id, item_id, now],
        )
        .map_err(|e| e.to_string())?;
        if Self::favorites_board_id(&conn) == Some(board_id) {
            let _ = conn.execute("UPDATE clipboard_items SET is_pinned = 1 WHERE id = ?1", [item_id]);
        }
        Ok(())
    }

    /// 将条目移出板；若为内置收藏板则同步 is_pinned
    pub fn remove_item_from_board(&self, board_id: i64, item_id: i64) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "DELETE FROM board_items WHERE board_id = ?1 AND item_id = ?2",
            rusqlite::params![board_id, item_id],
        )
        .map_err(|e| e.to_string())?;
        if Self::favorites_board_id(&conn) == Some(board_id) {
            let _ = conn.execute("UPDATE clipboard_items SET is_pinned = 0 WHERE id = ?1", [item_id]);
        }
        Ok(())
    }

    /// 获取某条目所属的所有板 id
    pub fn get_board_ids_for_item(&self, item_id: i64) -> Result<Vec<i64>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare("SELECT board_id FROM board_items WHERE item_id = ?1")
            .map_err(|e| e.to_string())?;
        let ids = stmt
            .query_map([item_id], |row| row.get(0))
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();
        Ok(ids)
    }

    /// 获取板内条目（按使用时间倒序）
    pub fn get_items_in_board(&self, board_id: i64, limit: u32) -> Result<Vec<ClipboardItem>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        // board_items 与 clipboard_items 无同名列，列名无需表前缀
        let mut stmt = conn
            .prepare(&format!(
                "SELECT {} FROM clipboard_items c
                 JOIN board_items b ON b.item_id = c.id
                 WHERE b.board_id = ?1
                 ORDER BY c.updated_at DESC
                 LIMIT ?2",
                Self::ITEM_COLS
            ))
            .map_err(|e| e.to_string())?;
        let items = stmt
            .query_map(rusqlite::params![board_id, limit], Self::map_item)
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();
        Ok(items)
    }
}

// ========== 内置检测器（标签规则 pattern = "builtin:xxx" 时走这里，比正则更准） ==========

/// 按标识分发到对应检测器，未知标识返回 false
fn builtin_detect(kind: &str, content: &str) -> bool {
    match kind {
        // JSON：真正的语法校验，且限定对象/数组（裸数字、字符串也是合法 JSON，但打标无意义）
        "json" => {
            (content.starts_with('{') || content.starts_with('['))
                && serde_json::from_str::<serde_json::Value>(content).is_ok()
        }
        // URL：标准解析器校验，仅限 http/https 且单行
        "url" => {
            !content.contains(char::is_whitespace)
                && url::Url::parse(content)
                    .map(|u| matches!(u.scheme(), "http" | "https"))
                    .unwrap_or(false)
        }
        "bankcard" => luhn_check(content),
        "idcard" => idcard_check(content),
        _ => false,
    }
}

/// Luhn 校验：银行卡号（12-19 位数字，允许空格/连字符分隔）
fn luhn_check(s: &str) -> bool {
    let cleaned: String = s.chars().filter(|c| !c.is_whitespace() && *c != '-').collect();
    if cleaned.len() < 12 || cleaned.len() > 19 || !cleaned.chars().all(|c| c.is_ascii_digit()) {
        return false;
    }
    let sum: u32 = cleaned
        .chars()
        .rev()
        .enumerate()
        .map(|(i, c)| {
            let mut d = c.to_digit(10).unwrap();
            if i % 2 == 1 {
                d *= 2;
                if d > 9 {
                    d -= 9;
                }
            }
            d
        })
        .sum();
    sum % 10 == 0
}

/// 身份证校验：18 位，末位为加权校验码（GB 11643）
fn idcard_check(s: &str) -> bool {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() != 18 || !chars[..17].iter().all(|c| c.is_ascii_digit()) {
        return false;
    }
    const WEIGHTS: [u32; 17] = [7, 9, 10, 5, 8, 4, 2, 1, 6, 3, 7, 9, 10, 5, 8, 4, 2];
    const CODES: [char; 11] = ['1', '0', 'X', '9', '8', '7', '6', '5', '4', '3', '2'];
    let sum: u32 = chars[..17]
        .iter()
        .zip(WEIGHTS)
        .map(|(c, w)| c.to_digit(10).unwrap() * w)
        .sum();
    chars[17].to_ascii_uppercase() == CODES[(sum % 11) as usize]
}
