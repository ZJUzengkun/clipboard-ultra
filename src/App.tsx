import { createSignal, onMount, onCleanup } from "solid-js";
import SearchBar from "./components/SearchBar";
import ClipboardList from "./components/ClipboardList";
import { ClipboardItemData } from "./components/ClipboardItem";
import {
  getClipboardItems,
  searchClipboard,
  togglePinItem,
  deleteClipboardItem,
  pasteItem,
} from "./hooks/useClipboard";
import { getCurrentWindow } from "@tauri-apps/api/window";

function App() {
  const [items, setItems] = createSignal<ClipboardItemData[]>([]);
  const [keyword, setKeyword] = createSignal("");
  const [selectedIndex, setSelectedIndex] = createSignal(0);
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

    loadItems();
    refreshInterval = window.setInterval(loadItems, 1000);

    // 窗口失焦时自动隐藏
    const appWindow = getCurrentWindow();
    const unlisten = appWindow.onFocusChanged(({ payload: focused }) => {
      if (!focused) {
        appWindow.hide();
      }
    });

    onCleanup(() => {
      unlisten.then((fn) => fn());
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
        onPaste={handlePaste}
        onTogglePin={handleTogglePin}
        onDelete={handleDelete}
      />
    </div>
  );
}

export default App;