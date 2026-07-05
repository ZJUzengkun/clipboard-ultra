# AI Agent 开发记忆

> 本文件记录 AI 辅助开发过程中积累的关键经验、技术决策和注意事项，供后续会话快速恢复上下文。

## 项目概况

- **当前版本**：v0.7.0
- **技术栈**：Tauri 2 + SolidJS + Rust + SQLite
- **目标平台**：macOS (Apple Silicon) + Windows
- **仓库地址**：`git@github.com:ZJUzengkun/clipboard-ultra.git`（Public）

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

---

## Windows 平台支持（v0.6.0+）

### 实现方案

**前台窗口句柄保存 + SetForegroundWindow 恢复焦点 + enigo Ctrl+V**

流程：
1. 热键触发显示面板前，`GetForegroundWindow` 保存窗口句柄 → `AppState.previous_window_hwnd`
2. 用户选中条目按 Enter → 调用 `paste_item` 命令
3. 写入系统剪贴板 → 隐藏窗口
4. spawn 线程：`SetForegroundWindow` 恢复焦点 → sleep 50ms → enigo Ctrl+V

### 关键代码位置

| 文件 | 职责 |
|------|------|
| `src-tauri/src/clipboard/mod.rs` | `get_frontmost_app_exe()` / `get_foreground_window_handle()` / `restore_foreground_window()` |
| `src-tauri/src/clipboard/monitor.rs` | Windows 排除应用检测（比较 exe 名） |
| `src-tauri/src/commands/mod.rs` | Windows `paste_item` 焦点恢复 + `get_running_apps` EnumWindows 实现 |
| `src-tauri/src/hotkey/mod.rs` | 面板显示前记录前台窗口句柄 |
| `src-tauri/src/lib.rs` | `AppState { previous_window_hwnd: Mutex<usize> }` |
| `src-tauri/Cargo.toml` | `[target.'cfg(target_os = "windows")'.dependencies] windows-sys = "0.59"` |

### Windows 特有问题与解决（v0.6.0 → v0.6.9）

| 问题 | 根因 | 解决方案 | 版本 |
|------|------|----------|------|
| 设置窗口白屏 + 整个应用卡死 | `WebviewWindowBuilder::build()` 在 Windows 上阻塞主线程 | 设置窗口改为 `tauri.conf.json` 预定义，`open_settings` 只调用 `show()` | v0.6.7 |
| 托盘"退出"无响应 | 主线程被 `build()` 阻塞后事件循环停滞 | 托盘退出改用 `std::process::exit(0)` 强制退出 | v0.6.7 |
| 图片显示裂图 | Windows 路径含反斜杠 `\`，`convertFileSrc` 无法正确生成 asset URL | 前端路径统一 `replace(/\\/g, "/")` + asset scope 增加 `C:/**` `D:/**` | v0.6.9 |
| 设置窗口无法关闭 | `decorations(false)` 无系统 X 按钮 | Windows 上启用系统装饰 | v0.6.0 |
| WebView2 白屏 | `transparent: true` 导致 WebView2 渲染异常 | 前端 CSS 平台隔离覆盖实色背景 | v0.6.1 |

---

## 设置窗口架构（v0.6.7 方案）

### 预定义窗口模式

**核心决策**：设置窗口不再运行时动态创建，而是在 `tauri.conf.json` 中预定义，应用启动时由 Tauri 初始化。

```json
// tauri.conf.json → app.windows[1]
{
  "label": "settings",
  "title": "偏好设置",
  "url": "settings.html",
  "width": 580, "height": 620,
  "visible": false,
  "center": true
}
```

**打开逻辑**：
```rust
pub fn open_settings(app_handle: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app_handle.get_webview_window("settings") {
        let _ = window.show();
        let _ = window.set_focus();
        Ok(())
    } else {
        Err("Settings window not found".to_string())
    }
}
```

**关闭逻辑**：前端使用 `getCurrentWindow().hide()` 隐藏（非销毁），确保可重复打开。

### 为什么不用动态创建

在 Windows 上，`WebviewWindowBuilder::new(...).build()` 会阻塞主线程（WebView2 初始化耗时），导致：
- 整个应用事件循环停滞
- 托盘菜单无法响应
- 无法退出应用

预定义窗口由 Tauri 在应用启动时异步完成初始化，完全避免了此问题。

---

## 跨平台样式隔离策略（v0.6.1）

**核心原则：`tauri.conf.json` 保持 `transparent: true`（macOS 需要），Windows 前端覆盖样式**

### 实现路径

1. `src/index.tsx` + `src/settings.tsx`：检测 `navigator.userAgent` 含 "Windows" → `document.documentElement.classList.add("platform-windows")`
2. `src/styles/global.css`：`.platform-windows body` 和 `.platform-windows .app` 设置实色背景 + 禁用 backdrop-filter

### 平台效果对比

| | macOS | Windows |
|---|---|---|
| 背景 | 毛玻璃透明 + backdrop-filter blur | 实色 `#1e1e2e` |
| 设置窗口 | 自定义标题栏（decorations=false） | 系统标题栏（decorations=true，预定义窗口默认） |
| 粘贴方式 | CGEvent Cmd+V | enigo Ctrl+V |
| 焦点恢复 | osascript activate | SetForegroundWindow |
| 排除应用检测 | bundle ID | exe 名称 |
| 图片路径 | 正斜杠 `/` 原生兼容 | 需 `replace(/\\/g, "/")` 标准化 |

### 重要约束

- **任何 Windows 改动禁止影响 macOS**
- Rust 层用 `#[cfg(target_os = "...")]` 条件编译
- 前端层用 `.platform-windows` CSS 类隔离

---

## 图片显示架构

### 存储流程

1. `monitor.rs` 检测剪贴板图片 → 转 PNG 字节
2. 计算 SHA256 哈希去重
3. 保存原图 `{blobs_dir}/{uuid}.png` + 缩略图 `{uuid}_thumb.png`（宽度 ≤200px）
4. 数据库 `blob_path` 字段存文件名（相对路径）

### 显示流程

1. 前端调用 `getBlobsDir()` 获取绝对路径
2. `ClipboardItem.tsx` 拼接完整路径 → **统一正斜杠** → `convertFileSrc()` 转为 asset 协议 URL
3. `<img src={thumbSrc()} />` 加载缩略图

### Windows 路径注意

```typescript
// 必须！Windows 返回的路径含反斜杠
const fullPath = `${blobsDir}/${thumbName}`.replace(/\\/g, "/");
return convertFileSrc(fullPath);
```

asset 协议 scope 需覆盖 Windows 驱动器：
```json
"scope": ["**", "$APPDATA/**", "$HOME/**", "C:/**", "D:/**"]
```

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
- `Swatinem/rust-cache` 使用 `shared-key: release-${{ matrix.platform }}` 跨 job 共享
- 效果：sccache 100% hit rate，构建从 ~8min 降到 ~3min

### 构建矩阵

- macOS: `macos-latest` + `--target aarch64-apple-darwin`
- Windows: `windows-latest`
- 代码签名步骤仅 macOS 执行（`if: matrix.platform == 'macos-latest'`）

### macOS codesign 防卡死（v0.6.3）

keychain 默认 5 分钟无访问后自动锁定，构建耗时超过后 codesign 等待交互密码输入 → CI 无限挂起。

修复：
```yaml
# 导入证书后禁用自动锁定
security set-keychain-settings build.keychain  # 不带 -t 参数

# 签名前重新解锁
security unlock-keychain -p actions build.keychain

# 兜底超时
timeout-minutes: 2
continue-on-error: true
```

### 容错降级

- `cache-build` job 设置 `continue-on-error: true`
- `build` job 先尝试 sccache，失败后 fallback 到无缓存编译

### 跳过 CI

commit message 中加 `[skip ci]` 可跳过 workflow（如纯文档更新）。

### Windows CI 兼容性

- Windows runner 使用 PowerShell，`ls -la` 等 Unix 命令不可用
- 需用跨平台写法或 `if: matrix.platform != 'windows-latest'` 条件

---

## 版本发布规范

1. 三个文件同步更新版本号：`package.json`、`src-tauri/Cargo.toml`、`src-tauri/tauri.conf.json`
2. **注意 JSON 逗号**：修改 version 字段后确保行尾有逗号
3. 每次实质性改动递增 patch 版本号
4. `git tag vx.y.z && git push origin main --tags`
5. 版本号递增和推送必须征得用户确认
6. **严禁删除重打 tag** — 已发布的版本号不可复用

---

## Tauri 2 注意事项

- `capabilities/default.json` 中 `windows` 数组必须包含所有需要权限的窗口名（如 `["main", "settings"]`）
- 设置窗口关闭需要 `core:window:allow-close` 和 `core:event:allow-emit` 权限
- 关闭按钮父容器需加 `data-tauri-drag-region="false"` 防止拖拽区域拦截点击
- ESC 关闭需全局 `keydown` 监听（录制快捷键时排除）
- **多窗口应用推荐使用预定义窗口**（`tauri.conf.json`），避免运行时 `build()` 在 Windows 上阻塞
- Vite 多页构建：`rollupOptions.input` 配置多个 HTML 入口，Tauri 会正确打包所有产出文件

---

## macOS 特有问题

- 未签名 .dmg 需要 `xattr -cr /Applications/clipboard-ultra.app` 绕过 Gatekeeper
- 闪退日志查看：`Console.app` 或 `log show --predicate 'process == "clipboard-ultra"' --last 5m`
- Accessibility 权限：系统设置 → 隐私与安全 → 辅助功能
- Automation 权限：系统设置 → 隐私与安全 → 自动化（System Events 需单独授权）

---

## Windows 特有问题

- **WebView2 动态创建会阻塞主线程** → 使用预定义窗口
- WebView2 不支持 `transparent: true`（会导致事件异常和白屏）→ 前端 CSS 覆盖
- `decorations(false)` 无系统关闭按钮 → 设置窗口使用系统装饰（预定义窗口默认行为）
- 排除应用检测使用 exe 文件名（小写比较）
- `EnumWindows` 获取运行应用列表（callback 内部独立 import）
- asset 协议路径必须用正斜杠，Windows 返回的路径需 `replace(/\\/g, "/")`
- DevTools 在 release 构建默认禁用，需 Cargo.toml 启用 `devtools` feature + 代码调用 `window.open_devtools()`
- 调试日志写到 exe 同目录 `debug.log`（如 `D:\tools\clipboard-ultra\debug.log`）

---

## 版本号管理工具

### 一键同步脚本

`scripts/sync-version.js` 支持自动递增和直接指定：

```bash
npm run version -- patch    # 0.7.0 -> 0.7.1
npm run version -- minor    # 0.7.0 -> 0.8.0
npm run version -- major    # 0.7.0 -> 1.0.0
npm run version -- 1.2.3    # 直接指定
npm run version             # 显示当前版本
```

同步更新三个文件：`package.json`、`src-tauri/Cargo.toml`、`src-tauri/tauri.conf.json`

### 版本规划路线

| 版本 | 目标 |
|------|------|
| v0.7.x | 图标更换 + macOS 粘贴实机验证 + 日常使用稳定性 |
| v0.8.0 | 数据导出/导入 + 自动更新（Tauri updater） |
| v1.0.0 | 生产就绪，上述功能稳定 + 无重大 bug 至少 2-3 周 |
