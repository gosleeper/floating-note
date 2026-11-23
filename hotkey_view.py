from __future__ import annotations

from PySide6.QtCore import Qt, Signal
from PySide6.QtWidgets import (
    QHBoxLayout,
    QLabel,
    QLineEdit,
    QPushButton,
    QVBoxLayout,
    QWidget,
)

from store import (
    DEFAULT_HOTKEY_LABEL,
    DEFAULT_HOTKEY_MODS,
    DEFAULT_HOTKEY_VK,
    MOD_ALT,
    MOD_CONTROL,
    MOD_SHIFT,
    MOD_WIN,
)

SPECIAL_KEYS = {
    0x20: "Space",
    0x09: "Tab",
    0x0D: "Enter",
    0x08: "Backspace",
    0x2D: "Insert",
    0x2E: "Delete",
    0x24: "Home",
    0x23: "End",
    0x21: "PageUp",
    0x22: "PageDown",
    0x25: "Left",
    0x26: "Up",
    0x27: "Right",
    0x28: "Down",
    0xBA: ";",
    0xBB: "=",
    0xBC: ",",
    0xBD: "-",
    0xBE: ".",
    0xBF: "/",
    0xC0: "`",
    0xDB: "[",
    0xDC: "\\",
    0xDD: "]",
    0xDE: "'",
}


def key_name(vk: int) -> str:
    if 0x41 <= vk <= 0x5A or 0x30 <= vk <= 0x39:
        return chr(vk)
    if 0x70 <= vk <= 0x87:
        return f"F{vk - 0x6F}"
    return SPECIAL_KEYS.get(vk, f"VK{vk:02X}")


def combo_label(mods: int, vk: int) -> str:
    parts = []
    if mods & MOD_WIN:
        parts.append("Win")
    if mods & MOD_CONTROL:
        parts.append("Ctrl")
    if mods & MOD_ALT:
        parts.append("Alt")
    if mods & MOD_SHIFT:
        parts.append("Shift")
    parts.append(key_name(vk))
    return "+".join(parts)


def _qt_vk_fallback(qt_key: int) -> int:
    if Qt.Key_A <= qt_key <= Qt.Key_Z or Qt.Key_0 <= qt_key <= Qt.Key_9:
        return int(qt_key)
    if Qt.Key_F1 <= qt_key <= Qt.Key_F24:
        return 0x70 + (int(qt_key) - int(Qt.Key_F1))
    return 0


class HotkeyCapture(QLineEdit):
    def __init__(self, parent=None) -> None:
        super().__init__(parent)
        self.setReadOnly(True)
        self.setPlaceholderText("点击这里后按下组合键")
        self._mods = 0
        self._vk = 0
        self._label = ""

    def keyPressEvent(self, event) -> None:
        if event.key() in (Qt.Key_Control, Qt.Key_Shift, Qt.Key_Alt, Qt.Key_Meta, Qt.Key_Escape):
            event.accept()
            return
        mods = event.modifiers()
        win_mods = 0
        if mods & Qt.ControlModifier:
            win_mods |= MOD_CONTROL
        if mods & Qt.AltModifier:
            win_mods |= MOD_ALT
        if mods & Qt.ShiftModifier:
            win_mods |= MOD_SHIFT
        if mods & Qt.MetaModifier:
            win_mods |= MOD_WIN
        if not win_mods:
            event.accept()
            return
        vk = event.nativeVirtualKey() or _qt_vk_fallback(event.key())
        if not vk:
            event.accept()
            return
        self.set_combo(win_mods, vk)
        event.accept()

    def set_combo(self, mods: int, vk: int, label: str | None = None) -> None:
        self._mods = mods & (MOD_ALT | MOD_CONTROL | MOD_SHIFT | MOD_WIN)
        self._vk = vk
        self._label = label or combo_label(self._mods, self._vk)
        self.setText(self._label)

    def combo(self) -> tuple[int, int, str]:
        return self._mods, self._vk, self._label


class HotkeyWindow(QWidget):
    closed = Signal()

    def __init__(self, mods: int, vk: int, label: str, on_apply) -> None:
        super().__init__()
        self.setWindowTitle("快捷键设置")
        self.setWindowFlags(Qt.Window | Qt.WindowStaysOnTopHint)
        self.resize(480, 190)
        self._on_apply = on_apply

        hint = QLabel("点击下面的输入框，再按下新的组合键。必须包含 Ctrl / Alt / Shift / Win 之一。")
        hint.setWordWrap(True)

        self.capture = HotkeyCapture(self)
        self.capture.set_combo(mods, vk, label)

        self.status = QLabel("", self)
        self.status.setStyleSheet("color: #c0392b;")
        self.status.setWordWrap(True)

        reset = QPushButton("恢复默认", self)
        reset.clicked.connect(
            lambda: self.capture.set_combo(
                DEFAULT_HOTKEY_MODS, DEFAULT_HOTKEY_VK, DEFAULT_HOTKEY_LABEL
            )
        )
        cancel = QPushButton("取消", self)
        cancel.clicked.connect(self.close)
        confirm = QPushButton("确定", self)
        confirm.clicked.connect(self._apply)

        buttons = QHBoxLayout()
        buttons.addWidget(reset)
        buttons.addStretch(1)
        buttons.addWidget(cancel)
        buttons.addWidget(confirm)

        layout = QVBoxLayout(self)
        layout.addWidget(hint)
        layout.addWidget(self.capture)
        layout.addWidget(self.status)
        layout.addLayout(buttons)

    def _apply(self) -> None:
        mods, vk, label = self.capture.combo()
        if not (mods and vk and label):
            self.status.setText("请先按下一组快捷键（需包含 Ctrl / Alt / Shift / Win 之一）")
            return
        if self._on_apply(mods, vk, label):
            self.close()
        else:
            self.status.setText(f"{label} 被其他程序占用，请换一个组合")

    def closeEvent(self, event) -> None:
        self.closed.emit()
        super().closeEvent(event)