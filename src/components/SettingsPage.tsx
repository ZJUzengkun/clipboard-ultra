import { Component, createSignal, onMount, Show, For } from "solid-js";
import { getShortcut, setShortcut, getTagRules, addTagRule, deleteTagRule, TagRule } from "../hooks/useClipboard";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { emit } from "@tauri-apps/api/event";

const SettingsPage: Component = () => {
  const [currentShortcut, setCurrentShortcut] = createSignal("");
  const [recording, setRecording] = createSignal(false);
  const [recordedKeys, setRecordedKeys] = createSignal("");
  const [saving, setSaving] = createSignal(false);
  const [error, setError] = createSignal("");
  const [success, setSuccess] = createSignal("");

  // 标签规则状态
  const [rules, setRules] = createSignal<TagRule[]>([]);
  const [newRuleName, setNewRuleName] = createSignal("");
  const [newRulePattern, setNewRulePattern] = createSignal("");
  const [newRuleColor, setNewRuleColor] = createSignal("#7c6df0");
  const [ruleError, setRuleError] = createSignal("");

  const presetColors = ["#7c6df0", "#f06070", "#f0a050", "#50d0a0", "#5090f0", "#d060d0"];

  onMount(async () => {
    // 初始化主题跟随系统
    const prefersDark = window.matchMedia("(prefers-color-scheme: dark)").matches;
    document.documentElement.setAttribute("data-theme", prefersDark ? "dark" : "light");

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
  });

  const notifyMainWindow = () => {
    // 通知主窗口刷新标签规则
    emit("tag-rules-changed");
  };

  const handleAddRule = async () => {
    const name = newRuleName().trim();
    const pattern = newRulePattern().trim();
    if (!name || !pattern) {
      setRuleError("名称和正则不能为空");
      return;
    }
    try {
      new RegExp(pattern);
    } catch {
      setRuleError("正则表达式格式不合法");
      return;
    }
    setRuleError("");
    try {
      await addTagRule(name, pattern, newRuleColor(), rules().length);
      const updated = await getTagRules();
      setRules(updated);
      setNewRuleName("");
      setNewRulePattern("");
      setNewRuleColor("#7c6df0");
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
    getCurrentWindow().close();
  };

  return (
    <div class="settings-window">
      <div class="settings-titlebar" data-tauri-drag-region>
        <div class="settings-titlebar-left">
          <button class="settings-traffic-btn traffic-close" onClick={handleClose} title="关闭">
            <svg viewBox="0 0 6 6"><path d="M0 0L6 6M6 0L0 6" stroke="currentColor" stroke-width="1.2"/></svg>
          </button>
        </div>
        <span class="settings-titlebar-title">偏好设置</span>
        <div class="settings-titlebar-right" />
      </div>

      <div class="settings-window-body">
        {/* 快捷键区域 */}
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

        {/* 标签规则区域 */}
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
                      <code class="tag-rule-pattern">{rule.pattern}</code>
                    </div>
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
              <input
                type="text"
                placeholder="正则表达式 (如 https?://)"
                value={newRulePattern()}
                onInput={(e) => setNewRulePattern(e.currentTarget.value)}
                class="tag-rule-input input-pattern"
              />
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
      </div>
    </div>
  );
};

export default SettingsPage;
