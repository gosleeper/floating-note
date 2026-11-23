from __future__ import annotations

from PySide6.QtCore import QPointF, QRectF, Qt, QTimer
from PySide6.QtGui import (
    QAction,
    QColor,
    QFont,
    QFontMetricsF,
    QGuiApplication,
    QPainter,
    QPainterPath,
    QPen,
    QTextLayout,
    QTextOption,
)
from PySide6.QtWidgets import (
    QApplication,
    QColorDialog,
    QMenu,
    QPlainTextEdit,
    QWidget,
)

from store import Config

PAD = 10
MAX_SCREEN_RATIO = 0.8

FONT_SIZES = (12, 14, 16, 18, 20, 22, 24, 28, 32, 36)
OPACITIES = (100, 90, 80, 70, 60, 50, 40, 30)
WIDTHS = (300, 360, 420, 500, 600, 700, 800)


class _Editor(QPlainTextEdit):
    def __init__(self, parent: QWidget, on_finish) -> None:
        super().__init__(parent)
        self._on_finish = on_finish
        self._finished = False

    def keyPressEvent(self, event) -> None:
        if event.key() == Qt.Key_Escape or (
            event.key() in (Qt.Key_Return, Qt.Key_Enter)
            and event.modifiers() & Qt.ControlModifier
        ):
            self.finish()
            return
        super().keyPressEvent(event)

    def focusOutEvent(self, event) -> None:
        super().focusOutEvent(event)
        try:
            self.finish()
        except RuntimeError:
            pass

    def finish(self) -> None:
        if self._finished:
            return
        self._finished = True
        self._on_finish()


class NoteWindow(QWidget):
    def __init__(self, config: Config, text: str, on_config_change, on_text_change) -> None:
        super().__init__()
        self.cfg = config
        self.text = text
        self._on_config_change = on_config_change
        self._on_text_change = on_text_change

        self._render_lines: list[list[tuple[float, float, str]]] = []
        self._line_heights: list[float] = []
        self._content_height = 0.0
        self._viewport_height = 0.0
        self._scroll = 0.0
        self._dragging = False
        self._drag_offset = None
        self._editor: _Editor | None = None

        self.setWindowTitle("FloatNote")
        self.setWindowFlags(
            Qt.FramelessWindowHint | Qt.WindowStaysOnTopHint | Qt.Tool
        )
        self.setAttribute(Qt.WA_TranslucentBackground)
        self.setWindowOpacity(self.cfg.opacity)
        self._apply_font()
        self._relayout()
        self._place_window()

    # ---------- appearance ----------

    def _apply_font(self) -> None:
        font = QFont("Microsoft YaHei UI")
        font.setPointSizeF(self.cfg.font_size)
        self.setFont(font)

    def _font(self) -> QFont:
        return self.font()

    def _build_layouts(self) -> None:
        text_width = max(40.0, self.cfg.width - 2 * PAD)
        metrics = QFontMetricsF(self._font())
        self._render_lines: list[list[tuple[float, float, str]]] = []
        self._line_heights: list[float] = []

        for paragraph in self.text.replace("\t", "    ").split("\n"):
            layout = QTextLayout(paragraph, self._font())
            option = QTextOption()
            option.setWrapMode(QTextOption.WrapAtWordBoundaryOrAnywhere)
            layout.setTextOption(option)
            layout.beginLayout()
            while True:
                line = layout.createLine()
                if not line.isValid():
                    break
                line.setLineWidth(text_width)
            layout.endLayout()

            lines = []
            y = 0.0
            for index in range(layout.lineCount()):
                line = layout.lineAt(index)
                fragment = layout.text()[
                    line.textStart() : line.textStart() + line.textLength()
                ]
                lines.append((line.x(), y + line.ascent(), fragment))
                y += line.height()
            self._render_lines.append(lines)
            self._line_heights.append(max(y, metrics.lineSpacing()))

    def _screen(self):
        return self.screen() or QGuiApplication.primaryScreen()

    def _max_height(self) -> float:
        return self._screen().availableGeometry().height() * MAX_SCREEN_RATIO

    def _relayout(self) -> None:
        metrics = QFontMetricsF(self._font())
        self._build_layouts()

        total = sum(self._line_heights)

        self._content_height = total
        available = max(metrics.lineSpacing(), self._max_height() - 2 * PAD)
        self._viewport_height = min(total, available)
        self._scroll = max(0.0, min(self._scroll, total - self._viewport_height))

        self.setFixedSize(self.cfg.width, int(self._viewport_height + 2 * PAD))
        if self._editor is not None:
            self._editor.setGeometry(
                PAD, PAD, self.cfg.width - 2 * PAD, int(self._viewport_height)
            )

    def _place_window(self) -> None:
        if self.cfg.x is None or self.cfg.y is None:
            self._reset_position()
            return
        self.move(self.cfg.x, self.cfg.y)
        self._clamp_to_screen()

    def _reset_position(self) -> None:
        geo = self._screen().availableGeometry()
        self.move(geo.right() - self.cfg.width - 60, geo.top() + 100)
        self._remember_position()

    def _clamp_to_screen(self) -> None:
        geo = self._screen().availableGeometry()
        x = min(max(self.pos().x(), geo.left()), max(geo.left(), geo.right() - self.width()))
        y = min(max(self.pos().y(), geo.top()), max(geo.top(), geo.bottom() - self.height()))
        if (x, y) != (self.pos().x(), self.pos().y()):
            self.move(x, y)

    def _remember_position(self) -> None:
        self.cfg.x = self.pos().x()
        self.cfg.y = self.pos().y()
        self._on_config_change(self.cfg)

    # ---------- painting ----------

    def paintEvent(self, event) -> None:
        painter = QPainter(self)
        painter.setRenderHint(QPainter.Antialiasing)
        painter.setRenderHint(QPainter.TextAntialiasing)
        painter.setClipRect(QRectF(0, 0, self.width(), self.height()))
        # alpha=1 让整个窗口可接收鼠标；纯 alpha=0 会被 Windows 层窗口判定为点击穿透
        painter.fillRect(self.rect(), QColor(0, 0, 0, 1))

        if self._editor is not None:
            return

        text_color = QColor(self.cfg.color)
        outline_color = QColor("#000000") if _luminance(text_color) > 140 else QColor("#ffffff")
        pen = QPen(
            outline_color,
            max(2.0, self.cfg.font_size * 0.12),
            Qt.SolidLine,
            Qt.RoundCap,
            Qt.RoundJoin,
        )

        y_cursor = PAD - self._scroll
        for lines, height in zip(self._render_lines, self._line_heights):
            for x, baseline, fragment in lines:
                if fragment.strip():
                    path = QPainterPath()
                    path.addText(
                        QPointF(PAD + x, y_cursor + baseline), self._font(), fragment
                    )
                    if self.cfg.outline:
                        painter.strokePath(path, pen)
                    painter.fillPath(path, text_color)
            y_cursor += height

    # ---------- mouse ----------

    def mousePressEvent(self, event) -> None:
        if self._editor is not None:
            return
        if event.button() == Qt.LeftButton:
            self._dragging = True
            self._drag_offset = event.globalPosition().toPoint() - self.frameGeometry().topLeft()
            event.accept()

    def mouseMoveEvent(self, event) -> None:
        if self._dragging and self._drag_offset is not None:
            self.move(event.globalPosition().toPoint() - self._drag_offset)
            event.accept()

    def mouseReleaseEvent(self, event) -> None:
        if self._dragging:
            self._dragging = False
            self._drag_offset = None
            self._clamp_to_screen()
            self._remember_position()
            event.accept()

    def mouseDoubleClickEvent(self, event) -> None:
        if self._editor is None and event.button() == Qt.LeftButton:
            self._enter_edit(event.position().toPoint())
            event.accept()

    def wheelEvent(self, event) -> None:
        if self._editor is not None:
            return
        overflow = self._content_height - self._viewport_height
        if overflow <= 0:
            return
        step = event.angleDelta().y() / 120 * 48
        self._scroll = max(0.0, min(self._scroll - step, overflow))
        self.update()
        event.accept()

    # ---------- editing ----------

    def _enter_edit(self, click_pos=None) -> None:
        if self._editor is not None:
            return

        editor = _Editor(self, self._exit_edit)
        editor.setFont(self._font())
        editor.setFrameStyle(0)
        editor.setStyleSheet(
            "QPlainTextEdit {"
            " background: rgba(18, 18, 18, 200);"
            " color: #ffffff;"
            " border: 1px solid rgba(255, 255, 255, 60);"
            " border-radius: 6px;"
            " padding: 4px;"
            " selection-background-color: rgba(90, 160, 255, 160);"
            "}"
        )
        editor.setPlainText(self.text)
        editor.setGeometry(PAD, PAD, self.cfg.width - 2 * PAD, int(self._viewport_height))
        editor.textChanged.connect(self._grow_for_edit)
        self._editor = editor
        editor.show()
        self.activateWindow()
        editor.setFocus()
        self._grow_for_edit()

        if click_pos is None:
            cursor = editor.textCursor()
            cursor.movePosition(cursor.MoveOperation.End)
        else:
            cursor = editor.cursorForPosition(click_pos - editor.pos())
        editor.setTextCursor(cursor)
        self.update()

    def _editor_content_height(self) -> float:
        document = self._editor.document()
        layout = document.documentLayout()
        height = 0.0
        block = document.firstBlock()
        while block.isValid():
            height += layout.blockBoundingRect(block).height()
            block = block.next()
        return height + 2 * self._editor.frameWidth() + 8

    def _grow_for_edit(self) -> None:
        if self._editor is None:
            return
        metrics = QFontMetricsF(self._font())
        doc_height = self._editor_content_height()
        available = max(metrics.lineSpacing(), self._max_height() - 2 * PAD)
        viewport = min(max(doc_height, metrics.lineSpacing()), available)
        if abs(viewport - self._viewport_height) < 1:
            return
        self._viewport_height = viewport
        self.setFixedSize(self.cfg.width, int(viewport + 2 * PAD))
        self._editor.setGeometry(PAD, PAD, self.cfg.width - 2 * PAD, int(viewport))

    def _exit_edit(self) -> None:
        editor = self._editor
        if editor is None:
            return
        self._editor = None
        text = editor.toPlainText()
        editor.hide()
        editor.deleteLater()

        if text != self.text:
            self.text = text
            self._on_text_change(text)
        self._relayout()
        self._clamp_to_screen()
        self.update()

    # ---------- settings menu ----------

    def contextMenuEvent(self, event) -> None:
        if self._editor is not None:
            return

        menu = QMenu(self)
        menu.addAction("编辑内容", lambda: QTimer.singleShot(0, self._enter_edit))

        font_menu = menu.addMenu("字号")
        for size in FONT_SIZES:
            action = QAction(f"{size}", font_menu)
            action.setCheckable(True)
            action.setChecked(size == self.cfg.font_size)
            action.triggered.connect(lambda _=False, s=size: self._apply(font_size=s))
            font_menu.addAction(action)

        opacity_menu = menu.addMenu("透明度")
        for percent in OPACITIES:
            action = QAction(f"{percent}%", opacity_menu)
            action.setCheckable(True)
            action.setChecked(abs(percent / 100 - self.cfg.opacity) < 0.001)
            action.triggered.connect(lambda _=False, p=percent: self._apply(opacity=p / 100))
            opacity_menu.addAction(action)

        color_action = QAction("文字颜色…", menu)
        color_action.triggered.connect(lambda: self._pick_color())
        menu.addAction(color_action)

        outline_action = QAction("文字描边", menu)
        outline_action.setCheckable(True)
        outline_action.setChecked(self.cfg.outline)
        outline_action.toggled.connect(lambda checked: self._apply(outline=checked))
        menu.addAction(outline_action)

        width_menu = menu.addMenu("宽度")
        for width in WIDTHS:
            action = QAction(f"{width}px", width_menu)
            action.setCheckable(True)
            action.setChecked(width == self.cfg.width)
            action.triggered.connect(lambda _=False, w=width: self._apply(width=w))
            width_menu.addAction(action)

        menu.addSeparator()
        menu.addAction("重置位置", lambda: self._reset_position())
        menu.addAction("退出", lambda: QApplication.instance().quit())
        menu.exec(event.globalPos())
        event.accept()

    def _pick_color(self) -> None:
        color = QColorDialog.getColor(QColor(self.cfg.color), self, "选择文字颜色")
        if color.isValid():
            self._apply(color=color.name())

    def _apply(self, **changes) -> None:
        for key, value in changes.items():
            setattr(self.cfg, key, value)
        if "font_size" in changes:
            self._apply_font()
        if "font_size" in changes or "width" in changes:
            self._relayout()
            self._clamp_to_screen()
        if "opacity" in changes:
            self.setWindowOpacity(self.cfg.opacity)
        self.update()
        self._on_config_change(self.cfg)


def _luminance(color: QColor) -> float:
    return 0.299 * color.red() + 0.587 * color.green() + 0.114 * color.blue()