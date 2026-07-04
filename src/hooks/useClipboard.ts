import { invoke } from "@tauri-apps/api/core";
import { ClipboardItemData } from "../components/ClipboardItem";

export async function getClipboardItems(limit?: number): Promise<ClipboardItemData[]> {
  return invoke("get_clipboard_items", { limit: limit || 50 });
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
