import { Component, For, Show, onMount, onCleanup } from "solid-js";
import ClipboardItem, { ClipboardItemData } from "./ClipboardItem";
import { TagRule, Board } from "../hooks/useClipboard";

interface ClipboardListProps {
  items: ClipboardItemData[];
  selectedIndex: number;
  blobsDir: string;
  tagRules: TagRule[];
  boards: Board[];
  onPaste: (id: number) => void;
  onTogglePin: (id: number) => void;
  onDelete: (id: number) => void;
  onSetTag: (id: number, tag: string) => void;
  onToggleBoard: (itemId: number, boardId: number, add: boolean) => void;
  hasMore?: boolean;
  onLoadMore?: () => void;
}

const ClipboardList: Component<ClipboardListProps> = (props) => {
  let listRef: HTMLDivElement | undefined;

  onMount(() => {
    if (!listRef) return;
    const onScroll = () => {
      if (!props.hasMore) return;
      // 横向列表：滚到接近右端（预留 300px）时加载下一页
      if (listRef!.scrollLeft + listRef!.clientWidth >= listRef!.scrollWidth - 300) {
        props.onLoadMore?.();
      }
    };
    listRef.addEventListener("scroll", onScroll, { passive: true });
    onCleanup(() => listRef?.removeEventListener("scroll", onScroll));
  });

  return (
    <div class="clipboard-list" ref={listRef}>
      <Show
        when={props.items.length > 0}
        fallback={
          <div class="empty-state">
            <div class="empty-icon">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
                <path d="M9 5H7a2 2 0 0 0-2 2v12a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V7a2 2 0 0 0-2-2h-2" />
                <rect x="9" y="3" width="6" height="4" rx="1" />
              </svg>
            </div>
            <p>暂无剪贴板记录</p>
            <span>复制文本后将自动出现在这里</span>
            <div class="empty-shortcut">
              <span class="kbd">Ctrl</span>+<span class="kbd">Shift</span>+<span class="kbd">V</span>
              <span style="margin-left: 4px">唤起窗口</span>
            </div>
          </div>
        }
      >
        <For each={props.items}>
          {(item, index) => (
            <ClipboardItem
              item={item}
              isSelected={index() === props.selectedIndex}
              blobsDir={props.blobsDir}
              tagRules={props.tagRules}
              boards={props.boards}
              onPaste={props.onPaste}
              onTogglePin={props.onTogglePin}
              onDelete={props.onDelete}
              onSetTag={props.onSetTag}
              onToggleBoard={props.onToggleBoard}
            />
          )}
        </For>
      </Show>
    </div>
  );
};

export default ClipboardList;
