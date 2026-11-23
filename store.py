from __future__ import annotations

import json
import os
import re
import tempfile
import time
from dataclasses import dataclass, replace
from pathlib import Path

APP_DIR = Path(__file__).resolve().parent
NOTES_FILE = APP_DIR / "notes.txt"
CONFIG_FILE = APP_DIR / "config.json"

DEFAULT_TEXT = "双击这里开始记录\n右键调整样式"

WIDTH_RANGE = (240, 1200)
FONT_RANGE = (10, 64)
OPACITY_RANGE = (0.2, 1.0)


@dataclass
class Config:
    x: int | None = None
    y: int | None = None
    width: int = 360
    font_size: int = 18
    color: str = "#ffffff"
    opacity: float = 1.0
    outline: bool = True


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