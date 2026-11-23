# FloatNote 悬浮便签

多智能体（或多个会话）并行干活时，经常一转头就忘：这个文件是谁改的？刚才那个 agent 负责哪一步？下一步该谁接手？

FloatNote 是一个常驻桌面的透明悬浮记事本。像桌面歌词一样浮在所有窗口之上，把「谁在做什么、做到哪了」随手记下来，抬眼就能看到，不用切窗口翻聊天记录。

![demo](docs/demo.png)

## 功能

- 常驻显示 agent 分工与进度（planner / coder / reviewer…），不用来回翻会话记录
- 多行纯文本备忘，双击浮窗直接编辑
- 无边框置顶窗口，背景全透明，文字带自动对比色描边
- 左键任意位置拖动；位置、宽度、字号、颜色、透明度自动记忆
- 全局快捷键 `Win+Shift+Q` 隐藏 / 显示面板，干活时随时收起
- 常驻系统托盘：显示/隐藏、重启、退出
- 每次保存自动提交到本地 git 历史；右键可按时间段查看增删记录
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
| `Win+Shift+Q` | 隐藏 / 显示面板（全局生效，隐藏后靠它唤回） |
| `Esc` / `Ctrl+Enter` / 点窗口外 | 保存并退出编辑 |
| 右键 | 字号 / 透明度 / 颜色 / 描边 / 宽度 / 变更记录 / 重置位置 / 退出 |
| 托盘图标 | 单击显示/隐藏；右键菜单可重启或退出 |
| 滚轮 | 内容超长时滚动 |

## 数据文件

都放在程序目录，纯文本：

| 文件 | 内容 |
| --- | --- |
| `notes.txt` | 备忘正文 |
| `config.json` | 窗口位置、宽度、字号、颜色、透明度、描边 |
| `notes-history/` | 本地 git 仓库，每次保存一次提交 |
| `error.log` | 启动异常、git 失败等日志，出错时才生成 |

`notes.txt`、`config.json`、`notes-history/` 都已加入 `.gitignore`，不会提交到代码仓库。备份时复制这些文件/目录即可。

## 笔记历史与变更记录

每次保存（退出编辑）自动把内容提交到本地 git 仓库 `notes-history/`，每一版都能找回，包括删掉的行。

右键菜单 →「查看变更记录…」：弹窗里选择起止时间（默认从第一条记录到现在），按保存时间列出这段时间内的增删内容，可一键复制。也可以直接用 git：

```bash
git -C notes-history log --oneline            # 所有保存记录
git -C notes-history show HEAD~1:notes.txt    # 查看上一次保存的内容
git -C notes-history show <commit>:notes.txt > notes.txt   # 恢复到某个版本
```

想换机器或重装前，复制 `notes.txt` 和 `notes-history/` 即可完整迁移。旧版生成的 `backups/` 已停用，不再更新，可自行删除。

## 开发

```bash
uv run python -m unittest discover -s tests   # 单元测试
uv run python app.py --screenshot out.png     # 渲染自检，输出窗口截图
```

## 项目结构

```
app.py          # 入口，托盘、重启/退出、--screenshot 自检
note_window.py  # 透明浮窗、描边自绘、编辑态、拖动、右键菜单
history_view.py # 变更记录窗口（按时间范围查看增删）
store.py        # notes.txt / config.json / git 历史读写
assets/         # 应用图标
run.vbs         # 静默启动
tests/          # 单元测试
```

## 已知限制

- 内容在退出编辑时才写入文件，编辑过程中强制结束进程可能丢失最后一次改动
- 关闭「变更记录」窗口只关闭它自己，程序继续在托盘运行；退出请用托盘菜单
- 历史仓库都在本机，重装或换机器前需要自行复制 `notes-history/`
- 不支持待办勾选、多便签、鼠标穿透、开机自启
- 宽度通过右键菜单调整，暂不支持拖动边缘缩放