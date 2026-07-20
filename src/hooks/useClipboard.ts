import { invoke } from "@tauri-apps/api/core";
import { ClipboardItemData } from "../components/ClipboardItem";

export interface TagRule {
  id: number;
  name: string;
  pattern: string;
  color: string;
  priority: number;
  expire_days: number;
}

export interface FilterTag {
  name: string;       // 显示名称，如"图片"、"代码"
  type: "content_type" | "rule" | "board";  // 标签来源
  value: string;      // 筛选值：content_type 时为 "image"；rule 时为 tag name；board 时为 "board:{id}"
  color: string;      // 标签颜色
  builtin?: boolean;  // board 类型：是否内置板（内置不可重命名/删除）
}

/// 收藏板数据结构
export interface Board {
  id: number;
  name: string;
  color: string;
  sort_order: number;
  is_builtin: boolean;
}

export async function getClipboardItems(limit?: number, offset?: number): Promise<ClipboardItemData[]> {
  return invoke("get_clipboard_items", { limit: limit || 50, offset: offset || 0 });
}

export async function countItems(): Promise<number> {
  return invoke("count_items");
}

export async function getPinnedItems(limit?: number): Promise<ClipboardItemData[]> {
  return invoke("get_pinned_items", { limit: limit || 200 });
}

// ========== 收藏板（Boards） ==========

export async function listBoards(): Promise<Board[]> {
  return invoke("list_boards");
}

export async function createBoard(name: string, color: string): Promise<Board> {
  return invoke("create_board", { name, color });
}

export async function renameBoard(id: number, name: string): Promise<void> {
  return invoke("rename_board", { id, name });
}

export async function recolorBoard(id: number, color: string): Promise<void> {
  return invoke("recolor_board", { id, color });
}

export async function deleteBoard(id: number): Promise<void> {
  return invoke("delete_board", { id });
}

export async function addItemToBoard(boardId: number, itemId: number): Promise<void> {
  return invoke("add_item_to_board", { boardId, itemId });
}

export async function removeItemFromBoard(boardId: number, itemId: number): Promise<void> {
  return invoke("remove_item_from_board", { boardId, itemId });
}

export async function getBoardIdsForItem(itemId: number): Promise<number[]> {
  return invoke("get_board_ids_for_item", { itemId });
}

export async function getItemsInBoard(boardId: number, limit?: number): Promise<ClipboardItemData[]> {
  return invoke("get_items_in_board", { boardId, limit: limit || 200 });
}

export async function searchClipboard(keyword: string): Promise<ClipboardItemData[]> {
  return invoke("search_clipboard", { keyword });
}

export async function togglePinItem(id: number): Promise<boolean> {
  return invoke("toggle_pin_item", { id });
}

export async function deleteClipboardItem(id: number): Promise<void> {
  return invoke("delete_clipboard_item", { id });
}

export async function pasteItem(id: number): Promise<void> {
  return invoke("paste_item", { id });
}

export async function getBlobsDir(): Promise<string> {
  return invoke("get_blobs_dir");
}

export async function getShortcut(): Promise<string> {
  return invoke("get_shortcut");
}

export async function setShortcut(shortcut: string): Promise<void> {
  return invoke("set_shortcut", { shortcut });
}

// ========== 标签规则 API ==========

export async function getTagRules(): Promise<TagRule[]> {
  return invoke("get_tag_rules");
}

export async function addTagRule(name: string, pattern: string, color: string, priority: number, expire_days: number = 0): Promise<TagRule> {
  return invoke("add_tag_rule", { name, pattern, color, priority, expireDays: expire_days });
}

export async function deleteTagRule(id: number): Promise<void> {
  return invoke("delete_tag_rule", { id });
}

export async function updateTagRuleExpire(id: number, expireDays: number): Promise<void> {
  return invoke("update_tag_rule_expire", { id, expireDays });
}

export async function getDefaultExpireDays(): Promise<number> {
  return invoke("get_default_expire_days");
}

export async function setDefaultExpireDays(days: number): Promise<void> {
  return invoke("set_default_expire_days", { days });
}

export async function getMaxItems(): Promise<number> {
  return invoke("get_max_items");
}

export async function setMaxItems(count: number): Promise<void> {
  return invoke("set_max_items", { count });
}

export async function getContentTypeExpireDays(contentType: string): Promise<number> {
  return invoke("get_content_type_expire_days", { contentType });
}

export async function setContentTypeExpireDays(contentType: string, days: number): Promise<void> {
  return invoke("set_content_type_expire_days", { contentType, days });
}

export async function getItemsByTag(tag: string, limit?: number): Promise<ClipboardItemData[]> {
  return invoke("get_items_by_tag", { tag, limit: limit || 50 });
}

export async function setItemTag(id: number, tag: string): Promise<void> {
  return invoke("set_item_tag", { id, tag });
}

// ========== 排除应用 API ==========

export interface RunningApp {
  bundle_id: string;
  name: string;
}

export async function getExcludedApps(): Promise<string[]> {
  return invoke("get_excluded_apps");
}

export async function addExcludedApp(bundleId: string, appName: string): Promise<void> {
  return invoke("add_excluded_app", { bundleId, appName });
}

export async function removeExcludedApp(bundleId: string): Promise<void> {
  return invoke("remove_excluded_app", { bundleId });
}

export async function getRunningApps(): Promise<RunningApp[]> {
  return invoke("get_running_apps");
}

export async function getExcludedAppsNames(): Promise<Record<string, string>> {
  return invoke("get_excluded_apps_names");
}
