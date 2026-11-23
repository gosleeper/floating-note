# FloatNote 悬浮便签

Windows 桌面歌词式悬浮记事本：无边框、背景透明、永远置顶，把常用备忘一直浮在屏幕上。

![demo](docs/demo.png)

## 功能

- 多行纯文本备忘，双击浮窗直接编辑
- 无边框置顶窗口，背景全透明，文字带自动对比色描边
- 左键任意位置拖动；位置、宽度、字号、颜色、透明度自动记忆
- 右键调整字号（12–36）、透明度（30%–100%）、文字颜色、描边开关、宽度
- 高度随内容增长，超过屏幕可用高度 80% 后内部滚动
- 数据全部保存在本地纯文本文件中，不上传任何内容

## 环境

- Windows 10 / 11
- Python 3.11+
- [uv](https://docs.astral.sh/uv/)

## 快速开始

```bash
git clone https://github.com/gosleeper/floating-note.git
cd floating-note
uv sync
```

双击 `run.vbs` 启动（无控制台窗口）。想放桌面的话，创建指向 `run.vbs` 的快捷方式即可。

开发调试可前台运行：

```bash
uv run python app.py
```

## 使用

| 操作 | 行为 |
| --- | --- |
| 左键按住拖动 | 移动窗口 |
| 双击 | 进入编辑 |
| `Esc` / `Ctrl+Enter` / 点窗口外 | 保存并退出编辑 |
| 右键 | 字号 / 透明度 / 颜色 / 描边 / 宽度 / 重置位置 / 退出 |
| 滚轮 | 内容超长时滚动 |

## 数据文件

都放在程序目录，纯文本：

| 文件 | 内容 |
| --- | --- |
| `notes.txt` | 备忘正文 |
| `config.json` | 窗口位置、宽度、字号、颜色、透明度、描边 |
| `error.log` | 启动异常日志，仅出错时生成 |

`notes.txt` 和 `config.json` 已加入 `.gitignore`，不会提交到仓库。备份时复制这两个文件即可。

## 开发

```bash
uv run python -m unittest discover -s tests   # 单元测试
uv run python app.py --screenshot out.png     # 渲染自检，输出窗口截图
```

## 项目结构

```
app.py          # 入口，--screenshot 自检，error.log
note_window.py  # 透明浮窗、描边自绘、编辑态、拖动、右键菜单
store.py        # notes.txt / config.json 读写
run.vbs         # 静默启动
tests/          # 单元测试
```

## 已知限制

- 内容在退出编辑时才写入文件，编辑过程中强制结束进程可能丢失最后一次改动
- 不支持待办勾选、多便签、鼠标穿透、开机自启
- 宽度通过右键菜单调整，暂不支持拖动边缘缩放