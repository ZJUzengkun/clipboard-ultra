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

  let refreshInterval: number;

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
        <span class="titlebar-count">{items().length} 条记录</span>
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

