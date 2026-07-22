import { Component, Show, For, createSignal, createMemo } from "solid-js";
import { convertFileSrc } from "@tauri-apps/api/core";
import { TagRule, Board, getBoardIdsForItem } from "../hooks/useClipboard";
import { detectKind, extractDomain } from "../contentKind";

export interface ClipboardItemData {
  id: number;
  content_type: string;
  content: string;
  content_hash: string;
  blob_path: string | null;
  created_at: number;
  updated_at: number;
  is_pinned: boolean;
  tag: string | null;
  use_count: number;
  last_used_at: number | null;
  source_app: string | null;
  source_app_name: string | null;
}

interface ClipboardItemProps {
  item: ClipboardItemData;
  isSelected: boolean;
  blobsDir: string;
  tagRules: TagRule[];
  boards: Board[];
  onPaste: (id: number) => void;
  onTogglePin: (id: number) => void;
  onDelete: (id: number) => void;
  onSetTag: (id: number, tag: string) => void;
  onToggleBoard: (itemId: number, boardId: number, add: boolean) => void;
  onEdit: (id: number) => void;
}

const ClipboardItem: Component<ClipboardItemProps> = (props) => {
  const [showTagMenu, setShowTagMenu] = createSignal(false);
  const [showBoardMenu, setShowBoardMenu] = createSignal(false);
  const [memberBoardIds, setMemberBoardIds] = createSignal<number[]>([]);

  // 星标即归板入口：只有收藏夹一个板时直接切收藏；多板时弹选择菜单
  const handleStarClick = (e: MouseEvent) => {
    e.stopPropagation();
    if (props.boards.length <= 1) {
      props.onTogglePin(props.item.id);
      setShowBoardMenu(false);
    } else {
      toggleBoardMenu();
    }
  };

  // 打开板菜单时懒加载成员状态
  const toggleBoardMenu = async () => {
    const next = !showBoardMenu();
    setShowBoardMenu(next);
    setShowTagMenu(false);
    if (next) {
      try {
        setMemberBoardIds(await getBoardIdsForItem(props.item.id));
      } catch (e) {
        console.error("Failed to load board membership:", e);
      }
    }
  };

  const CONTENT_TYPE_MAP: Record<string, { name: string; color: string }> = {
    text: { name: "文字", color: "#50d0a0" },
    image: { name: "图片", color: "#7c6df0" },
  };

  const getContentTypeColor = () => {
    return CONTENT_TYPE_MAP[props.item.content_type]?.color || "#a09fbd";
  };

  // 头部标题：有标签显标签名，否则显内容类型名（仃 Paste 分类展示）
  const headerTitle = () => props.item.tag ?? (CONTENT_TYPE_MAP[props.item.content_type]?.name || "文字");

  const getTagColor = () => {
    if (!props.item.tag) return "";
    const rule = props.tagRules.find((r) => r.name === props.item.tag);
    return rule?.color || "#7c6df0";
  };

  // Header 差异化配色：仿 Paste 风格，亮色饱和背景 + 白色文字
  const HEADER_PALETTES = [
    { from: "#2563eb", to: "#3b82f6" },  // 蓝
    { from: "#7c3aed", to: "#8b5cf6" },  // 紫
    { from: "#0891b2", to: "#06b6d4" },  // 青
    { from: "#059669", to: "#10b981" },  // 绿
    { from: "#d97706", to: "#f59e0b" },  // 橙
    { from: "#dc2626", to: "#ef4444" },  // 红
    { from: "#7c3aed", to: "#a855f7" },  // 深紫
    { from: "#0d9488", to: "#14b8a6" },  // 鸦绿
  ];

  const hexToHsl = (hex: string) => {
    let r = parseInt(hex.slice(1, 3), 16) / 255;
    let g = parseInt(hex.slice(3, 5), 16) / 255;
    let b = parseInt(hex.slice(5, 7), 16) / 255;
    const max = Math.max(r, g, b), min = Math.min(r, g, b);
    let h = 0, s = 0, l = (max + min) / 2;
    if (max !== min) {
      const d = max - min;
      s = l > 0.5 ? d / (2 - max - min) : d / (max + min);
      if (max === r) h = ((g - b) / d + (g < b ? 6 : 0)) / 6;
      else if (max === g) h = ((b - r) / d + 2) / 6;
      else h = ((r - g) / d + 4) / 6;
    }
    return { h: Math.round(h * 360), s: Math.round(s * 100), l: Math.round(l * 100) };
  };

  const getHeaderGradient = () => {
    // 收藏卡片用暖橙色
    if (props.item.is_pinned) return "linear-gradient(135deg, #d97706 0%, #f59e0b 100%)";
  
    // 有标签 → 基于标签颜色生成亮色渐变
    if (props.item.tag) {
      const color = getTagColor();
      if (color) {
        const { h, s } = hexToHsl(color);
        return `linear-gradient(135deg, hsl(${h}, ${Math.max(s, 60)}%, 42%) 0%, hsl(${h}, ${Math.max(s, 55)}%, 52%) 100%)`;
      }
    }
  
    // 图片类型 → 青色
    if (isImage()) {
      return "linear-gradient(135deg, #0891b2 0%, #06b6d4 100%)";
    }
  
    // 颜色值卡片 → header 直接用该颜色（hex 时限制亮度保证白字可读，其他格式走默认色板）
    if (kind() === "color") {
      const val = props.item.content.trim();
      if (val.startsWith("#") && (val.length === 7 || val.length === 9)) {
        const { h, s, l } = hexToHsl(val);
        return `linear-gradient(135deg, hsl(${h}, ${s}%, ${Math.min(l, 55)}%) 0%, hsl(${h}, ${s}%, ${Math.min(l + 8, 62)}%) 100%)`;
      }
    }

    // 纯文字 → 根据 content_hash 轮选色板
    const hash = props.item.content_hash || props.item.content;
    let hashNum = 0;
    for (let i = 0; i < Math.min(hash.length, 8); i++) {
      hashNum += hash.charCodeAt(i);
    }
    const palette = HEADER_PALETTES[hashNum % HEADER_PALETTES.length];
    return `linear-gradient(135deg, ${palette.from} 0%, ${palette.to} 100%)`;
  };

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
    return `${text.length} 个字符`;
  };

  const isImage = () => props.item.content_type === "image";

  // 内容形态（颜色/链接/普通文本）：渲染时现算，content 不可变故 memo 一次
  const kind = createMemo(() => (isImage() ? "text" : detectKind(props.item.content)));

  const thumbSrc = () => {
    if (!isImage() || !props.item.blob_path || !props.blobsDir) return "";
    const thumbName = props.item.blob_path.replace(".png", "_thumb.png");
    // Windows 路径用反斜杠，需统一为正斜杠后再传给 convertFileSrc
    const fullPath = `${props.blobsDir}/${thumbName}`.replace(/\\/g, "/");
    return convertFileSrc(fullPath);
  };

  return (
    <div
      class={`clipboard-item ${props.isSelected ? "selected" : ""} ${props.item.is_pinned ? "pinned" : ""}`}
      onClick={() => props.onPaste(props.item.id)}
    >
      {/* Header: 类型/标签在上、时间在下两行（仃 Paste） */}
      <div class="item-header" style={{ background: getHeaderGradient() }}>
        <div class="header-left">
          <div class="header-title-row">
            <Show when={props.item.tag}>
              <span class="header-tag-dot" style={{ background: getTagColor() }}></span>
            </Show>
            <span class="header-title">{headerTitle()}</span>
          </div>
          <span class="header-time">{formatTime(props.item.updated_at)}</span>
        </div>
        <div class="header-right">
          <Show when={props.item.is_pinned}>
            <span class="header-pin-icon" title="已收藏">★</span>
          </Show>
        </div>
      </div>

      {/* Content */}
      <div class="item-content">
        <Show when={isImage()} fallback={
          <Show when={kind() === "color"} fallback={
            <Show when={kind() === "url"} fallback={<pre>{truncate(props.item.content)}</pre>}>
              <div class="item-url">
                <span class="url-domain">{extractDomain(props.item.content)}</span>
                <span class="url-full">{truncate(props.item.content.trim(), 90)}</span>
              </div>
            </Show>
          }>
            <div class="item-color">
              <div class="item-color-swatch" style={{ background: props.item.content.trim() }}></div>
              <span class="color-value">{props.item.content.trim()}</span>
            </div>
          </Show>
        }>
          <div class="item-image">
            <img src={thumbSrc()} alt="clipboard image" loading="lazy" />
          </div>
        </Show>
      </div>

      {/* Footer: 左侧信息 + 右侧操作按钮 */}
      <div class="item-footer">
        <span class="item-info" title={props.item.source_app ?? undefined}>
          <Show when={props.item.source_app_name}>
            <span class="info-source">{props.item.source_app_name}</span>
            <span class="info-sep">·</span>
          </Show>
          <Show when={isImage()} fallback={charCount(props.item.content)}>
            <span class="info-type-dot" style={{ background: getContentTypeColor() }}></span>
            图片
          </Show>
          <Show when={props.item.use_count >= 2}>
            <span class="info-sep">·</span>
            <span class="info-use-count">用过 {props.item.use_count} 次</span>
          </Show>
        </span>
        <div class="item-actions">
          <Show when={!isImage()}>
            <button
              class="btn-action btn-edit"
              onClick={(e) => {
                e.stopPropagation();
                props.onEdit(props.item.id);
              }}
              title="编辑"
            >
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7" />
                <path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z" />
              </svg>
            </button>
          </Show>
          <div class="tag-action-wrapper">
            <button
              class="btn-action btn-tag"
              onClick={(e) => {
                e.stopPropagation();
                setShowTagMenu(!showTagMenu());
                setShowBoardMenu(false);
              }}
              title="打标签"
            >
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M20.59 13.41l-7.17 7.17a2 2 0 0 1-2.83 0L2 12V2h10l8.59 8.59a2 2 0 0 1 0 2.82z" />
                <line x1="7" y1="7" x2="7.01" y2="7" />
              </svg>
            </button>
            <Show when={showTagMenu()}>
              <div class="tag-menu">
                <Show when={props.tagRules.length === 0}>
                  <div class="tag-menu-empty">请先在设置中添加标签规则</div>
                </Show>
                <For each={props.tagRules}>
                  {(rule) => (
                    <button
                      class="tag-menu-item"
                      onClick={(e) => {
                        e.stopPropagation();
                        props.onSetTag(props.item.id, rule.name);
                        setShowTagMenu(false);
                      }}
                    >
                      <span class="tag-dot" style={{ background: rule.color }}></span>
                      {rule.name}
                    </button>
                  )}
                </For>
                <Show when={props.item.tag}>
                  <button
                    class="tag-menu-item tag-menu-clear"
                    onClick={(e) => {
                      e.stopPropagation();
                      props.onSetTag(props.item.id, "");
                      setShowTagMenu(false);
                    }}
                  >
                    清除标签
                  </button>
                </Show>
              </div>
            </Show>
          </div>
          <div class="tag-action-wrapper">
            <button
              class={`btn-action btn-pin ${props.item.is_pinned ? "active" : ""}`}
              onClick={handleStarClick}
              title={
                props.boards.length > 1
                  ? "加入板"
                  : props.item.is_pinned
                    ? "取消收藏"
                    : "收藏"
              }
            >
              <svg viewBox="0 0 24 24" fill={props.item.is_pinned ? "currentColor" : "none"} stroke="currentColor" stroke-width="2">
                <polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2" />
              </svg>
            </button>
            <Show when={showBoardMenu()}>
              <div class="tag-menu">
                <For each={props.boards}>
                  {(board) => {
                    const isMember = () => memberBoardIds().includes(board.id);
                    return (
                      <button
                        class="tag-menu-item"
                        onClick={(e) => {
                          e.stopPropagation();
                          const add = !isMember();
                          props.onToggleBoard(props.item.id, board.id, add);
                          setMemberBoardIds((prev) =>
                            add ? [...prev, board.id] : prev.filter((id) => id !== board.id)
                          );
                        }}
                      >
                        <span class="tag-dot" style={{ background: board.color }}></span>
                        {board.name}
                        <Show when={isMember()}>
                          <span class="board-menu-check">✓</span>
                        </Show>
                      </button>
                    );
                  }}
                </For>
              </div>
            </Show>
          </div>
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
