# FloatNote 桌面版实施计划

> 使用已审阅设计执行。每部分以分支中的草稿为起点，使用临时数据副本验证。

设计见 [FloatNote Windows / macOS 桌面版设计](../specs/2026-11-23-floatnote-desktop-design.md)。

## 1. 编译和验证窗口

- [ ] 确认 `codex/tauri-desktop` 工作区和现有构建工具。用 `npm run build`、`cargo fetch --manifest-path src-tauri/Cargo.toml`、`cargo check --locked --manifest-path src-tauri/Cargo.toml` 定位原生构建阻塞。
- [ ] 检查 `src-tauri/src/main.rs`、`src-tauri/tauri.conf.json`、`src/note.ts` 的浮窗、焦点、托盘、热键和窗口动作。构建后用空行与长文本测试点击、双击、滚动、拖动、退出编辑。热键唤起时继续向当前应用输入，双击后能输入与保存中文。
- [ ] 用临时 `FLOATNOTE_DATA_DIR` 跑窗口验证。记录 Windows Release 的实测结果和仍需要的 macOS 真机用例。分别验证工具窗口关闭和浮窗隐藏。

## 2. 完成升级和保存

- [ ] 完成 `src-tauri/src/storage.rs` 的数据导入。测试夹具创建含两次提交的旧便携目录，导入后校验正文、配置和 Git HEAD，模拟目标已有笔记和导入中断。
- [ ] 设置窗口提供选目录、查看冲突、确认导入以及恢复提示。数据副本成功切换后沿用浮窗操作和历史查询。
- [ ] 接通保存、热键冲突、重启和退出。观察同时失焦与隐藏只写入一次笔记，Git 缺失时正文仍可读，磁盘写入失败时草稿还能重试。保持旧版历史可读取。

## 3. 验证发行产物

- [ ] 提交锁定依赖的 CI 配置，按 Windows x64、macOS Apple Silicon 和 Intel 分别构建。运行 `npm run build`、`cargo fmt --check --manifest-path src-tauri/Cargo.toml`、`cargo test --locked --manifest-path src-tauri/Cargo.toml`、`npm run tauri build -- --bundles nsis -- --locked`（Windows）。
- [ ] 在 README 补上开发、发行、迁移和兼容说明。用同一段笔记对比 Windows 旧便携版和新版 Release 的体积、启动和常驻资源。macOS 按实际设备与 CI 结果记录。
- [ ] 从该分支的通过构建下载产物，核对架构与 SHA-256，使用 `gh release create` 发布 v0.3.0 并验证下载页。
