# AI Agent 开发记忆

> 本文件记录 AI 辅助开发过程中积累的关键经验、技术决策和注意事项，供后续会话快速恢复上下文。

## 项目概况

- **当前版本**：v0.5.5
- **技术栈**：Tauri 2 + SolidJS + Rust + SQLite
- **目标平台**：macOS (Apple Silicon) + Windows（测试阶段仅构建 macOS aarch64）

## macOS 自动粘贴方案（核心难点）

### 最终方案（v0.5.5）

**osascript activate 恢复焦点 + core-graphics CGEvent 直接发送 Cmd+V**

流程：
1. 热键触发显示面板前，用 osascript 获取前台应用 bundle ID → 保存到 `AppState.previous_app`
2. 用户选中条目按 Enter → 调用 `paste_item` 命令
3. 写入系统剪贴板 → 隐藏窗口
4. spawn 线程：osascript activate 恢复焦点 → sleep 100ms → CGEvent 发 Cmd+V

### 关键代码位置

| 文件 | 职责 |
|------|------|
| `src-tauri/src/commands/mod.rs` | `simulate_paste_macos()` + `paste_item` 命令 |
| `src-tauri/src/hotkey/mod.rs` | `get_frontmost_app_bundle_id()` 面板显示前保存前台 app |
| `src-tauri/src/lib.rs` | `AppState { previous_app: Mutex<Option<String>> }` |
| `src-tauri/Cargo.toml` | `[target.'cfg(target_os = "macos")'.dependencies] core-graphics = "0.24"` |

### 失败方案记录（避免重走弯路）

| 方案 | 失败原因 |
|------|----------|
| enigo + `run_on_main_thread` | 窗口隐藏后 Tauri 主线程回调不执行 |
| AppleScript `System Events keystroke` | 需要 Automation 权限（用户未授予，macOS 可能静默拒绝） |
| enigo 直接调用（非主线程） | `TSMGetInputSourceProperty` 必须主线程，否则 SIGTRAP 崩溃 |

### 待验证

用户需在 macOS 上测试 v0.5.5 构建产物确认 CGEvent 方案是否生效。

---

## 前端交互规范

### 删除键作用域

Backspace/Delete 仅在焦点位于 `.clipboard-list` 区域或 `document.body` 时作为删除快捷键；搜索框等输入控件中保持原生退格功能。

```typescript
case "Backspace":
case "Delete":
  if (document.activeElement === document.body || document.activeElement?.closest(".clipboard-list")) {
    e.preventDefault();
    handleDelete(list[selectedIndex()].id);
  }
  break;
```

### 撤销删除（延迟落库）

- 删除操作仅前端移除（`pendingDeletes` 数组）
- Cmd+Z 弹出最后一个恢复
- 面板隐藏时（失焦/ESC/粘贴）批量 `deleteClipboardItem` 落库

---

## CI/CD 配置要点

### sccache 缓存暖机

- GitHub Actions 缓存按 ref 隔离：tag 间互相读不到，只有 `refs/heads/main` 的缓存全局可读
- 解决方案：`push: branches: [main]` 触发 `cache-build` job 暖缓存
- `Swatinem/rust-cache` 使用 `shared-key: release-macos-aarch64` 跨 job 共享
- 效果：sccache 100% hit rate，构建从 ~8min 降到 ~3min

### 容错降级

- `cache-build` job 设置 `continue-on-error: true`
- `build` job 先尝试 sccache，失败后 fallback 到无缓存编译（`steps.tauri-build.outcome == 'failure'`）

### 跳过 CI

commit message 中加 `[skip ci]` 可跳过 workflow（如纯文档更新）。

---

## 版本发布规范

1. 三个文件同步更新版本号：`package.json`、`src-tauri/Cargo.toml`、`src-tauri/tauri.conf.json`
2. **注意 JSON 逗号**：修改 version 字段后确保行尾有逗号
3. 每次实质性改动递增 patch 版本号
4. `git tag vx.y.z && git push origin main --tags`
5. 版本号递增和推送必须征得用户确认

---

## Tauri 2 注意事项

- `capabilities/default.json` 中 `windows` 数组必须包含所有需要权限的窗口名（如 `["main", "settings"]`）
- 设置窗口关闭需要 `core:window:allow-close` 和 `core:event:allow-emit` 权限
- 关闭按钮父容器需加 `data-tauri-drag-region="false"` 防止拖拽区域拦截点击
- ESC 关闭需全局 `keydown` 监听（录制快捷键时排除）

---

## macOS 特有问题

- 未签名 .dmg 需要 `xattr -cr /Applications/clipboard-pro.app` 绕过 Gatekeeper
- 闪退日志查看：`Console.app` 或 `log show --predicate 'process == "clipboard-pro"' --last 5m`
- Accessibility 权限：系统设置 → 隐私与安全 → 辅助功能
- Automation 权限：系统设置 → 隐私与安全 → 自动化（System Events 需单独授权）
