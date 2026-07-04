import { Component, Show } from "solid-js";

export interface ClipboardItemData {
  id: number;
  content_type: string;
  content: string;
  content_hash: string;
  created_at: number;
  updated_at: number;
  is_pinned: boolean;
}

interface ClipboardItemProps {
  item: ClipboardItemData;
  isSelected: boolean;
  onPaste: (id: number) => void;
  onTogglePin: (id: number) => void;
  onDelete: (id: number) => void;
}

const ClipboardItem: Component<ClipboardItemProps> = (props) => {
  const formatTime = (timestamp: number) => {
    const date = new Date(timestamp * 1000);
    const now = new Date();
    const diff = now.getTime() - date.getTime();
    const minutes = Math.floor(diff / 60000);
    if (minutes < 1) return "刚刚";
    if (minutes < 60) return `${minutes} 分钟前`;
    const hours = Math.floor(minutes / 60);
    if (hours < 24) return `${hours} 小时前`;
    const days = Math.floor(hours / 24);
    if (days < 7) return `${days} 天前`;
    return date.toLocaleDateString("zh-CN", { month: "short", day: "numeric" });
  };

  const truncate = (text: string, maxLen: number = 120) => {
    if (text.length <= maxLen) return text;
    return text.slice(0, maxLen) + "…";
  };

  const charCount = (text: string) => {
    if (text.length < 50) return "";
    return `${text.length} 字符`;
  };

  return (
    <div
      class={`clipboard-item ${props.isSelected ? "selected" : ""} ${props.item.is_pinned ? "pinned" : ""}`}
      onClick={() => props.onPaste(props.item.id)}
    >
      <div class="item-content">
        <pre>{truncate(props.item.content)}</pre>
      </div>
      <div class="item-footer">
        <div class="item-meta">
          <span class="item-time">{formatTime(props.item.updated_at)}</span>
          <Show when={charCount(props.item.content)}>
            <span class="item-chars">{charCount(props.item.content)}</span>
          </Show>
        </div>
        <div class="item-actions">
          <button
            class={`btn-action btn-pin ${props.item.is_pinned ? "active" : ""}`}
            onClick={(e) => {
              e.stopPropagation();
              props.onTogglePin(props.item.id);
            }}
            title={props.item.is_pinned ? "取消收藏" : "收藏"}
          >
            <svg viewBox="0 0 24 24" fill={props.item.is_pinned ? "currentColor" : "none"} stroke="currentColor" stroke-width="2">
              <polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2" />
            </svg>
          </button>
          <button
            class="btn-action btn-delete"
            onClick={(e) => {
              e.stopPropagation();
              props.onDelete(props.item.id);
            }}
            title="删除"
          >
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M3 6h18M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" />
            </svg>
          </button>
        </div>
      </div>
    </div>
  );
};

export default ClipboardItem;
