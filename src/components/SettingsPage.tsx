import { Component, createSignal, onMount, onCleanup, Show, For } from "solid-js";
import { getShortcut, setShortcut, getTagRules, addTagRule, deleteTagRule, updateTagRuleExpire, getDefaultExpireDays, setDefaultExpireDays, getContentTypeExpireDays, setContentTypeExpireDays, getMaxItems, setMaxItems, TagRule, getExcludedApps, getExcludedAppsNames, addExcludedApp, removeExcludedApp, getRunningApps, RunningApp } from "../hooks/useClipboard";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { emit, listen } from "@tauri-apps/api/event";
import { THEMES, getStoredChoice, applyChoice, saveChoice } from "../theme";
import { getVersion } from "@tauri-apps/api/app";
import { check } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";

// 内置检测器：pattern 存 "builtin:xxx"，Rust 端用解析器/校验位算法检测，比正则更准
const BUILTIN_DETECTORS: { value: string; label: string; desc: string }[] = [
  { value: "builtin:json", label: "JSON", desc: "JSON 语法校验" },
  { value: "builtin:url", label: "链接", desc: "URL 解析校验" },
  { value: "builtin:bankcard", label: "银行卡", desc: "Luhn 校验位" },
  { value: "builtin:idcard", label: "身份证", desc: "加权校验码" },
];

const builtinLabel = (pattern: string) => {
  const d = BUILTIN_DETECTORS.find((d) => d.value === pattern);
  return d ? `内置检测 · ${d.desc}` : pattern;
};

const SettingsPage: Component = () => {
  const [currentShortcut, setCurrentShortcut] = createSignal("");
  const [recording, setRecording] = createSignal(false);
  const [recordedKeys, setRecordedKeys] = createSignal("");
  const [saving, setSaving] = createSignal(false);
  const [error, setError] = createSignal("");
  const [success, setSuccess] = createSignal("");

  // 设置分类导航：左侧边栏当前选中项
  const [activeTab, setActiveTab] = createSignal("appearance");

  // 导航分类（图标与各区块标题一致）
  const navItems = [
    { id: "appearance", label: "外观", icon: <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="13.5" cy="6.5" r=".5" /><circle cx="17.5" cy="10.5" r=".5" /><circle cx="8.5" cy="7.5" r=".5" /><circle cx="6.5" cy="12.5" r=".5" /><path d="M12 2C6.5 2 2 6.5 2 12s4.5 10 10 10c.926 0 1.648-.746 1.648-1.688 0-.437-.18-.835-.437-1.125-.29-.289-.438-.652-.438-1.125a1.64 1.64 0 0 1 1.668-1.668h1.996c3.051 0 5.555-2.503 5.555-5.554C21.965 6.012 17.461 2 12 2z" /></svg> },
    { id: "hotkey", label: "快捷键", icon: <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="2" y="4" width="20" height="16" rx="2" /><path d="M6 8h.01M10 8h.01M14 8h.01M18 8h.01M6 12h.01M10 12h.01M14 12h.01M18 12h.01M8 16h8" /></svg> },
    { id: "tags", label: "标签规则", icon: <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M20.59 13.41l-7.17 7.17a2 2 0 0 1-2.83 0L2 12V2h10l8.59 8.59a2 2 0 0 1 0 2.82z" /><line x1="7" y1="7" x2="7.01" y2="7" /></svg> },
    { id: "privacy", label: "隐私", icon: <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z" /></svg> },
    { id: "data", label: "数据管理", icon: <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10" /><polyline points="12 6 12 12 16 14" /></svg> },
    { id: "about", label: "关于与更新", icon: <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z" /></svg> },
  ];

  // 标签规则状态
  const [rules, setRules] = createSignal<TagRule[]>([]);
  const [newRuleName, setNewRuleName] = createSignal("");
  const [newRulePattern, setNewRulePattern] = createSignal("");
  // 规则类型：regex = 手写正则（含留空手动标签），其余为内置检测器的 builtin:xxx
  const [newRuleType, setNewRuleType] = createSignal("regex");
  const [newRuleColor, setNewRuleColor] = createSignal("#7c6df0");
  const [ruleError, setRuleError] = createSignal("");

  const presetColors = ["#7c6df0", "#f06070", "#f0a050", "#50d0a0", "#5090f0", "#d060d0"];

  // 排除应用状态
  const [excludedApps, setExcludedApps] = createSignal<string[]>([]);
  const [excludedNames, setExcludedNames] = createSignal<Record<string, string>>({});
  const [showAppPicker, setShowAppPicker] = createSignal(false);
  const [runningApps, setRunningApps] = createSignal<RunningApp[]>([]);
  const [loadingApps, setLoadingApps] = createSignal(false);

  // 过期策略状态
  const [defaultExpire, setDefaultExpire] = createSignal(0);
  const [imageExpire, setImageExpire] = createSignal(0);
  const [textExpire, setTextExpire] = createSignal(0);
  const [newRuleExpire, setNewRuleExpire] = createSignal(0);

  // 最大保存数量（-1 = 不限制）
  const [maxItems, setMaxItemsSignal] = createSignal(1000);

  // 更新状态
  const [appVersion, setAppVersion] = createSignal("");
  const [updateStatus, setUpdateStatus] = createSignal<"idle" | "checking" | "available" | "downloading" | "latest" | "error">("idle");
  const [updateVersion, setUpdateVersion] = createSignal("");
  const [updateError, setUpdateError] = createSignal("");
  const [downloadProgress, setDownloadProgress] = createSignal(0);
  let pendingUpdate: Awaited<ReturnType<typeof check>> = null;

  // 主题选择（"system" 或主题 id）
  const [themeChoice, setThemeChoice] = createSignal(getStoredChoice());

  const selectTheme = (choice: string) => {
    setThemeChoice(choice);
    saveChoice(choice);
    emit("theme-changed", choice);
  };

  const expireOptions = [
    { value: 0, label: "永不过期" },
    { value: 1, label: "1 天" },
    { value: 3, label: "3 天" },
    { value: 7, label: "7 天" },
    { value: 14, label: "14 天" },
    { value: 30, label: "30 天" },
    { value: 90, label: "90 天" },
  ];

  onMount(async () => {
    // 应用已保存的主题
    applyChoice(getStoredChoice());

    // 主窗口顶栏快切时同步本窗口
    const unlistenTheme = listen<string>("theme-changed", (e) => {
      setThemeChoice(e.payload);
      applyChoice(e.payload);
    });
    onCleanup(() => unlistenTheme.then((fn) => fn()));

    try {
      const shortcut = await getShortcut();
      setCurrentShortcut(shortcut);
    } catch (e) {
      console.error("Failed to get shortcut:", e);
    }
    try {
      const tagRules = await getTagRules();
      setRules(tagRules);
    } catch (e) {
      console.error("Failed to load tag rules:", e);
    }
    try {
      const apps = await getExcludedApps();
      setExcludedApps(apps);
      const names = await getExcludedAppsNames();
      setExcludedNames(names);
    } catch (e) {
      console.error("Failed to load excluded apps:", e);
    }
    try {
      const days = await getDefaultExpireDays();
      setDefaultExpire(days);
    } catch (e) {
      console.error("Failed to load default expire days:", e);
    }
    try {
      const count = await getMaxItems();
      setMaxItemsSignal(count);
    } catch (e) {
      console.error("Failed to load max items:", e);
    }
    try {
      const imgDays = await getContentTypeExpireDays("image");
      setImageExpire(imgDays);
      const txtDays = await getContentTypeExpireDays("text");
      setTextExpire(txtDays);
    } catch (e) {
      console.error("Failed to load content type expire days:", e);
    }
    try {
      setAppVersion(await getVersion());
    } catch (e) {
      console.error("Failed to get app version:", e);
    }
  });

  // 检查更新
  const handleCheckUpdate = async () => {
    setUpdateStatus("checking");
    setUpdateError("");
    try {
      const update = await check();
      if (update) {
        pendingUpdate = update;
        setUpdateVersion(update.version);
        setUpdateStatus("available");
      } else {
        setUpdateStatus("latest");
      }
    } catch (e: any) {
      setUpdateError(`检查失败: ${e}`);
      setUpdateStatus("error");
    }
  };

  // 下载并安装更新
  const handleInstallUpdate = async () => {
    if (!pendingUpdate) return;
    setUpdateStatus("downloading");
    setDownloadProgress(0);
    let downloaded = 0;
    let contentLength = 0;
    try {
      await pendingUpdate.downloadAndInstall((event) => {
        switch (event.event) {
          case "Started":
            contentLength = event.data.contentLength ?? 0;
            break;
          case "Progress":
            downloaded += event.data.chunkLength;
            if (contentLength > 0) {
              setDownloadProgress(Math.round((downloaded / contentLength) * 100));
            }
            break;
          case "Finished":
            setDownloadProgress(100);
            break;
        }
      });
      // 安装完成，重启应用
      await relaunch();
    } catch (e: any) {
      setUpdateError(`更新失败: ${e}`);
      setUpdateStatus("error");
    }
  };

  const notifyMainWindow = () => {
    // 通知主窗口刷新标签规则
    emit("tag-rules-changed");
  };

  const handleAddRule = async () => {
    const name = newRuleName().trim();
    const isBuiltin = newRuleType() !== "regex";
    const pattern = isBuiltin ? newRuleType() : newRulePattern().trim();
    if (!name) {
      setRuleError("标签名称不能为空");
      return;
    }
    // 正则选填：留空则为手动标签；填了才校验合法性（内置检测器无需校验）
    if (!isBuiltin && pattern) {
      try {
        new RegExp(pattern);
      } catch {
        setRuleError("正则表达式格式不合法");
        return;
      }
    }
    setRuleError("");
    try {
      await addTagRule(name, pattern, newRuleColor(), rules().length, newRuleExpire());
      const updated = await getTagRules();
      setRules(updated);
      setNewRuleName("");
      setNewRulePattern("");
      setNewRuleType("regex");
      setNewRuleColor("#7c6df0");
      setNewRuleExpire(0);
      notifyMainWindow();
    } catch (e: any) {
      setRuleError(`添加失败: ${e}`);
    }
  };

  const handleDeleteRule = async (id: number) => {
    try {
      await deleteTagRule(id);
      const updated = await getTagRules();
      setRules(updated);
      notifyMainWindow();
    } catch (e) {
      console.error("Failed to delete rule:", e);
    }
  };

  const addPreset = (name: string, pattern: string) => {
    setNewRuleName(name);
    setNewRulePattern(pattern);
  };

  // 排除应用操作
  const handleShowAppPicker = async () => {
    setShowAppPicker(true);
    setLoadingApps(true);
    try {
      const apps = await getRunningApps();
      // 过滤已排除的
      setRunningApps(apps.filter(a => !excludedApps().includes(a.bundle_id)));
    } catch (e) {
      console.error("Failed to load running apps:", e);
    } finally {
      setLoadingApps(false);
    }
  };

  const handleAddExcludedApp = async (app: RunningApp) => {
    try {
      await addExcludedApp(app.bundle_id, app.name);
      setExcludedApps([...excludedApps(), app.bundle_id]);
      setExcludedNames({ ...excludedNames(), [app.bundle_id]: app.name });
      setRunningApps(runningApps().filter(a => a.bundle_id !== app.bundle_id));
    } catch (e) {
      console.error("Failed to add excluded app:", e);
    }
  };

  const handleRemoveExcludedApp = async (bundleId: string) => {
    try {
      await removeExcludedApp(bundleId);
      setExcludedApps(excludedApps().filter(id => id !== bundleId));
      const names = { ...excludedNames() };
      delete names[bundleId];
      setExcludedNames(names);
    } catch (e) {
      console.error("Failed to remove excluded app:", e);
    }
  };

  const keyEventToShortcut = (e: KeyboardEvent): string => {
    const parts: string[] = [];
    if (e.ctrlKey || e.metaKey) parts.push("CommandOrControl");
    if (e.altKey) parts.push("Alt");
    if (e.shiftKey) parts.push("Shift");
    const key = e.key;
    if (!["Control", "Meta", "Alt", "Shift"].includes(key)) {
      const keyMap: Record<string, string> = {
        " ": "Space", ArrowUp: "Up", ArrowDown: "Down", ArrowLeft: "Left",
        ArrowRight: "Right", Escape: "Escape", Enter: "Enter",
        Backspace: "Backspace", Delete: "Delete", Tab: "Tab",
      };
      const mapped = keyMap[key] || key.toUpperCase();
      parts.push(mapped);
    }
    return parts.join("+");
  };

  const handleKeyDown = (e: KeyboardEvent) => {
    if (!recording()) return;
    e.preventDefault();
    e.stopPropagation();
    const shortcut = keyEventToShortcut(e);
    if (shortcut.includes("+") && !["CommandOrControl", "Alt", "Shift"].includes(shortcut)) {
      setRecordedKeys(shortcut);
    }
  };

  const handleKeyUp = (e: KeyboardEvent) => {
    if (!recording()) return;
    e.preventDefault();
    if (recordedKeys() && !e.ctrlKey && !e.metaKey && !e.altKey && !e.shiftKey) {
      setRecording(false);
    }
  };

  const startRecording = () => {
    setRecording(true);
    setRecordedKeys("");
    setError("");
    setSuccess("");
  };

  const saveShortcut = async () => {
    const newShortcut = recordedKeys();
    if (!newShortcut) return;
    setSaving(true);
    setError("");
    setSuccess("");
    try {
      await setShortcut(newShortcut);
      setCurrentShortcut(newShortcut);
      setRecordedKeys("");
      setSuccess("快捷键已更新");
      setTimeout(() => setSuccess(""), 2000);
    } catch (e: any) {
      setError(`设置失败: ${e}`);
    } finally {
      setSaving(false);
    }
  };

  const resetShortcut = async () => {
    setSaving(true);
    setError("");
    try {
      await setShortcut("CommandOrControl+Shift+V");
      setCurrentShortcut("CommandOrControl+Shift+V");
      setRecordedKeys("");
      setSuccess("已恢复默认快捷键");
      setTimeout(() => setSuccess(""), 2000);
    } catch (e: any) {
      setError(`重置失败: ${e}`);
    } finally {
      setSaving(false);
    }
  };

  const formatShortcut = (s: string) => {
    return s
      .replace("CommandOrControl", "Ctrl/⌘")
      .replace("Shift", "⇧")
      .replace("Alt", "⌥");
  };

  const handleClose = () => {
    getCurrentWindow().hide();
  };

  // ESC 关闭窗口
  const handleGlobalKeyDown = (e: KeyboardEvent) => {
    if (e.key === "Escape" && !recording()) {
      handleClose();
    }
  };

  onMount(() => {
    document.addEventListener("keydown", handleGlobalKeyDown);
  });

  onCleanup(() => {
    document.removeEventListener("keydown", handleGlobalKeyDown);
  });

  return (
    <div class="settings-window">
      <div class="settings-titlebar" data-tauri-drag-region>
        <div class="settings-titlebar-left" data-tauri-drag-region="false">
          <button class="settings-traffic-btn traffic-close" onClick={handleClose} title="关闭">
            <svg viewBox="0 0 6 6"><path d="M0 0L6 6M6 0L0 6" stroke="currentColor" stroke-width="1.2"/></svg>
          </button>
        </div>
        <span class="settings-titlebar-title">偏好设置</span>
        <div class="settings-titlebar-right" />
      </div>

      <div class="settings-layout">
        {/* 左侧分类导航 */}
        <nav class="settings-nav">
          <For each={navItems}>
            {(item) => (
              <button
                class={`settings-nav-item ${activeTab() === item.id ? "active" : ""}`}
                onClick={() => setActiveTab(item.id)}
              >
                {item.icon}
                <span>{item.label}</span>
              </button>
            )}
          </For>
        </nav>

      <div class="settings-window-body">
        {/* 外观区域 */}
        <Show when={activeTab() === "appearance"}>
        <section class="settings-section">
          <div class="section-title">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="section-icon">
              <circle cx="13.5" cy="6.5" r=".5" />
              <circle cx="17.5" cy="10.5" r=".5" />
              <circle cx="8.5" cy="7.5" r=".5" />
              <circle cx="6.5" cy="12.5" r=".5" />
              <path d="M12 2C6.5 2 2 6.5 2 12s4.5 10 10 10c.926 0 1.648-.746 1.648-1.688 0-.437-.18-.835-.437-1.125-.29-.289-.438-.652-.438-1.125a1.64 1.64 0 0 1 1.668-1.668h1.996c3.051 0 5.555-2.503 5.555-5.554C21.965 6.012 17.461 2 12 2z" />
            </svg>
            <span>外观</span>
          </div>

          <div class="theme-grid">
            <button
              class={`theme-card ${themeChoice() === "system" ? "active" : ""}`}
              onClick={() => selectTheme("system")}
            >
              <div class="theme-swatch theme-swatch-system">
                <span class="theme-swatch-half" style={{ background: "#16161e" }}></span>
                <span class="theme-swatch-half" style={{ background: "#f4f4f8" }}></span>
              </div>
              <span class="theme-card-name">跟随系统</span>
            </button>
            <For each={THEMES}>
              {(t) => (
                <button
                  class={`theme-card ${themeChoice() === t.id ? "active" : ""}`}
                  onClick={() => selectTheme(t.id)}
                >
                  <div class="theme-swatch" style={{ background: t.preview.bg }}>
                    <span class="theme-swatch-card" style={{ background: t.preview.card }}>
                      <span class="theme-swatch-line" style={{ background: t.preview.text }}></span>
                      <span class="theme-swatch-line short" style={{ background: t.preview.accent }}></span>
                    </span>
                  </div>
                  <span class="theme-card-name">{t.name}</span>
                </button>
              )}
            </For>
          </div>
        </section>
        </Show>

        {/* 快捷键区域 */}
        <Show when={activeTab() === "hotkey"}>
        <section class="settings-section">
          <div class="section-title">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="section-icon">
              <rect x="2" y="4" width="20" height="16" rx="2" />
              <path d="M6 8h.01M10 8h.01M14 8h.01M18 8h.01M6 12h.01M10 12h.01M14 12h.01M18 12h.01M8 16h8" />
            </svg>
            <span>快捷键</span>
          </div>

          <div class="shortcut-card">
            <div class="shortcut-row">
              <span class="shortcut-label">唤起面板</span>
              <span class="current-shortcut">{formatShortcut(currentShortcut())}</span>
            </div>

            <div class="shortcut-recorder">
              <Show
                when={recording()}
                fallback={
                  <button class="btn-record" onClick={startRecording}>
                    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" style="width:14px;height:14px">
                      <circle cx="12" cy="12" r="10" />
                      <circle cx="12" cy="12" r="4" fill="currentColor" />
                    </svg>
                    录制新快捷键
                  </button>
                }
              >
                <div
                  class="recorder-active"
                  tabIndex={0}
                  onKeyDown={handleKeyDown}
                  onKeyUp={handleKeyUp}
                  ref={(el) => el && el.focus()}
                >
                  <Show when={recordedKeys()} fallback={<span class="recorder-hint">请按下组合键...</span>}>
                    <span class="recorded-keys">{formatShortcut(recordedKeys())}</span>
                  </Show>
                </div>
              </Show>
            </div>

            <Show when={recordedKeys() && !recording()}>
              <div class="shortcut-actions">
                <button class="btn-save" onClick={saveShortcut} disabled={saving()}>
                  {saving() ? "保存中..." : "应用"}
                </button>
                <button class="btn-cancel" onClick={() => setRecordedKeys("")}>
                  取消
                </button>
              </div>
            </Show>

            <Show when={error()}>
              <p class="settings-error">{error()}</p>
            </Show>
            <Show when={success()}>
              <p class="settings-success">{success()}</p>
            </Show>

            <button class="btn-reset" onClick={resetShortcut} disabled={saving()}>
              恢复默认 (Ctrl+Shift+V)
            </button>
          </div>
        </section>
        </Show>

        {/* 标签规则区域 */}
        <Show when={activeTab() === "tags"}>
        <section class="settings-section">
          <div class="section-title">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="section-icon">
              <path d="M20.59 13.41l-7.17 7.17a2 2 0 0 1-2.83 0L2 12V2h10l8.59 8.59a2 2 0 0 1 0 2.82z" />
              <line x1="7" y1="7" x2="7.01" y2="7" />
            </svg>
            <span>标签规则</span>
            <span class="section-badge">{rules().length} 条</span>
          </div>

          {/* 已有规则列表 */}
          <Show when={rules().length > 0}>
            <div class="tag-rules-list">
              <For each={rules()}>
                {(rule) => (
                  <div class="tag-rule-row">
                    <span class="tag-dot-lg" style={{ background: rule.color }}></span>
                    <div class="tag-rule-info">
                      <span class="tag-rule-name">{rule.name}</span>
                      <Show when={rule.pattern} fallback={<span class="tag-rule-manual">手动打标签</span>}>
                        <code class="tag-rule-pattern">{rule.pattern.startsWith("builtin:") ? builtinLabel(rule.pattern) : rule.pattern}</code>
                      </Show>
                    </div>
                    <select
                      class="expire-select"
                      value={rule.expire_days}
                      onChange={async (e) => {
                        const days = parseInt(e.currentTarget.value);
                        await updateTagRuleExpire(rule.id, days);
                        const updated = await getTagRules();
                        setRules(updated);
                      }}
                    >
                      <For each={expireOptions}>
                        {(opt) => <option value={opt.value}>{opt.label}</option>}
                      </For>
                    </select>
                    <button class="btn-rule-delete" onClick={() => handleDeleteRule(rule.id)} title="删除规则">
                      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                        <path d="M3 6h18M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" />
                      </svg>
                    </button>
                  </div>
                )}
              </For>
            </div>
          </Show>

          <Show when={rules().length === 0}>
            <div class="tag-rules-empty">
              <span>暂无规则，添加规则后新复制的内容将自动匹配标签</span>
            </div>
          </Show>

          {/* 新增规则表单 */}
          <div class="tag-rule-form">
            <div class="form-row">
              <input
                type="text"
                placeholder="标签名称"
                value={newRuleName()}
                onInput={(e) => setNewRuleName(e.currentTarget.value)}
                class="tag-rule-input"
              />
              {/* 规则类型：手写正则 or 内置检测器；选内置时隐藏正则输入框 */}
              <select
                class="expire-select rule-type-select"
                value={newRuleType()}
                onChange={(e) => {
                  const v = e.currentTarget.value;
                  setNewRuleType(v);
                  // 选内置检测器时，名称为空则自动带出默认名
                  if (v !== "regex" && !newRuleName().trim()) {
                    const d = BUILTIN_DETECTORS.find((d) => d.value === v);
                    if (d) setNewRuleName(d.label);
                  }
                }}
              >
                <option value="regex">正则</option>
                <For each={BUILTIN_DETECTORS}>
                  {(d) => <option value={d.value}>{d.label}</option>}
                </For>
              </select>
              <Show when={newRuleType() === "regex"}>
                <input
                  type="text"
                  placeholder="正则表达式（留空=手动标签）"
                  value={newRulePattern()}
                  onInput={(e) => setNewRulePattern(e.currentTarget.value)}
                  class="tag-rule-input input-pattern"
                />
              </Show>
            </div>

            <div class="form-row form-row-between">
              <div class="tag-color-picker">
                <For each={presetColors}>
                  {(color) => (
                    <button
                      class={`color-swatch ${newRuleColor() === color ? "active" : ""}`}
                      style={{ background: color }}
                      onClick={() => setNewRuleColor(color)}
                    />
                  )}
                </For>
              </div>
              <select class="expire-select" value={newRuleExpire()} onChange={(e) => setNewRuleExpire(parseInt(e.currentTarget.value))}>
                <For each={expireOptions}>
                  {(opt) => <option value={opt.value}>{opt.label}</option>}
                </For>
              </select>
              <button class="btn-add-rule" onClick={handleAddRule}>
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" style="width:14px;height:14px">
                  <path d="M12 5v14M5 12h14" />
                </svg>
                添加
              </button>
            </div>
          </div>

          {/* 预设示例 */}
          <div class="tag-presets">
            <span class="tag-preset-label">快捷预设</span>
            <div class="tag-preset-btns">
              <button class="tag-preset-btn" onClick={() => addPreset("链接", "https?://")}>
                <span class="preset-dot" style="background: #5090f0"></span>URL
              </button>
              <button class="tag-preset-btn" onClick={() => addPreset("邮箱", "[\\w.-]+@[\\w.-]+\\.\\w+")}>
                <span class="preset-dot" style="background: #f0a050"></span>邮箱
              </button>
              <button class="tag-preset-btn" onClick={() => addPreset("代码", "^(import|function|const|class|def|pub fn)")}>
                <span class="preset-dot" style="background: #50d0a0"></span>代码
              </button>
            </div>
          </div>

          <Show when={ruleError()}>
            <p class="settings-error">{ruleError()}</p>
          </Show>
        </section>
        </Show>

        {/* 排除应用区域 */}
        <Show when={activeTab() === "privacy"}>
        <section class="settings-section">
          <div class="section-title">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="section-icon">
              <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z" />
            </svg>
            <span>隐私</span>
            <span class="section-badge">{excludedApps().length} 个</span>
          </div>

          <p class="excluded-apps-desc">来自以下应用的复制内容将不会被记录</p>

          <Show when={excludedApps().length > 0}>
            <div class="excluded-apps-list">
              <For each={excludedApps()}>
                {(bundleId) => (
                  <div class="excluded-app-row">
                    <div class="excluded-app-info">
                      <span class="excluded-app-name">{excludedNames()[bundleId] || bundleId}</span>
                      <span class="excluded-app-bundle">{bundleId}</span>
                    </div>
                    <button class="btn-rule-delete" onClick={() => handleRemoveExcludedApp(bundleId)} title="移除">
                      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                        <path d="M18 6L6 18M6 6l12 12" />
                      </svg>
                    </button>
                  </div>
                )}
              </For>
            </div>
          </Show>

          <Show when={excludedApps().length === 0 && !showAppPicker()}>
            <div class="tag-rules-empty">
              <span>暂无排除应用，建议添加密码管理器等敏感应用</span>
            </div>
          </Show>

          <div class="excluded-apps-actions">
            <button class="btn-add-rule" onClick={handleShowAppPicker}>
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" style="width:14px;height:14px">
                <path d="M12 5v14M5 12h14" />
              </svg>
              添加应用
            </button>
          </div>

          <Show when={showAppPicker()}>
            <div class="app-picker-panel">
              <div class="app-picker-header">
                <span>选择要排除的应用</span>
                <button class="btn-close-sm" onClick={() => setShowAppPicker(false)}>
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                    <path d="M18 6L6 18M6 6l12 12" />
                  </svg>
                </button>
              </div>
              <Show when={loadingApps()}>
                <div class="app-picker-loading">加载中...</div>
              </Show>
              <Show when={!loadingApps()}>
                <div class="app-picker-list">
                  <For each={runningApps()}>
                    {(app) => (
                      <div class="app-picker-item" onClick={() => handleAddExcludedApp(app)}>
                        <span class="app-picker-name">{app.name}</span>
                        <span class="app-picker-bundle">{app.bundle_id}</span>
                      </div>
                    )}
                  </For>
                  <Show when={runningApps().length === 0}>
                    <div class="app-picker-empty">没有可添加的运行中应用</div>
                  </Show>
                </div>
              </Show>
            </div>
          </Show>
        </section>
        </Show>

        {/* 数据管理区域 */}
        <Show when={activeTab() === "data"}>
        <section class="settings-section">
          <div class="section-title">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="section-icon">
              <circle cx="12" cy="12" r="10" />
              <polyline points="12 6 12 12 16 14" />
            </svg>
            <span>数据管理</span>
          </div>

          <div class="data-management-card">
            <div class="data-mgmt-row">
              <div class="data-mgmt-info">
                <span class="data-mgmt-label">图片保留时间</span>
                <span class="data-mgmt-desc">无标签图片的自动清理时间</span>
              </div>
              <select
                class="expire-select"
                value={imageExpire()}
                onChange={async (e) => {
                  const days = parseInt(e.currentTarget.value);
                  await setContentTypeExpireDays("image", days);
                  setImageExpire(days);
                }}
              >
                <For each={expireOptions}>
                  {(opt) => <option value={opt.value}>{opt.label}</option>}
                </For>
              </select>
            </div>
            <div class="data-mgmt-row">
              <div class="data-mgmt-info">
                <span class="data-mgmt-label">文字保留时间</span>
                <span class="data-mgmt-desc">无标签文字的自动清理时间</span>
              </div>
              <select
                class="expire-select"
                value={textExpire()}
                onChange={async (e) => {
                  const days = parseInt(e.currentTarget.value);
                  await setContentTypeExpireDays("text", days);
                  setTextExpire(days);
                }}
              >
                <For each={expireOptions}>
                  {(opt) => <option value={opt.value}>{opt.label}</option>}
                </For>
              </select>
            </div>
            <div class="data-mgmt-row">
              <div class="data-mgmt-info">
                <span class="data-mgmt-label">其他类型保留时间</span>
                <span class="data-mgmt-desc">未单独配置的内容类型兑底策略</span>
              </div>
              <select
                class="expire-select"
                value={defaultExpire()}
                onChange={async (e) => {
                  const days = parseInt(e.currentTarget.value);
                  await setDefaultExpireDays(days);
                  setDefaultExpire(days);
                }}
              >
                <For each={expireOptions}>
                  {(opt) => <option value={opt.value}>{opt.label}</option>}
                </For>
              </select>
            </div>
            <div class="data-mgmt-row">
              <div class="data-mgmt-info">
                <span class="data-mgmt-label">最大保存数量</span>
                <span class="data-mgmt-desc">超出后自动清理最早的未收藏条目（-1 = 不限制）</span>
              </div>
              <input
                type="number"
                class="expire-select"
                min="-1"
                step="1"
                value={maxItems()}
                onChange={async (e) => {
                  let count = parseInt(e.currentTarget.value);
                  if (isNaN(count)) count = 1000;
                  if (count < 0) count = -1;
                  await setMaxItems(count);
                  setMaxItemsSignal(count);
                  e.currentTarget.value = String(count);
                }}
              />
            </div>
            <p class="data-mgmt-note">设置过大或不限制会导致历史条目持续堆积，占用磁盘空间并拖慢启动与搜索速度，建议保持在 1000 以内。</p>
            <p class="data-mgmt-note">置顶条目永不过期，有标签的条目按标签规则配置过期</p>
          </div>
        </section>
        </Show>

        {/* 关于与更新区域 */}
        <Show when={activeTab() === "about"}>
        <section class="settings-section">
          <div class="section-title">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="section-icon">
              <path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z" />
            </svg>
            <span>关于与更新</span>
          </div>

          <div class="data-management-card">
            <div class="data-mgmt-row">
              <div class="data-mgmt-info">
                <span class="data-mgmt-label">当前版本</span>
                <span class="data-mgmt-desc">Clipboard Ultra v{appVersion()}</span>
              </div>
              <Show when={updateStatus() !== "downloading"}>
                <button class="btn-add-rule" onClick={handleCheckUpdate} disabled={updateStatus() === "checking"}>
                  {updateStatus() === "checking" ? "检查中..." : "检查更新"}
                </button>
              </Show>
            </div>

            <Show when={updateStatus() === "latest"}>
              <p class="settings-success">已是最新版本</p>
            </Show>

            <Show when={updateStatus() === "available"}>
              <div class="update-available">
                <span class="update-available-text">发现新版本 v{updateVersion()}</span>
                <button class="btn-save" onClick={handleInstallUpdate}>下载并安装</button>
              </div>
            </Show>

            <Show when={updateStatus() === "downloading"}>
              <div class="update-progress">
                <div class="update-progress-bar">
                  <div class="update-progress-fill" style={{ width: `${downloadProgress()}%` }} />
                </div>
                <span class="update-progress-text">下载中 {downloadProgress()}%，完成后将自动重启</span>
              </div>
            </Show>

            <Show when={updateError()}>
              <p class="settings-error">{updateError()}</p>
            </Show>
          </div>
        </section>
        </Show>
      </div>
      </div>
    </div>
  );
};

export default SettingsPage;
