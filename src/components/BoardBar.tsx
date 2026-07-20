import { Component, For, Show, createSignal, onMount, onCleanup } from "solid-js";
import { Board } from "../hooks/useClipboard";

interface BoardBarProps {
  boards: Board[];
  activeTag: string;
  onSelectTag: (value: string) => void;
  onCreateBoard: (name: string) => void;
  onRenameBoard: (boardId: number, name: string) => void;
  onRecolorBoard: (boardId: number, color: string) => void;
  onDeleteBoard: (boardId: number) => void;
}

// 板色盘（与 App 新建板色盘一致）
const BOARD_COLORS = ["#f5c518", "#5aa9f0", "#f06d6d", "#50d0a0", "#f0a05a", "#7c6df0", "#e267c8"];

const BoardBar: Component<BoardBarProps> = (props) => {
  const [creating, setCreating] = createSignal(false);
  const [newName, setNewName] = createSignal("");
  let inputRef: HTMLInputElement | undefined;

  // 板右键菜单状态
  const [menuBoard, setMenuBoard] = createSignal<Board | null>(null);
  const [menuPos, setMenuPos] = createSignal({ x: 0, y: 0 });
  const [renaming, setRenaming] = createSignal(false);
  const [renameValue, setRenameValue] = createSignal("");
  const [confirmDelete, setConfirmDelete] = createSignal(false);
  let renameRef: HTMLInputElement | undefined;

  // "剪贴板"页签在非板视图时保持高亮（标签筛选也是在全历史上）
  const isClipboardActive = () => !props.activeTag.startsWith("board:");

  const closeMenu = () => {
    setMenuBoard(null);
    setRenaming(false);
    setConfirmDelete(false);
  };

  onMount(() => {
    document.addEventListener("click", closeMenu);
    onCleanup(() => document.removeEventListener("click", closeMenu));
  });

  const openBoardMenu = (e: MouseEvent, board: Board) => {
    e.preventDefault();
    e.stopPropagation();
    setRenaming(false);
    setConfirmDelete(false);
    setMenuPos({ x: e.clientX, y: e.clientY });
    setMenuBoard(board);
  };

  const startRename = () => {
    const board = menuBoard();
    if (!board) return;
    setRenameValue(board.name);
    setRenaming(true);
    requestAnimationFrame(() => renameRef?.focus());
  };

  const confirmRename = () => {
    const board = menuBoard();
    const name = renameValue().trim();
    if (board && name && name !== board.name) {
      props.onRenameBoard(board.id, name);
    }
    closeMenu();
  };

  const startCreate = () => {
    setNewName("");
    setCreating(true);
    requestAnimationFrame(() => inputRef?.focus());
  };

  const confirmCreate = () => {
    const name = newName().trim();
    if (name) props.onCreateBoard(name);
    setCreating(false);
  };

  // 阻断全局快捷键（Enter 粘贴 / Backspace 删除 / ESC 隐藏面板）
  const handleInputKeyDown = (e: KeyboardEvent) => {
    e.stopPropagation();
    if (e.key === "Enter") {
      confirmCreate();
    } else if (e.key === "Escape") {
      setCreating(false);
    }
  };

  const handleRenameKeyDown = (e: KeyboardEvent) => {
    e.stopPropagation();
    if (e.key === "Enter") {
      confirmRename();
    } else if (e.key === "Escape") {
      closeMenu();
    }
  };

  return (
    <div class="board-bar">
      <button
        class={`board-tab ${isClipboardActive() ? "active" : ""}`}
        onClick={() => props.onSelectTag("")}
      >
        剪贴板
      </button>
      <For each={props.boards}>
        {(board) => (
          <button
            class={`board-tab ${props.activeTag === `board:${board.id}` ? "active" : ""}`}
            onClick={() => props.onSelectTag(`board:${board.id}`)}
            onContextMenu={(e) => openBoardMenu(e, board)}
          >
            <span class="tag-dot" style={{ background: board.color }}></span>
            {board.name}
          </button>
        )}
      </For>
      <Show
        when={creating()}
        fallback={
          <button class="board-tab board-tab-add" onClick={startCreate} title="新建板">
            +
          </button>
        }
      >
        <input
          ref={inputRef}
          class="tag-new-input"
          type="text"
          placeholder="板名称"
          value={newName()}
          onInput={(e) => setNewName(e.currentTarget.value)}
          onKeyDown={handleInputKeyDown}
          onBlur={() => setCreating(false)}
        />
      </Show>

      {/* 板右键菜单 */}
      <Show when={menuBoard()}>
        <div
          class="board-ctx-menu"
          style={{ left: `${menuPos().x}px`, top: `${menuPos().y}px` }}
          onClick={(e) => e.stopPropagation()}
        >
          <Show
            when={renaming()}
            fallback={
              <>
                <Show when={!menuBoard()!.is_builtin}>
                  <button class="tag-menu-item" onClick={startRename}>
                    重命名
                  </button>
                </Show>
                <div class="board-color-row">
                  <For each={BOARD_COLORS}>
                    {(color) => (
                      <button
                        class={`board-color-swatch ${menuBoard()!.color === color ? "active" : ""}`}
                        style={{ background: color }}
                        onClick={() => {
                          props.onRecolorBoard(menuBoard()!.id, color);
                          closeMenu();
                        }}
                      />
                    )}
                  </For>
                </div>
                <Show when={!menuBoard()!.is_builtin}>
                  <button
                    class="tag-menu-item tag-menu-clear"
                    onClick={() => {
                      if (confirmDelete()) {
                        props.onDeleteBoard(menuBoard()!.id);
                        closeMenu();
                      } else {
                        setConfirmDelete(true);
                      }
                    }}
                  >
                    {confirmDelete() ? "确认删除？（条目保留）" : "删除板"}
                  </button>
                </Show>
              </>
            }
          >
            <input
              ref={renameRef}
              class="tag-new-input"
              type="text"
              value={renameValue()}
              onInput={(e) => setRenameValue(e.currentTarget.value)}
              onKeyDown={handleRenameKeyDown}
              onBlur={closeMenu}
            />
          </Show>
        </div>
      </Show>
    </div>
  );
};

export default BoardBar;
