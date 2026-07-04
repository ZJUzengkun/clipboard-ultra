import { Component, For, Show } from "solid-js";
import ClipboardItem, { ClipboardItemData } from "./ClipboardItem";

interface ClipboardListProps {
  items: ClipboardItemData[];
  selectedIndex: number;
  onPaste: (id: number) => void;
  onTogglePin: (id: number) => void;
  onDelete: (id: number) => void;
}

const ClipboardList: Component<ClipboardListProps> = (props) => {
  return (
    <div class="clipboard-list">
      <Show
        when={props.items.length > 0}
        fallback={
          <div class="empty-state">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
              <path d="M9 5H7a2 2 0 0 0-2 2v12a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V7a2 2 0 0 0-2-2h-2" />
              <rect x="9" y="3" width="6" height="4" rx="1" />
            </svg>
            <p>暂无剪贴板记录</p>
            <span>复制文本后将自动出现在这里</span>
          </div>
        }
      >
        <For each={props.items}>
          {(item, index) => (
            <ClipboardItem
              item={item}
              isSelected={index() === props.selectedIndex}
              onPaste={props.onPaste}
              onTogglePin={props.onTogglePin}
              onDelete={props.onDelete}
            />
          )}
        </For>
      </Show>
    </div>
  );
};

export default ClipboardList;
