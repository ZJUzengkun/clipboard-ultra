# Clipboard Pro

> **本项目由 AI 辅助编写，完全开源，欢迎贡献！**

一个轻量级跨平台剪贴板历史管理工具，灵感来源于 macOS 上的 [Paste](https://pasteapp.io/)。

## 功能特性

### 已实现（v0.5.5）

- **剪贴板历史记录** — 自动记录复制的文本内容
- **文本自动去重** — 完全相同的内容不会重复存储，而是更新时间戳置顶
- **模糊搜索** — 按关键词实时过滤历史记录（搜索框支持正常编辑操作）
- **点击即粘贴** — 选中条目后自动写入剪贴板并粘贴到原应用光标位置（macOS 使用 CGEvent）
- **收藏/置顶** — 重要内容固定在顶部，不会被清理
- **历史数量限制** — 上限 1000 条，自动清理最旧的非收藏记录
- **全局快捷键** — `Ctrl+Shift+V`（Windows）/ `Cmd+Shift+V`（macOS）快速唤起
- **系统托盘** — 后台常驻，右键菜单操作
- **正则标签筛选** — 基于正则表达式自动对剪贴板条目打标签，支持按标签快速筛选
- **手动打标** — 用户可对任意条目手动编辑/覆盖标签
- **快捷删除与多级撤销** — Backspace/Delete 删除选中项，Cmd+Z 多级撤销（延迟落库，面板关闭前均可恢复）
- **独立设置窗口** — 支持 `Cmd+,` 快捷打开，可配置快捷键与标签正则规则
- **窗口交互优化** — 失焦自动隐藏、ESC 关闭、选中态视觉增强
- **macOS 焦点恢复** — 面板隐藏后自动激活原前台应用
- **CI 加速** — sccache + rust-cache 暖缓存，构建 ~3 分钟（含容错降级）

### 规划中

- [ ] 图片剪贴板支持（本地文件存储 + 缩略图预览）
- [ ] 富文本支持（HTML/RTF）
- [ ] OCR 识别插件（从图片中提取文字）
- [ ] 自定义快捷键
- [ ] 数据导出/导入
- [ ] 多语言支持
- [ ] 主题切换（深色/浅色）
- [ ] 剪贴板同步（跨设备）

## 技术栈

| 层级 | 技术 | 说明 |
|------|------|------|
| 框架 | [Tauri 2](https://v2.tauri.app/) | 跨平台桌面应用框架 |
| 前端 | [SolidJS](https://www.solidjs.com/) + TypeScript | 响应式 UI |
| 后端 | Rust | 系统级能力（剪贴板、快捷键、数据库） |
| 存储 | SQLite（rusqlite bundled） | 内嵌数据库，无需额外安装 |
| 构建 | Vite | 前端开发与打包 |
| CI/CD | GitHub Actions | 自动构建 Windows + macOS 安装包 |

## 快速开始

### 环境要求

- Node.js >= 18
- Rust >= 1.70
- 系统依赖（仅 Linux）：`sudo apt install libwebkit2gtk-4.1-dev librsvg2-dev libappindicator3-dev libxdo-dev`

### 开发

```bash
# 安装前端依赖
npm install

# 启动开发模式（前端热更新 + Rust 增量编译）
npm run tauri dev
```

### 构建

```bash
# 生产构建（输出安装包到 src-tauri/target/release/bundle/）
npm run tauri build
```

### 自动发版

推送 `v*` 格式的 Git tag 会自动触发 GitHub Actions 构建：

```bash
git tag v0.1.0
git push origin v0.1.0
```

构建完成后在 [Releases](https://github.com/ZJUzengkun/clipboard-pro/releases) 页面下载安装包。

## 支持平台

| 平台 | 状态 | 安装包格式 |
|------|------|-----------|
| Windows 10/11 | ✅ | `.msi` / `.exe` |
| macOS (Apple Silicon) | ✅ | `.dmg` |
| macOS (Intel) | ✅ | `.dmg` |
| Linux | 🔧 开发用 | — |

## 项目结构

```
clipboard-pro/
├── src/                    # 前端（SolidJS）
│   ├── components/         # UI 组件
│   ├── hooks/              # 业务逻辑 Hook
│   └── styles/             # 样式
├── src-tauri/              # 后端（Rust）
│   └── src/
│       ├── clipboard/      # 剪贴板监听
│       ├── commands/       # Tauri IPC 命令
│       ├── db/             # SQLite 数据库
│       ├── hotkey/         # 全局快捷键
│       ├── tray/           # 系统托盘
│       └── plugins/        # 插件预留
└── .github/workflows/      # CI/CD
```

## 开源协议

[MIT](LICENSE)

## 致谢

本项目由 AI 辅助完成架构设计与代码实现，人类负责产品规划与最终决策。
