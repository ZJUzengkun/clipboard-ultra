import { createSignal, onMount, onCleanup, Show } from "solid-js";
import SearchBar from "./components/SearchBar";
import ClipboardList from "./components/ClipboardList";
import Settings from "./components/Settings";
import { ClipboardItemData } from "./components/ClipboardItem";
import {
  getClipboardItems,
  searchClipboard,
  togglePinItem,
  deleteClipboardItem,
  pasteItem,
  getBlobsDir,
} from "./hooks/useClipboard";
import { getCurrentWindow } from "@tauri-apps/api/window";

function App() {
  const [items, setItems] = createSignal<ClipboardItemData[]>([]);
  const [keyword, setKeyword] = createSignal("");
  const [selectedIndex, setSelectedIndex] = createSignal(0);
  const [blobsDir, setBlobsDir] = createSignal("");
  const [showSettings, setShowSettings] = createSignal(false);
  const [theme, setTheme] = createSignal<"dark" | "light">(
    window.matchMedia("(prefers-color-scheme: light)").matches ? "light" : "dark"
  );

  let refreshInterval: number;

  const toggleTheme = () => {
    const next = theme() === "dark" ? "light" : "dark";
    setTheme(next);
    document.documentElement.setAttribute("data-theme", next);
  };

  const loadItems = async () => {
    try {
      const k = keyword();
      const result =
        k.length > 0
          ? await searchClipboard(k)
          : await getClipboardItems(50);
      setItems(result);
    } catch (e) {
      console.error("Failed to load items:", e);
    }
  };

  onMount(() => {
    // 初始化主题
    document.documentElement.setAttribute("data-theme", theme());

    // 获取 blobs 目录路径
    getBlobsDir().then(setBlobsDir).catch(console.error);

    loadItems();
    refreshInterval = window.setInterval(loadItems, 1000);

    // 窗口失焦时自动隐藏（带 debounce 防止 show 后立即触发 blur）
    const appWindow = getCurrentWindow();
    let showTimestamp = 0;

    // 监听窗口显示事件，记录时间戳
    const unlistenShow = appWindow.listen("tauri://focus", () => {
      showTimestamp = Date.now();
    });

    const unlisten = appWindow.onFocusChanged(({ payload: focused }) => {
      if (!focused) {
        // 如果窗口刚获得焦点不到 300ms，不要隐藏（防止闪屏）
        const elapsed = Date.now() - showTimestamp;
        if (elapsed > 300) {
          appWindow.hide();
        }
      }
    });

    onCleanup(() => {
      unlisten.then((fn) => fn());
      unlistenShow.then((fn) => fn());
    });

    // 监听系统主题变化
    const mediaQuery = window.matchMedia("(prefers-color-scheme: light)");
    const handleThemeChange = (e: MediaQueryListEvent) => {
      const newTheme = e.matches ? "light" : "dark";
      setTheme(newTheme);
      document.documentElement.setAttribute("data-theme", newTheme);
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
    loadItems();
  };

  const handlePaste = async (id: number) => {
    await pasteItem(id);
  };

  const handleTogglePin = async (id: number) => {
    await togglePinItem(id);
    await loadItems();
  };

  const handleDelete = async (id: number) => {
    await deleteClipboardItem(id);
    await loadItems();
  };

  // 键盘导航
  const handleKeyDown = (e: KeyboardEvent) => {
    const list = items();
    switch (e.key) {
      case "ArrowDown":
        e.preventDefault();
        setSelectedIndex((i) => Math.min(i + 1, list.length - 1));
        break;
      case "ArrowUp":
        e.preventDefault();
        setSelectedIndex((i) => Math.max(i - 1, 0));
        break;
      case "Enter":
        e.preventDefault();
        if (list[selectedIndex()]) {
          handlePaste(list[selectedIndex()].id);
        }
        break;
      case "Escape":
        getCurrentWindow().hide();
        break;
    }
  };

  onMount(() => {
    document.addEventListener("keydown", handleKeyDown);
  });

  onCleanup(() => {
    document.removeEventListener("keydown", handleKeyDown);
  });

  return (
    <div class="app">
      <div class="titlebar" data-tauri-drag-region>
        <span class="titlebar-title">ClipBoard Pro</span>
        <div class="titlebar-right">
          <span class="titlebar-count">{items().length} 条</span>
          <button class="btn-theme" onClick={() => setShowSettings(true)} title="设置">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <circle cx="12" cy="12" r="3" />
              <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z" />
            </svg>
          </button>
          <button class="btn-theme" onClick={toggleTheme} title="切换主题">
            {theme() === "dark" ? (
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
      <SearchBar value={keyword()} onInput={handleSearch} />
      <ClipboardList
        items={items()}
        selectedIndex={selectedIndex()}
        blobsDir={blobsDir()}
        onPaste={handlePaste}
        onTogglePin={handleTogglePin}
        onDelete={handleDelete}
      />
      <Show when={showSettings()}>
        <Settings onClose={() => setShowSettings(false)} />
      </Show>
    </div>
  );
}

export default App;