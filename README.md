# Clipboard Ultra

> **本项目由 AI 辅助编写，完全开源，欢迎贡献！**

一个轻量级跨平台剪贴板历史管理工具，灵感来源于 macOS 上的 [Paste](https://pasteapp.io/)。

## 功能特性

### 已实现（v0.7.0）

- **剪贴板历史记录** — 自动记录复制的文本和图片
- **自动去重** — 完全相同的内容不会重复存储，而是更新时间戳置顶
- **模糊搜索** — 按关键词实时过滤历史记录
- **点击即粘贴** — 选中条目后自动写入剪贴板并粘贴到原应用光标位置
- **收藏/置顶** — 重要内容固定在顶部，不会被清理
- **正则标签筛选** — 基于正则表达式自动打标签，支持按标签快速筛选
- **手动打标** — 用户可对任意条目手动编辑/覆盖标签
- **图片剪贴板** — 支持图片复制记录（本地 blob 存储 + 缩略图预览）
- **按内容类型过期策略** — 图片/文字/其他类型独立配置保留天数
- **卡片差异化配色** — 仿 Paste 风格，标签/类型/内容决定 Header 颜色（8 色轮选）
- **快捷删除与多级撤销** — Backspace 删除，Cmd+Z 多级撤销（延迟落库）
- **全局快捷键** — `Ctrl+Shift+V` / `Cmd+Shift+V` 快速唤起
- **系统托盘** — 后台常驻，右键菜单操作
- **独立设置窗口** — 可配置快捷键、标签规则、过期策略、排除应用
- **排除应用** — 指定应用的复制不记录（macOS bundle ID / Windows exe）
- **跨平台支持** — macOS + Windows 完整功能对等
- **平台样式隔离** — macOS 毛玻璃效果 / Windows 实色背景，互不影响
- **CI 加速** — sccache + rust-cache 暖缓存，构建 ~3 分钟
- **自定义应用图标** — 亮色扁平风格剪贴板图标，全平台适配
- **版本号一键同步** — `npm run version -- patch/minor/major` 自动更新三文件

### 规划中

- [ ] 确认 macOS 自动粘贴（CGEvent）实机可用
- [ ] 数据导出/导入
- [ ] 自动更新（Tauri updater）
- [ ] 富文本支持（HTML/RTF）
- [ ] OCR 识别插件（从图片中提取文字）
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
| 构建 | Vite | 前端开发与打包（多页模式） |
| CI/CD | GitHub Actions | 自动构建 Windows + macOS 安装包 |

## 快速开始

### 环境要求

- Node.js >= 18
- Rust >= 1.70
- 系统依赖（仅 Linux 开发）：`sudo apt install libwebkit2gtk-4.1-dev librsvg2-dev libappindicator3-dev libxdo-dev`

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
# 升版本（自动同步 package.json + Cargo.toml + tauri.conf.json）
npm run version -- patch

# 提交并打 tag
git add -A && git commit -m "chore: bump version to x.y.z"
git tag vx.y.z && git push origin main --tags
```

构建完成后在 [Releases](https://github.com/ZJUzengkun/clipboard-ultra/releases) 页面下载安装包。

## 支持平台

| 平台 | 状态 | 安装包格式 |
|------|------|-----------|
| Windows 10/11 | ✅ | `.msi` / `.exe` |
| macOS (Apple Silicon) | ✅ | `.dmg` |
| macOS (Intel) | ✅ | `.dmg` |
| Linux | 🔧 开发用 | — |

### macOS 安装注意事项

未签名的 `.dmg` 安装后首次运行需执行：
```bash
xattr -cr /Applications/clipboard-ultra.app
```

## 项目结构

```
clipboard-ultra/
├── src/                    # 前端（SolidJS）
│   ├── components/         # UI 组件
│   │   ├── ClipboardItem.tsx   # 剪贴板条目卡片
│   │   ├── ClipboardList.tsx   # 条目列表容器
│   │   ├── SearchBar.tsx       # 搜索栏
│   │   ├── Settings.tsx        # 设置入口
│   │   ├── SettingsPage.tsx    # 设置面板完整页面
│   │   └── TagBar.tsx          # 标签筛选栏
│   ├── hooks/              # 业务逻辑 Hook
│   │   └── useClipboard.ts    # 剪贴板操作 API 封装
│   └── styles/             # 样式
│       └── global.css         # 全局样式（含平台隔离）
├── src-tauri/              # 后端（Rust）
│   └── src/
│       ├── clipboard/      # 剪贴板监听 + 图片处理
│       │   ├── mod.rs         # 平台函数（焦点恢复等）
│       │   └── monitor.rs     # 轮询监听 + 图片保存
│       ├── commands/       # Tauri IPC 命令
│       │   └── mod.rs         # 所有前端可调用的命令
│       ├── db/             # SQLite 数据库
│       │   ├── mod.rs         # 数据库初始化 + schema
│       │   └── operations.rs  # CRUD 操作
│       ├── hotkey/         # 全局快捷键
│       │   └── mod.rs         # 注册/注销 + 焦点记录
│       ├── tray/           # 系统托盘
│       │   └── mod.rs         # 托盘菜单 + 事件
│       └── plugins/        # 插件预留
├── settings.html           # 设置页面入口（Vite 多页）
├── index.html              # 主页面入口
└── .github/workflows/      # CI/CD
    └── release.yml         # 自动构建 + 发布
```

## 版本历史

| 版本 | 主要变更 |
|------|----------|
| v0.7.0 | 更换应用图标（亮色扁平风格）、版本号同步脚本增强、文档清理 |
| v0.6.9 | 修复 Windows 图片裂图（路径分隔符标准化 + asset scope 扩展） |
| v0.6.8 | 同步版本号、移除不兼容的 CI 验证步骤 |
| v0.6.7 | 设置窗口改为预定义（修复 Windows 白屏+卡死）、托盘强制退出 |
| v0.6.6 | 尝试 External URL 加载设置页面（过渡版本） |
| v0.6.5 | 添加 exe 目录日志输出调试 Windows 白屏 |
| v0.6.4 | 启用 DevTools feature 用于 Windows 调试 |
| v0.6.3 | 修复 macOS codesign keychain 自动锁定卡住问题 |
| v0.6.2 | 设置页面内联样式 + debug overlay |
| v0.6.1 | 跨平台样式隔离（macOS 毛玻璃 / Windows 实色） |
| v0.6.0 | Windows 平台完整支持（粘贴、焦点恢复、排除应用） |

## 开源协议

[MIT](LICENSE)

## 致谢

本项目由 AI 辅助完成架构设计与代码实现，人类负责产品规划与最终决策。
