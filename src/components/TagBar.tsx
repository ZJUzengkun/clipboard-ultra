import { Component, For, Show, createSignal, onMount, onCleanup } from "solid-js";
import { FilterTag } from "../hooks/useClipboard";

interface TagBarProps {
  tags: FilterTag[];
  activeTag: string;
  onSelectTag: (value: string) => void;
  /// 一键清空某标签下全部记录（收藏板成员保留），由父组件执行并刷新列表
  onClearTag: (tag: FilterTag) => void;
}

const TagBar: Component<TagBarProps> = (props) => {
  // 标签右键菜单：目标标签 + 位置 + 二段确认态（仿 BoardBar 删除板交互）
  const [menuTag, setMenuTag] = createSignal<FilterTag | null>(null);
  const [menuPos, setMenuPos] = createSignal({ x: 0, y: 0 });
  const [confirmClear, setConfirmClear] = createSignal(false);

  const closeMenu = () => {
    setMenuTag(null);
    setConfirmClear(false);
  };

  // 点击菜单外部才关闭：document 级 click 监听不受 stopPropagation 影响（同节点监听器仍会执行），
  // 若不做目标判断，菜单内的第一次确认点击会被误关
  const handleDocClick = (e: MouseEvent) => {
    if ((e.target as HTMLElement | null)?.closest(".board-ctx-menu")) return;
    closeMenu();
  };

  onMount(() => {
    document.addEventListener("click", handleDocClick);
    onCleanup(() => document.removeEventListener("click", handleDocClick));
  });

  const openTagMenu = (e: MouseEvent, tag: FilterTag) => {
    e.preventDefault();
    e.stopPropagation();
    setMenuPos({ x: e.clientX, y: e.clientY });
    setConfirmClear(false);
    setMenuTag(tag);
  };

  return (
    <div class="tag-bar">
      <For each={props.tags}>
        {(tag) => (
          <button
            class={`tag-chip ${props.activeTag === tag.value ? "active" : ""}`}
            onClick={() =>
              // 再点一次已选标签 = 取消筛选，回到当前板视图
              props.onSelectTag(props.activeTag === tag.value ? "" : tag.value)
            }
            onContextMenu={(e) => openTagMenu(e, tag)}
            style={{ "--chip-color": tag.color }}
          >
            <span class="tag-dot" style={{ background: tag.color }}></span>
            {tag.name}
          </button>
        )}
      </For>

      {/* 标签右键菜单：一键清空该标签下全部记录 */}
      <Show when={menuTag()}>
        <div
          class="board-ctx-menu"
          style={{ left: `${menuPos().x}px`, top: `${menuPos().y}px` }}
          onClick={(e) => e.stopPropagation()}
        >
          <button
            class="tag-menu-item tag-menu-clear"
            onClick={() => {
              if (confirmClear()) {
                props.onClearTag(menuTag()!);
                closeMenu();
              } else {
                setConfirmClear(true);
              }
            }}
          >
            {confirmClear() ? "确认清空？（收藏板内保留）" : `清空「${menuTag()!.name}」全部记录`}
          </button>
        </div>
      </Show>
    </div>
  );
};

export default TagBar;
