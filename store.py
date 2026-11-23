from __future__ import annotations

import json
import os
import re
import subprocess
import sys
import tempfile
import time
from dataclasses import dataclass, replace
from datetime import datetime
from pathlib import Path

if getattr(sys, "frozen", False):
    APP_DIR = Path(sys.executable).resolve().parent
else:
    APP_DIR = Path(__file__).resolve().parent
NOTES_FILE = APP_DIR / "notes.txt"
CONFIG_FILE = APP_DIR / "config.json"
HISTORY_DIR = APP_DIR / "notes-history"

DEFAULT_TEXT = "双击这里开始记录\n右键调整样式"

WIDTH_RANGE = (240, 1200)
FONT_RANGE = (10, 64)
OPACITY_RANGE = (0.2, 1.0)

MOD_ALT = 0x0001
MOD_CONTROL = 0x0002
MOD_SHIFT = 0x0004
MOD_WIN = 0x0008
MOD_NOREPEAT = 0x4000
HOTKEY_MODS_MASK = MOD_ALT | MOD_CONTROL | MOD_SHIFT | MOD_WIN

DEFAULT_HOTKEY_MODS = MOD_WIN | MOD_SHIFT
DEFAULT_HOTKEY_VK = 0x51  # Q
DEFAULT_HOTKEY_LABEL = "Win+Shift+Q"


@dataclass
class Config:
    x: int | None = None
    y: int | None = None
    width: int = 360
    font_size: int = 18
    color: str = "#ffffff"
    opacity: float = 1.0
    outline: bool = True
    hotkey_mods: int = DEFAULT_HOTKEY_MODS
    hotkey_vk: int = DEFAULT_HOTKEY_VK
    hotkey_label: str = DEFAULT_HOTKEY_LABEL


def _clamp(value, low, high):
    return max(low, min(high, value))


def _as_int(value):
    return value if isinstance(value, int) and not isinstance(value, bool) else None


def config_from_dict(raw: dict) -> Config:
    cfg = Config()
    if not isinstance(raw, dict):
        return cfg

    x = _as_int(raw.get("x"))
    y = _as_int(raw.get("y"))
    if x is not None and y is not None:
        cfg.x, cfg.y = x, y

    width = _as_int(raw.get("width"))
    if width is not None:
        cfg.width = _clamp(width, *WIDTH_RANGE)

    font_size = _as_int(raw.get("font_size"))
    if font_size is not None:
        cfg.font_size = _clamp(font_size, *FONT_RANGE)

    color = raw.get("color")
    if isinstance(color, str) and re.fullmatch(r"#[0-9a-fA-F]{6}", color):
        cfg.color = color.lower()

    opacity = raw.get("opacity")
    if isinstance(opacity, (int, float)) and not isinstance(opacity, bool):
        cfg.opacity = round(_clamp(float(opacity), *OPACITY_RANGE), 2)

    outline = raw.get("outline")
    if isinstance(outline, bool):
        cfg.outline = outline

    hotkey_mods = _as_int(raw.get("hotkey_mods"))
    hotkey_vk = _as_int(raw.get("hotkey_vk"))
    hotkey_label = raw.get("hotkey_label")
    if (
        hotkey_mods is not None
        and hotkey_mods & HOTKEY_MODS_MASK
        and hotkey_vk is not None
        and 0 < hotkey_vk <= 0xFF
        and isinstance(hotkey_label, str)
        and 0 < len(hotkey_label) <= 40
    ):
        cfg.hotkey_mods = hotkey_mods & HOTKEY_MODS_MASK
        cfg.hotkey_vk = hotkey_vk
        cfg.hotkey_label = hotkey_label

    return cfg


def _write_atomic(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    fd, tmp_name = tempfile.mkstemp(dir=str(path.parent), prefix=path.name + ".", suffix=".tmp")
    try:
        with os.fdopen(fd, "w", encoding="utf-8", newline="") as handle:
            handle.write(text)
        os.replace(tmp_name, path)
    except BaseException:
        try:
            os.unlink(tmp_name)
        except OSError:
            pass
        raise


def load_config(path: Path = CONFIG_FILE) -> Config:
    path = Path(path)
    if not path.exists():
        return Config()
    try:
        raw = json.loads(path.read_text(encoding="utf-8"))
        return config_from_dict(raw)
    except (json.JSONDecodeError, OSError):
        try:
            backup = path.with_suffix(path.suffix + f".corrupt-{int(time.time())}")
            path.replace(backup)
        except OSError:
            pass
        return Config()


def save_config(cfg: Config, path: Path = CONFIG_FILE) -> None:
    payload = {
        "x": cfg.x,
        "y": cfg.y,
        "width": cfg.width,
        "font_size": cfg.font_size,
        "color": cfg.color,
        "opacity": cfg.opacity,
        "outline": cfg.outline,
        "hotkey_mods": cfg.hotkey_mods,
        "hotkey_vk": cfg.hotkey_vk,
        "hotkey_label": cfg.hotkey_label,
    }
    _write_atomic(Path(path), json.dumps(payload, ensure_ascii=False, indent=2))


def load_notes(path: Path = NOTES_FILE) -> str:
    path = Path(path)
    if not path.exists():
        save_notes(DEFAULT_TEXT, path)
        return DEFAULT_TEXT
    try:
        return path.read_text(encoding="utf-8-sig", errors="replace")
    except OSError:
        return DEFAULT_TEXT


def save_notes(text: str, path: Path = NOTES_FILE) -> None:
    _write_atomic(Path(path), text)


def with_changes(cfg: Config, **changes) -> Config:
    return replace(cfg, **changes)


GIT_NO_WINDOW = 0x08000000 if os.name == "nt" else 0


def _log_error(message: str) -> None:
    try:
        with (APP_DIR / "error.log").open("a", encoding="utf-8") as handle:
            handle.write(f"[{datetime.now():%Y-%m-%d %H:%M:%S}] {message}\n")
    except OSError:
        pass


def _git(args: list[str], cwd: Path, env: dict | None = None):
    try:
        return subprocess.run(
            ["git", *args],
            cwd=str(cwd),
            capture_output=True,
            encoding="utf-8",
            errors="replace",
            creationflags=GIT_NO_WINDOW,
            env=env,
        )
    except OSError as exc:
        _log_error(f"git 不可用: {exc}")
        return None


def _commit_history(history_dir: Path, text: str, stamp: datetime) -> None:
    history_dir = Path(history_dir)
    try:
        _write_atomic(history_dir / "notes.txt", text)
    except OSError as exc:
        _log_error(f"写历史副本失败: {exc}")
        return

    if not (history_dir / ".git").exists():
        result = _git(["init", "-q"], history_dir)
        if result is None or result.returncode != 0:
            _log_error("git init 失败")
            return

    _git(["add", "notes.txt"], history_dir)
    diff = _git(["diff", "--cached", "--quiet"], history_dir)
    if diff is None or diff.returncode == 0:
        return

    env = os.environ.copy()
    env["GIT_AUTHOR_DATE"] = stamp.strftime("%Y-%m-%dT%H:%M:%S")
    env["GIT_COMMITTER_DATE"] = env["GIT_AUTHOR_DATE"]
    result = _git(
        [
            "-c", "user.name=FloatNote",
            "-c", "user.email=floatnote@local",
            "commit", "-q", "-m",
            f"notes {stamp:%Y-%m-%d %H:%M:%S}",
        ],
        history_dir,
        env=env,
    )
    if result is None or result.returncode != 0:
        detail = result.stderr.strip() if result else ""
        _log_error(f"git commit 失败: {detail}")


def start_history(
    text: str,
    *,
    history_dir: Path = HISTORY_DIR,
    now: datetime | None = None,
) -> None:
    _commit_history(history_dir, text, now or datetime.now())


def record_history(
    new_text: str,
    *,
    notes_path: Path = NOTES_FILE,
    history_dir: Path = HISTORY_DIR,
    now: datetime | None = None,
) -> None:
    stamp = now or datetime.now()
    save_notes(new_text, notes_path)
    _commit_history(history_dir, new_text, stamp)


def history_bounds(
    *, history_dir: Path = HISTORY_DIR
) -> tuple[datetime | None, datetime | None]:
    if not (Path(history_dir) / ".git").exists():
        return None, None
    result = _git(["log", "--format=%ct"], history_dir)
    if result is None or result.returncode != 0:
        return None, None
    stamps = sorted(int(v) for v in result.stdout.split() if v.strip())
    if not stamps:
        return None, None
    return datetime.fromtimestamp(stamps[0]), datetime.fromtimestamp(stamps[-1])


def history_increment(
    start: datetime,
    end: datetime,
    *,
    history_dir: Path = HISTORY_DIR,
) -> str:
    header = f"{start:%Y-%m-%d %H:%M:%S} ~ {end:%Y-%m-%d %H:%M:%S}"
    if not (Path(history_dir) / ".git").exists():
        return f"{header}\n\n还没有保存历史。退出一次编辑后就会有记录。"

    result = _git(
        ["log", "--reverse", "--format=%x01%H%x09%ct", "-p", "--unified=0", "--no-color"],
        Path(history_dir),
    )
    if result is None or result.returncode != 0:
        return f"{header}\n\n读取历史失败，详情见 error.log。"

    start_epoch = int(start.timestamp())
    end_epoch = int(end.timestamp())

    blocks = []
    current = None
    for raw in result.stdout.splitlines():
        if raw.startswith("\x01"):
            if current is not None:
                blocks.append(current)
            parts = raw[1:].split("\t")
            try:
                epoch = int(parts[1])
            except (IndexError, ValueError):
                epoch = 0
            current = {"epoch": epoch, "added": [], "removed": []}
            continue
        if current is None:
            continue
        if raw.startswith("+"):
            if raw.startswith("+++ ") or raw.startswith("+++b/"):
                continue
            current["added"].append(raw[1:])
        elif raw.startswith("-"):
            if raw.startswith("--- ") or raw.startswith("---a/"):
                continue
            current["removed"].append(raw[1:])
    if current is not None:
        blocks.append(current)

    lines = [f"变更范围：{header}", ""]
    shown = 0
    for block in blocks:
        if not (start_epoch <= block["epoch"] <= end_epoch):
            continue
        if not block["added"] and not block["removed"]:
            continue
        stamp = datetime.fromtimestamp(block["epoch"])
        lines.append(f"[{stamp:%Y-%m-%d %H:%M:%S}]")
        for item in block["added"]:
            if item.strip():
                lines.append(f"  + {item}")
        for item in block["removed"]:
            if item.strip():
                lines.append(f"  - {item}")
        lines.append("")
        shown += 1
        if len(lines) > 4000:
            lines.append("……内容过长已截断，可在 notes-history 里用 git log 查看完整历史。")
            break

    if shown == 0:
        lines.append("该时间段没有变更记录。")
    return "\n".join(lines).rstrip() + "\n"