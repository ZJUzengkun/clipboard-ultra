import { Component, createSignal, onMount, Show } from "solid-js";
import { getShortcut, setShortcut } from "../hooks/useClipboard";

interface SettingsProps {
  onClose: () => void;
}

const Settings: Component<SettingsProps> = (props) => {
  const [currentShortcut, setCurrentShortcut] = createSignal("");
  const [recording, setRecording] = createSignal(false);
  const [recordedKeys, setRecordedKeys] = createSignal("");
  const [saving, setSaving] = createSignal(false);
  const [error, setError] = createSignal("");
  const [success, setSuccess] = createSignal("");

  onMount(async () => {
    try {
      const shortcut = await getShortcut();
      setCurrentShortcut(shortcut);
    } catch (e) {
      console.error("Failed to get shortcut:", e);
    }
  });

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
        </div>
      </div>
    </div>
  );
};

export default Settings;
