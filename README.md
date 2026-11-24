# FloatNote 悬浮便签

FloatNote 是一张常驻桌面的透明便签。适合在编辑器、浏览器和多个 agent 会话之间记录分工、当前进度和下一步。阅读时只显示描边文字，双击即可在原位补一句。

![便签浮在代码窗口上](docs/why-transparent.png)

## 下载与安装

从 [GitHub Releases](https://github.com/gosleeper/floating-note/releases) 下载对应系统和芯片的文件。

| 平台 | 文件 | 使用方式 |
| --- | --- | --- |
| Windows x64 | `*-setup.exe` | 运行安装程序。缺少 WebView2 时安装程序会联网安装它。 |
| Windows x64 便携版 | `*-portable.exe` | 本机已有 WebView2 时可以直接运行。升级旧便携版时，先退出旧程序，再把新版 EXE 放到原数据目录。 |
| macOS Apple Silicon | `*-aarch64.dmg` | 打开磁盘映像，把 FloatNote 拖到“应用程序”。 |
| macOS Intel | `*-x64.dmg` | 在 Intel Mac 上打开磁盘映像，把 FloatNote 拖到“应用程序”。 |

以 Release 页标注的签名状态为准。临时签名的 macOS 包首次打开时，系统可能要求你在“系统设置 → 隐私与安全性”中选择“仍要打开”。[Apple 的首次打开说明](https://support.apple.com/guide/mac-help/open-a-mac-app-from-an-unidentified-developer-mh40616/mac)

## 使用

| 操作 | 行为 |
| --- | --- |
| 拖动文字或便签内空白处 | 移动窗口 |
| 双击便签 | 编辑正文 |
| Esc、Ctrl+Enter 或点到其他窗口 | 保存并结束编辑，macOS 也可按 Cmd+Enter |
| Win+Shift+Q / Control+Option+N | 在 Windows / macOS 上显示或隐藏便签，可在快捷键设置中修改 |
| 右键便签 | 打开样式、快捷键、历史、数据导入或重置位置 |
| Windows 托盘 / macOS 菜单栏 | 显示或隐藏便签，打开设置，重启或退出 |
| 滚轮 | 当正文超过屏幕可用高度的 80% 时滚动 |

## 数据和升级

笔记保存在本机。全新安装使用当前账户的应用数据目录，可从“数据与导入”查看路径。

- `notes.txt` 存放 UTF-8 正文。
- `config.json` 存放外观、位置和快捷键。
- `notes-history/` 存放本地 Git 历史。系统有 Git 时，保存后会记录版本，可按时间范围查看和复制增删内容。

从 0.2.x 导入时，在“数据与导入”选择包含上述文件的旧版目录，预览后确认。导入会保留来源和当前数据，重启后打开导入副本。系统 Git 缺失时仍可读写正文，历史功能在安装 Git 后可用。备份请复制整个当前数据目录。

## 开发与构建

准备 Node.js 22.12+、Rust stable 和对应系统的 [Tauri 构建依赖](https://v2.tauri.app/start/prerequisites/)。Windows 使用 MSVC、Windows SDK 和 WebView2，macOS 使用 Xcode 命令行工具。

```bash
npm ci
npm run tauri dev
npm run build
cargo test --locked --manifest-path src-tauri/Cargo.toml
```

本地构建 Windows 安装包：

```powershell
npm run tauri build -- --bundles nsis -- --locked
```

在 Mac 上构建当前架构的磁盘映像：

```bash
npm run tauri build -- --bundles dmg -- --locked
```

CI 对 Windows x64、macOS Apple Silicon 和 Intel 分别构建。开发与测试可通过 `FLOATNOTE_DATA_DIR` 使用独立目录。需求与验收标准见 [桌面版设计](docs/superpowers/specs/2026-11-23-floatnote-desktop-design.md)。

原 Windows 版的 Python 源码仍可用于对照。它使用 `uv sync` 安装依赖，`uv run python app.py` 启动。
