import { Component, createSignal, onMount, Show, For } from "solid-js";
import { getShortcut, setShortcut, getTagRules, addTagRule, deleteTagRule, TagRule } from "../hooks/useClipboard";

interface SettingsProps {
  onClose: () => void;
  onTagRulesChanged?: () => void;
}

const Settings: Component<SettingsProps> = (props) => {
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

  const handleAddRule = async () => {
    const name = newRuleName().trim();
    const pattern = newRulePattern().trim();
    if (!name || !pattern) {
      setRuleError("名称和正则不能为空");
      return;
    }
    // 验证正则合法性
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
      props.onTagRulesChanged?.();
    } catch (e: any) {
      setRuleError(`添加失败: ${e}`);
    }
  };

  const handleDeleteRule = async (id: number) => {
    try {
      await deleteTagRule(id);
      const updated = await getTagRules();
      setRules(updated);
      props.onTagRulesChanged?.();
    } catch (e) {
      console.error("Failed to delete rule:", e);
    }
  };

  const addPreset = (name: string, pattern: string) => {
    setNewRuleName(name);
    setNewRulePattern(pattern);
  };

  // 将按键事件转换为 Tauri 快捷键字符串格式
  const keyEventToShortcut = (e: KeyboardEvent): string => {
    const parts: string[] = [];

    if (e.ctrlKey || e.metaKey) parts.push("CommandOrControl");
    if (e.altKey) parts.push("Alt");
    if (e.shiftKey) parts.push("Shift");

    // 获取实际按键（非修饰键）
    const key = e.key;
    if (!["Control", "Meta", "Alt", "Shift"].includes(key)) {
      // 特殊键映射
      const keyMap: Record<string, string> = {
        " ": "Space",
        ArrowUp: "Up",
        ArrowDown: "Down",
        ArrowLeft: "Left",
        ArrowRight: "Right",
        Escape: "Escape",
        Enter: "Enter",
        Backspace: "Backspace",
        Delete: "Delete",
        Tab: "Tab",
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
    // 只有当包含修饰键+普通键时才算有效
    if (shortcut.includes("+") && !["CommandOrControl", "Alt", "Shift"].includes(shortcut)) {
      setRecordedKeys(shortcut);
    }
  };

  const handleKeyUp = (e: KeyboardEvent) => {
    if (!recording()) return;
    e.preventDefault();
    // 当所有修饰键释放时，如果有记录到快捷键则停止录制
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

  return (
    <div class="settings-overlay" onClick={props.onClose}>
      <div class="settings-panel" onClick={(e) => e.stopPropagation()}>
        <div class="settings-header">
          <h3>设置</h3>
          <button class="btn-close" onClick={props.onClose}>
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M18 6L6 18M6 6l12 12" />
            </svg>
          </button>
        </div>

        <div class="settings-content">
          <div class="settings-section">
            <label class="settings-label">唤起快捷键</label>
            <div class="shortcut-display">
              <span class="current-shortcut">{formatShortcut(currentShortcut())}</span>
            </div>

            <div class="shortcut-recorder">
              <Show
                when={recording()}
                fallback={
                  <button class="btn-record" onClick={startRecording}>
                    点击录制新快捷键
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

          {/* 标签规则管理 */}
          <div class="settings-section" style="margin-top: 20px">
            <label class="settings-label">标签规则</label>

            {/* 已有规则列表 */}
            <Show when={rules().length > 0}>
              <div class="tag-rules-list">
                <For each={rules()}>
                  {(rule) => (
                    <div class="tag-rule-row">
                      <span class="tag-dot" style={{ background: rule.color }}></span>
                      <span class="tag-rule-name">{rule.name}</span>
                      <span class="tag-rule-pattern">{rule.pattern}</span>
                      <button class="btn-action btn-delete" onClick={() => handleDeleteRule(rule.id)} title="删除">
                        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                          <path d="M18 6L6 18M6 6l12 12" />
                        </svg>
                      </button>
                    </div>
                  )}
                </For>
              </div>
            </Show>

            {/* 新增规则表单 */}
            <div class="tag-rule-form">
              <input
                type="text"
                placeholder="标签名称"
                value={newRuleName()}
                onInput={(e) => setNewRuleName(e.currentTarget.value)}
                class="tag-rule-input"
              />
              <input
                type="text"
                placeholder="正则表达式"
                value={newRulePattern()}
                onInput={(e) => setNewRulePattern(e.currentTarget.value)}
                class="tag-rule-input"
              />
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
              <button class="btn-save" onClick={handleAddRule} style="width: 100%">
                添加规则
              </button>
            </div>

            {/* 预设示例 */}
            <div class="tag-presets">
              <span class="tag-preset-label">快捷预设:</span>
              <button class="tag-preset-btn" onClick={() => addPreset("链接", "https?://")}>URL</button>
              <button class="tag-preset-btn" onClick={() => addPreset("邮箱", "[\\w.-]+@[\\w.-]+\\.\\w+")}>邮箱</button>
              <button class="tag-preset-btn" onClick={() => addPreset("代码", "^(import|function|const|class|def|pub fn)")}>代码</button>
            </div>

            <Show when={ruleError()}>
              <p class="settings-error">{ruleError()}</p>
            </Show>
          </div>
        </div>
      </div>
    </div>
  );
};

export default Settings;
