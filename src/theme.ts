// 主题系统：注册表 + 持久化 + 应用
// 存储于 localStorage（主/设置窗口同源共享），跨窗口用 Tauri 事件 "theme-changed" 同步

export type ThemeMode = "dark" | "light";

export interface ThemeDef {
  id: string;
  name: string;
  mode: ThemeMode;
  /** 设置页预览色板 */
  preview: { bg: string; card: string; accent: string; text: string };
}

export const THEMES: ThemeDef[] = [
  { id: "dark", name: "深邃夜", mode: "dark", preview: { bg: "#16161e", card: "#262636", accent: "#7c6df0", text: "#eae9fc" } },
  { id: "nord", name: "Nord", mode: "dark", preview: { bg: "#2e3440", card: "#3b4252", accent: "#88c0d0", text: "#eceff4" } },
  { id: "dracula", name: "Dracula", mode: "dark", preview: { bg: "#282a36", card: "#383a4c", accent: "#bd93f9", text: "#f8f8f2" } },
  { id: "mocha", name: "Catppuccin", mode: "dark", preview: { bg: "#1e1e2e", card: "#313244", accent: "#cba6f7", text: "#cdd6f4" } },
  { id: "ocean", name: "海洋蓝", mode: "light", preview: { bg: "#eef3fa", card: "#ffffff", accent: "#3478f6", text: "#1e2b3c" } },
  { id: "light", name: "清爽白", mode: "light", preview: { bg: "#ffffff", card: "#f4f4f8", accent: "#6355e0", text: "#1a1a2e" } },
  { id: "solarized", name: "Solarized", mode: "light", preview: { bg: "#fdf6e3", card: "#eee8d5", accent: "#268bd2", text: "#073642" } },
];

/** 用户选择："system" 或主题 id */
const KEY_CHOICE = "cu-theme";
/** 最近生效的 "id|mode"，供 html 内联脚本首帧防闪 */
const KEY_CACHE = "cu-theme-cache";
/** 暗/亮组内最近使用的主题，跟随系统与顶栏快切时用 */
const KEY_LAST_DARK = "cu-last-dark";
const KEY_LAST_LIGHT = "cu-last-light";

export function themeOf(id: string): ThemeDef {
  return THEMES.find((t) => t.id === id) ?? THEMES[0];
}

export function getStoredChoice(): string {
  return localStorage.getItem(KEY_CHOICE) || "system";
}

/** 某模式组内最近使用的主题 id */
export function lastThemeIdFor(mode: ThemeMode): string {
  const key = mode === "dark" ? KEY_LAST_DARK : KEY_LAST_LIGHT;
  const stored = localStorage.getItem(key);
  return stored && THEMES.some((t) => t.id === stored) ? stored : mode;
}

/** 把选择解析为具体主题 id（system → 按系统外观取对应组的最近主题） */
export function resolveThemeId(choice: string): string {
  if (choice !== "system") {
    return THEMES.some((t) => t.id === choice) ? choice : "dark";
  }
  const light = window.matchMedia("(prefers-color-scheme: light)").matches;
  return lastThemeIdFor(light ? "light" : "dark");
}

/** 应用选择到当前窗口 DOM，并刷新首帧缓存 */
export function applyChoice(choice: string): ThemeDef {
  const t = themeOf(resolveThemeId(choice));
  document.documentElement.setAttribute("data-theme", t.id);
  document.documentElement.setAttribute("data-mode", t.mode);
  localStorage.setItem(KEY_CACHE, `${t.id}|${t.mode}`);
  return t;
}

/** 保存选择 + 应用 + 记录组内最近主题 */
export function saveChoice(choice: string): ThemeDef {
  localStorage.setItem(KEY_CHOICE, choice);
  const t = applyChoice(choice);
  if (choice !== "system") {
    localStorage.setItem(t.mode === "dark" ? KEY_LAST_DARK : KEY_LAST_LIGHT, t.id);
  }
  return t;
}
