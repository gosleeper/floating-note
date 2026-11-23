from __future__ import annotations

import sys
from pathlib import Path


def _restart(window) -> None:
    from PySide6.QtCore import QProcess
    from PySide6.QtWidgets import QApplication, QMessageBox

    base = Path(__file__).resolve().parent
    launcher = base / ".venv" / "Scripts" / "pythonw.exe"
    program = str(launcher) if launcher.exists() else sys.executable
    # 先释放全局热键，避免新实例注册失败
    window.release_hotkey()
    if QProcess.startDetached(program, [str(base / "app.py")], str(base)):
        QApplication.instance().quit()
    else:
        window.restore_hotkey()
        QMessageBox.warning(None, "FloatNote", "重启失败，请手动退出后重新打开。")


def main() -> int:
    from PySide6.QtCore import QTimer
    from PySide6.QtGui import QIcon
    from PySide6.QtWidgets import QApplication, QMenu, QSystemTrayIcon

    from note_window import NoteWindow
    from store import load_config, load_notes, record_history, save_config, start_history

    app = QApplication(sys.argv)
    app.setApplicationName("FloatNote")

    base = Path(__file__).resolve().parent
    icon = QIcon(str(base / "assets" / "floatnote.ico"))
    app.setWindowIcon(icon)

    text = load_notes()
    start_history(text)
    state = {"text": text}

    def save_text(new_text: str) -> None:
        previous = state["text"]
        if new_text == previous:
            return
        record_history(new_text)
        state["text"] = new_text

    window = NoteWindow(load_config(), text, save_config, save_text)
    window.show()

    tray_available = QSystemTrayIcon.isSystemTrayAvailable()
    app.setQuitOnLastWindowClosed(not tray_available)
    if tray_available:
        tray = QSystemTrayIcon(icon, app)
        tray.setToolTip("FloatNote 悬浮便签")
        menu = QMenu()
        menu.addAction("显示 / 隐藏面板", window.toggle_visibility)
        menu.addSeparator()
        menu.addAction("重启", lambda: _restart(window))
        menu.addAction("退出", app.quit)
        tray.setContextMenu(menu)
        tray.activated.connect(
            lambda reason: window.toggle_visibility()
            if reason == QSystemTrayIcon.Trigger
            else None
        )
        tray.show()

    if "--screenshot" in sys.argv:
        target = sys.argv[sys.argv.index("--screenshot") + 1]

        def snapshot() -> None:
            window.grab().save(target)
            app.quit()

        QTimer.singleShot(800, snapshot)

    return app.exec()


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except SystemExit:
        raise
    except BaseException:
        import traceback

        Path(__file__).with_name("error.log").write_text(
            traceback.format_exc(), encoding="utf-8"
        )
        raise