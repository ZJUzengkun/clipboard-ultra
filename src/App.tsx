import { createSignal, onMount, onCleanup, Show } from "solid-js";
import SearchBar from "./components/SearchBar";
import ClipboardList from "./components/ClipboardList";
import TagBar from "./components/TagBar";
import BoardBar from "./components/BoardBar";
import { ClipboardItemData } from "./components/ClipboardItem";
import {
  getClipboardItems,
  countItems,
  searchClipboard,
  togglePinItem,
  deleteClipboardItem,
  pasteItem,
  getBlobsDir,
  getTagRules,
  getItemsByTag,
  setItemTag,
  updateItemContent,
  listBoards,
  createBoard,
  getItemsInBoard,
  addItemToBoard,
  removeItemFromBoard,
  renameBoard,
  recolorBoard,
  deleteBoard,
  TagRule,
  FilterTag,
  Board,
  checkAccessibility,
  openAccessibilitySettings,
  getAppConfig,
} from "./hooks/useClipboard";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { listen, emit } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { themeOf, getStoredChoice, resolveThemeId, applyChoice, saveChoice, lastThemeIdFor } from "./theme";

// 平台修饰键：macOS 只认 ⌘，Windows/Linux 只认 Ctrl（避免跨平台串键）
const IS_WINDOWS = navigator.userAgent.includes("Windows");
const MOD_KEY = IS_WINDOWS ? "Control" : "Meta";
const isModPressed = (e: KeyboardEvent) => (IS_WINDOWS ? e.ctrlKey : e.metaKey);

function App() {
  const [items, setItems] = createSignal<ClipboardItemData[]>([]);
  const [keyword, setKeyword] = createSignal("");
  const [selectedIndex, setSelectedIndex] = createSignal(0);
  const [blobsDir, setBlobsDir] = createSignal("");
  const [activeTag, setActiveTag] = createSignal("");
  const [tagRules, setTagRules] = createSignal<TagRule[]>([]);
  const [boards, setBoards] = createSignal<Board[]>([]);
  const [pendingDeletes, setPendingDeletes] = createSignal<ClipboardItemData[]>([]);
  const [ready, setReady] = createSignal(false);
  const [total, setTotal] = createSignal(0);
  const [hasMore, setHasMore] = createSignal(false);
  const [loadingMore, setLoadingMore] = createSignal(false);
  // 呼出时自动聚焦搜索框（设置→通用，默认开启）
  const [focusSearchOnShow, setFocusSearchOnShow] = createSignal(true);
  // 辅助功能权限缺失（macOS）：面板顶部展示授权引导条
  const [axMissing, setAxMissing] = createSignal(false);
  // 卡片内容编辑浮层：正在编辑的条目与草稿（仅文本类）
  const [editingItem, setEditingItem] = createSignal<ClipboardItemData | null>(null);
  const [editDraft, setEditDraft] = createSignal("");

  // 重新检测权限（用户授权后点"重新检测"收起引导条）
  const recheckAccessibility = async () => {
    try {
      setAxMissing(!(await checkAccessibility()));
    } catch (e) {
      console.error("Failed to check accessibility:", e);
    }
  };
  const PAGE_SIZE = 50;
  const [themeId, setThemeId] = createSignal(resolveThemeId(getStoredChoice()));
  const themeMode = () => themeOf(themeId()).mode;

  let refreshInterval: number;

  // 顶栏快切：在暗/亮两组的最近使用主题间切换
  const toggleTheme = () => {
    const next = lastThemeIdFor(themeMode() === "dark" ? "light" : "dark");
    setThemeId(saveChoice(next).id);
    emit("theme-changed", next);
  };

  const allTags = (): FilterTag[] => {
    // 标签行只放筛选标签，板页签独立在顶部 BoardBar
    const systemTags: FilterTag[] = [
      { name: "文字", type: "content_type", value: "text", color: "#50d0a0" },
      { name: "图片", type: "content_type", value: "image", color: "#7c6df0" },
    ];
    const ruleTags: FilterTag[] = tagRules().map((r) => ({
      name: r.name,
      type: "rule",
      value: r.name,
      color: r.color,
    }));
    return [...systemTags, ...ruleTags];
  };

  // 新建板预设色盘（按现有板数轮换）
  const BOARD_COLORS = ["#5aa9f0", "#f06d6d", "#50d0a0", "#f0a05a", "#7c6df0", "#e267c8"];

  const loadBoards = async () => {
    try {
      setBoards(await listBoards());
    } catch (e) {
      console.error("Failed to load boards:", e);
    }
  };

  const handleCreateBoard = async (name: string) => {
    try {
      const color = BOARD_COLORS[(boards().length - 1) % BOARD_COLORS.length];
      const board = await createBoard(name, color);
      await loadBoards();
      // 新建后直接切到该板
      handleSelectTag(`board:${board.id}`);
    } catch (e) {
      console.error("Failed to create board:", e);
    }
  };

  const handleRenameBoard = async (boardId: number, name: string) => {
    try {
      await renameBoard(boardId, name);
      await loadBoards();
    } catch (e) {
      console.error("Failed to rename board:", e);
    }
  };

  const handleRecolorBoard = async (boardId: number, color: string) => {
    try {
      await recolorBoard(boardId, color);
      await loadBoards();
    } catch (e) {
      console.error("Failed to recolor board:", e);
    }
  };

  const handleDeleteBoard = async (boardId: number) => {
    try {
      await deleteBoard(boardId);
      // 正在查看被删板时切回全部视图
      if (activeTag() === `board:${boardId}`) {
        setActiveTag("");
      }
      await loadBoards();
      await loadItems();
    } catch (e) {
      console.error("Failed to delete board:", e);
    }
  };

  const loadItems = async (markReady = true) => {
    try {
      const k = keyword();
      const tagValue = activeTag();
      let result: ClipboardItemData[];
      let paged = false;
      if (k.length > 0) {
        result = await searchClipboard(k);
      } else if (tagValue) {
        if (tagValue.startsWith("board:")) {
          // 板视图（板页签不在 allTags 中，直接按前缀判断）
          result = await getItemsInBoard(parseInt(tagValue.slice(6), 10), 200);
        } else {
          const filterTag = allTags().find((t) => t.value === tagValue);
          if (filterTag?.type === "content_type") {
            const all = await getClipboardItems(200);
            result = all.filter((item) => item.content_type === filterTag.value);
          } else {
            result = await getItemsByTag(tagValue, 50);
          }
        }
      } else {
        // 全部视图：分页加载首屏
        result = await getClipboardItems(PAGE_SIZE, 0);
        paged = true;
      }
      setItems(result);
      if (paged) {
        setHasMore(result.length === PAGE_SIZE);
        setTotal(await countItems());
      } else {
        setHasMore(false);
        setTotal(result.length);
      }
      // 数据就绪后触发入场（隐藏时后台预载不触发，留给唤起时播滑入动画）
      if (markReady) requestAnimationFrame(() => setReady(true));
    } catch (e) {
      console.error("Failed to load items:", e);
    }
  };

  // 滞动加载下一页（仅全部视图）
  const loadMore = async () => {
    if (loadingMore() || !hasMore()) return;
    if (keyword().length > 0 || activeTag()) return;
    setLoadingMore(true);
    try {
      // offset 计入待删项，避免软删除造成错位
      const offset = items().length + pendingDeletes().length;
      const next = await getClipboardItems(PAGE_SIZE, offset);
      if (next.length > 0) {
        const existing = new Set(items().map((i) => i.id));
        const merged = next.filter((i) => !existing.has(i.id));
        setItems((prev) => [...prev, ...merged]);
      }
      setHasMore(next.length === PAGE_SIZE);
    } catch (e) {
      console.error("Failed to load more:", e);
    } finally {
      setLoadingMore(false);
    }
  };

  const loadTagRules = async () => {
    try {
      const rules = await getTagRules();
      setTagRules(rules);
    } catch (e) {
      console.error("Failed to load tag rules:", e);
    }
  };

  onMount(() => {
    // 初始化主题
    setThemeId(applyChoice(getStoredChoice()).id);

    // 获取 blobs 目录路径
    getBlobsDir().then(setBlobsDir).catch(console.error);

    // 读取"呼出时自动聚焦搜索框"开关（默认开启）
    getAppConfig("focus_search_on_show")
      .then((v) => setFocusSearchOnShow(v !== "false"))
      .catch(console.error);

    // 检测辅助功能权限（macOS 自动粘贴依赖；未授权时面板顶部展示引导条）
    checkAccessibility()
      .then((ok) => setAxMissing(!ok))
      .catch(console.error);

    loadItems(false); // 启动时窗口隐藏，不点亮入场态，留给首次唤起播动画
    loadTagRules();
    loadBoards();

    // 监听后端剪贴板更新事件（替代轮询；隐藏时也会触发，不动入场态）
    const unlistenClipboard = listen("clipboard-updated", () => {
      loadItems(false);
    });

    // 监听设置窗口发来的标签规则变更事件
    const unlistenTagRules = listen("tag-rules-changed", () => {
      loadTagRules();
    });

    // 监听设置窗口发来的主题变更事件
    const unlistenTheme = listen<string>("theme-changed", (e) => {
      setThemeId(applyChoice(e.payload).id);
    });

    // 监听设置窗口发来的"自动聚焦搜索框"开关变更
    const unlistenFocusCfg = listen<boolean>("focus-search-changed", (e) => {
      setFocusSearchOnShow(e.payload);
    });

    // 监听热键再次触发的收起请求（Rust 侧不直接 hide，由前端播完滑出动画再隐藏）
    const unlistenDismiss = listen("panel-dismiss", () => {
      dismissPanel();
    });

    // 窗口获得焦点时只聚焦搜索框；视图已在隐藏时后台预备好，避免展示后重载闪烁
    const appWindow = getCurrentWindow();
    let showTimestamp = 0;

    const unlistenShow = appWindow.listen("tauri://focus", () => {
      showTimestamp = Date.now();
      // 播放从底部滑入的入场动画。用 WAAPI 而非 CSS transition：
      // 隐藏窗口恢复渲染时 transition 起始态从未呈现过，会被折叠成跳变；
      // WAAPI 动画的 startTime 在渲染恢复后才分配，保证完整播放
      if (!ready()) {
        requestAnimationFrame(() => {
          setReady(true);
          const app = document.querySelector(".app");
          if (app) {
            app.animate(
              [{ transform: "translateY(100%)" }, { transform: "translateY(0)" }],
              { duration: 200, easing: "cubic-bezier(0.32, 0.72, 0.28, 1)" }
            );
          }
        });
      }
      // 自动聚焦搜索框（可在设置→通用中关闭）
      if (focusSearchOnShow()) {
        requestAnimationFrame(() => {
          const input = document.querySelector(".search-bar input") as HTMLInputElement;
          if (input) input.focus();
        });
      }
    });

    const unlisten = appWindow.onFocusChanged(({ payload: focused }) => {
      if (!focused) {
        // 如果窗口刚获得焦点不到 300ms，不要隐藏（防止闪屏）
        const elapsed = Date.now() - showTimestamp;
        if (elapsed > 300) {
          dismissPanel();
        }
      }
    });

    onCleanup(() => {
      unlisten.then((fn) => fn());
      unlistenShow.then((fn) => fn());
      unlistenClipboard.then((fn) => fn());
      unlistenTagRules.then((fn) => fn());
      unlistenTheme.then((fn) => fn());
      unlistenFocusCfg.then((fn) => fn());
      unlistenDismiss.then((fn) => fn());
    });

    // 跟随系统时响应系统外观变化
    const mediaQuery = window.matchMedia("(prefers-color-scheme: light)");
    const handleThemeChange = () => {
      if (getStoredChoice() === "system") {
        setThemeId(applyChoice("system").id);
      }
    };
    mediaQuery.addEventListener("change", handleThemeChange);
    onCleanup(() => mediaQuery.removeEventListener("change", handleThemeChange));
  });

  onCleanup(() => {
    clearInterval(refreshInterval);
  });

  const handleSearch = (value: string) => {
    setKeyword(value);
    setSelectedIndex(0);
    if (value.length > 0) setActiveTag("");
    loadItems();
  };

  const handleSelectTag = (tag: string) => {
    setActiveTag(tag);
    setKeyword("");
    setSelectedIndex(0);
    loadItems();
  };

  const handleSetTag = async (id: number, tag: string) => {
    await setItemTag(id, tag);
    await loadItems();
  };

  // 打开编辑浮层（仅文本类，图片不可编）
  const openEditor = (id: number) => {
    const item = items().find((i) => i.id === id);
    if (!item || item.content_type !== "text") return;
    setEditingItem(item);
    setEditDraft(item.content);
  };

  const cancelEdit = () => {
    setEditingItem(null);
    setEditDraft("");
  };

  // 保存编辑：trim 非空→落库→刷新列表→关闭（标签/板归属后端保留）
  const saveEdit = async () => {
    const target = editingItem();
    if (!target) return;
    const content = editDraft().trim();
    if (!content) return;
    try {
      await updateItemContent(target.id, content);
      cancelEdit();
      await loadItems();
    } catch (e) {
      console.error("Failed to update item content:", e);
    }
  };

  const handlePaste = async (id: number) => {
    if (hiding) return;
    hiding = true;
    // 先播滑出动画再触发粘贴（Rust 侧会隐藏窗口并恢复焦点粘贴）
    const anim = await playSlideOut();
    await pasteItem(id);
    anim?.cancel();
    await resetViewAfterHide();
    hiding = false;
  };

  const handleTogglePin = async (id: number) => {
    await togglePinItem(id);
    await loadItems();
  };

  // 条目加入/移出收藏板；命中内置收藏板时后端会同步 is_pinned，刷新列表保持星标一致
  const handleToggleBoard = async (itemId: number, boardId: number, add: boolean) => {
    try {
      if (add) {
        await addItemToBoard(boardId, itemId);
      } else {
        await removeItemFromBoard(boardId, itemId);
      }
      await loadItems();
    } catch (e) {
      console.error("Failed to toggle board membership:", e);
    }
  };

  const handleDelete = (id: number) => {
    const item = items().find((i) => i.id === id);
    if (!item) return;
    // 前端软删除：推入待删队列，从显示列表移除
    setPendingDeletes((prev) => [...prev, item]);
    setItems((prev) => prev.filter((i) => i.id !== id));
    setSelectedIndex((i) => Math.min(i, items().length - 1));
  };

  const handleUndo = () => {
    const pending = pendingDeletes();
    if (pending.length === 0) return;
    const restored = pending[pending.length - 1];
    setPendingDeletes((prev) => prev.slice(0, -1));
    // 按使用时间倒序插回正确位置（收藏不再置顶）
    setItems((prev) =>
      [...prev, restored].sort((a, b) => b.updated_at - a.updated_at)
    );
  };

  // 面板隐藏时批量落库删除
  const commitDeletes = async () => {
    const pending = pendingDeletes();
    if (pending.length === 0) return;
    for (const item of pending) {
      await deleteClipboardItem(item.id);
    }
    setPendingDeletes([]);
  };

  // 隐藏后后台重置视图：复位入场动画 → 落库待删项 → 清搜索/筛选 → 预加载最新数据
  const resetViewAfterHide = async () => {
    setReady(false);
    await commitDeletes();
    setKeyword("");
    setActiveTag("");
    setSelectedIndex(0);
    await loadItems(false);
    loadBoards();
  };

  // 滑出动画（WAAPI，理由同入场：CSS transition 在隐藏/恢复时不可靠）
  const playSlideOut = async () => {
    const app = document.querySelector(".app");
    if (!app) return undefined;
    const anim = app.animate(
      [{ transform: "translateY(0)" }, { transform: "translateY(100%)" }],
      { duration: 140, easing: "cubic-bezier(0.4, 0, 1, 1)", fill: "forwards" }
    );
    await anim.finished.catch(() => {});
    return anim;
  };

  // 统一收起流程：滑出动画 → 隐藏窗口 → 后台重置视图（hiding 防多路径重入）
  let hiding = false;
  const dismissPanel = async () => {
    if (hiding) return;
    hiding = true;
    const anim = await playSlideOut();
    await getCurrentWindow().hide();
    anim?.cancel();
    await resetViewAfterHide();
    hiding = false;
  };

  // 按住修饰键（⌘/Ctrl）时前 9 张卡片显示数字角标（配合 ⌘/Ctrl+1~9 快速粘贴）
  const [cmdHeld, setCmdHeld] = createSignal(false);

  // 键盘导航（左右方向键切换卡片）
  const handleKeyDown = (e: KeyboardEvent) => {
    // 编辑浮层打开时挂起全局快捷键：仅响应 Esc 取消、⌘/Ctrl+Enter 保存
    if (editingItem()) {
      if (e.key === "Escape") {
        e.preventDefault();
        cancelEdit();
      } else if (e.key === "Enter" && isModPressed(e)) {
        e.preventDefault();
        saveEdit();
      }
      return;
    }

    if (e.key === MOD_KEY) {
      setCmdHeld(true);
    }

    // ⌘/Ctrl+Z 撤销删除
    if (e.key === "z" && isModPressed(e)) {
      e.preventDefault();
      handleUndo();
      return;
    }

    // ⌘/Ctrl+, 打开设置
    if (e.key === "," && isModPressed(e)) {
      e.preventDefault();
      invoke("open_settings");
      return;
    }

    const list = items();

    // ⌘/Ctrl+1~9 快速粘贴对应位置的卡片
    if (isModPressed(e) && e.key >= "1" && e.key <= "9") {
      e.preventDefault();
      const target = list[Number(e.key) - 1];
      if (target) {
        handlePaste(target.id);
      }
      return;
    }
    switch (e.key) {
      case "ArrowRight":
      case "ArrowDown":
        e.preventDefault();
        setSelectedIndex((i) => Math.min(i + 1, list.length - 1));
        // 滚动选中卡片进入视野
        scrollToSelected();
        break;
      case "ArrowLeft":
      case "ArrowUp":
        e.preventDefault();
        setSelectedIndex((i) => Math.max(i - 1, 0));
        scrollToSelected();
        break;
      case "Enter":
        e.preventDefault();
        if (list[selectedIndex()]) {
          handlePaste(list[selectedIndex()].id);
        }
        break;
      case "Backspace":
      case "Delete":
        // 仅当焦点不在任何输入控件时（即在剪贴板列表区域），才作为删除快捷键
        if (document.activeElement === document.body || document.activeElement?.closest(".clipboard-list")) {
          e.preventDefault();
          if (list[selectedIndex()]) {
            handleDelete(list[selectedIndex()].id);
          }
        }
        break;
      case "Escape":
        dismissPanel();
        break;
      case "e":
      case "E":
        // 仅当焦点在列表区域（非搜索框等输入控件）时，E 编辑选中条目
        if (
          !isModPressed(e) &&
          (document.activeElement === document.body ||
            document.activeElement?.closest(".clipboard-list"))
        ) {
          if (list[selectedIndex()]) {
            e.preventDefault();
            openEditor(list[selectedIndex()].id);
          }
        }
        break;
    }
  };

  const scrollToSelected = () => {
    requestAnimationFrame(() => {
      const el = document.querySelector(".clipboard-item.selected");
      if (el) {
        el.scrollIntoView({ behavior: "smooth", block: "nearest", inline: "nearest" });
      }
    });
  };

  // 修饰键释放或窗口失焦时收起数字角标（隐藏窗口会丢 keyup，需兜底）
  const handleKeyUp = (e: KeyboardEvent) => {
    if (e.key === MOD_KEY) setCmdHeld(false);
  };
  const handleWindowBlur = () => setCmdHeld(false);

  onMount(() => {
    document.addEventListener("keydown", handleKeyDown);
    document.addEventListener("keyup", handleKeyUp);
    window.addEventListener("blur", handleWindowBlur);
  });

  onCleanup(() => {
    document.removeEventListener("keydown", handleKeyDown);
    document.removeEventListener("keyup", handleKeyUp);
    window.removeEventListener("blur", handleWindowBlur);
  });

  return (
    <div class={`app ${ready() ? "ready" : ""} ${cmdHeld() ? "show-quick-keys" : ""}`}>
      <div class="titlebar" data-tauri-drag-region>
        <BoardBar
          boards={boards()}
          activeTag={activeTag()}
          onSelectTag={handleSelectTag}
          onCreateBoard={handleCreateBoard}
          onRenameBoard={handleRenameBoard}
          onRecolorBoard={handleRecolorBoard}
          onDeleteBoard={handleDeleteBoard}
        />
        <div class="titlebar-right">
          <span class="titlebar-count">{total()} 条</span>
          <button class="btn-theme" onClick={() => invoke("open_settings")} title="设置">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <circle cx="12" cy="12" r="3" />
              <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z" />
            </svg>
          </button>
          <button class="btn-theme" onClick={toggleTheme} title="切换主题">
            {themeMode() === "dark" ? (
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <circle cx="12" cy="12" r="5" />
                <path d="M12 1v2M12 21v2M4.22 4.22l1.42 1.42M18.36 18.36l1.42 1.42M1 12h2M21 12h2M4.22 19.78l1.42-1.42M18.36 5.64l1.42-1.42" />
              </svg>
            ) : (
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z" />
              </svg>
            )}
          </button>
        </div>
      </div>
      <Show when={axMissing()}>
        <div class="perm-banner">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M10.29 3.86 1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z" />
            <line x1="12" y1="9" x2="12" y2="13" /><line x1="12" y1="17" x2="12.01" y2="17" />
          </svg>
          <span class="perm-banner-text">辅助功能权限未授权，选中后无法自动粘贴</span>
          <button class="perm-banner-btn" onClick={() => openAccessibilitySettings()}>前往授权</button>
          <button class="perm-banner-btn ghost" onClick={recheckAccessibility}>我已授权</button>
        </div>
      </Show>
      <SearchBar value={keyword()} onInput={handleSearch} />
      <TagBar tags={allTags()} activeTag={activeTag()} onSelectTag={handleSelectTag} />
      <ClipboardList
        items={items()}
        selectedIndex={selectedIndex()}
        blobsDir={blobsDir()}
        tagRules={tagRules()}
        boards={boards()}
        onPaste={handlePaste}
        onTogglePin={handleTogglePin}
        onDelete={handleDelete}
        onSetTag={handleSetTag}
        onToggleBoard={handleToggleBoard}
        onEdit={openEditor}
        hasMore={hasMore()}
        onLoadMore={loadMore}
      />
      <Show when={editingItem()}>
        <div class="edit-overlay" onClick={cancelEdit}>
          <div class="edit-dialog" onClick={(e) => e.stopPropagation()}>
            <textarea
              class="edit-textarea"
              value={editDraft()}
              onInput={(e) => setEditDraft(e.currentTarget.value)}
              ref={(el) => requestAnimationFrame(() => { el.focus(); el.select(); })}
            />
            <div class="edit-footer">
              <span class="edit-charcount">{editDraft().trim().length} 个字符</span>
              <div class="edit-btns">
                <button class="edit-btn" onClick={cancelEdit}>取消</button>
                <button class="edit-btn primary" disabled={!editDraft().trim()} onClick={saveEdit}>
                  保存 {IS_WINDOWS ? "Ctrl+Enter" : "⌘↩"}
                </button>
              </div>
            </div>
          </div>
        </div>
      </Show>
    </div>
  );
}

export default App;