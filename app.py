from __future__ import annotations

import sys


def main() -> int:
    from PySide6.QtCore import QTimer
    from PySide6.QtWidgets import QApplication

    from note_window import NoteWindow
    from store import load_config, load_notes, save_config, save_notes

    app = QApplication(sys.argv)
    app.setApplicationName("FloatNote")

    window = NoteWindow(load_config(), load_notes(), save_config, save_notes)
    window.show()

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
        from pathlib import Path

        Path(__file__).with_name("error.log").write_text(
            traceback.format_exc(), encoding="utf-8"
        )
        raise