from __future__ import annotations

from datetime import datetime

from PySide6.QtCore import QDateTime, Qt, Signal
from PySide6.QtGui import QFont
from PySide6.QtWidgets import (
    QApplication,
    QDateTimeEdit,
    QHBoxLayout,
    QLabel,
    QPlainTextEdit,
    QPushButton,
    QVBoxLayout,
    QWidget,
)

from store import history_bounds, history_increment


def _qdate(moment: datetime) -> QDateTime:
    return QDateTime.fromSecsSinceEpoch(int(moment.timestamp()))


class HistoryWindow(QWidget):
    closed = Signal()

    def __init__(self) -> None:
        super().__init__()
        self.setWindowTitle("变更记录")
        self.setWindowFlags(Qt.Window | Qt.WindowStaysOnTopHint)
        self.resize(760, 560)

        first, _ = history_bounds()
        now = datetime.now()

        self.start_edit = QDateTimeEdit(self)
        self.start_edit.setDateTime(_qdate(first or now))
        self.end_edit = QDateTimeEdit(self)
        self.end_edit.setDateTime(_qdate(now))
        for edit in (self.start_edit, self.end_edit):
            edit.setCalendarPopup(True)
            edit.setDisplayFormat("yyyy-MM-dd HH:mm:ss")

        generate = QPushButton("生成", self)
        generate.clicked.connect(self.generate)

        range_row = QHBoxLayout()
        range_row.addWidget(QLabel("从", self))
        range_row.addWidget(self.start_edit, 1)
        range_row.addWidget(QLabel("到", self))
        range_row.addWidget(self.end_edit, 1)
        range_row.addWidget(generate)

        self.output = QPlainTextEdit(self)
        self.output.setReadOnly(True)
        self.output.setFont(QFont("Consolas", 10))
        self.output.setLineWrapMode(QPlainTextEdit.NoWrap)

        copy_button = QPushButton("复制", self)
        copy_button.clicked.connect(self._copy)
        close_button = QPushButton("关闭", self)
        close_button.clicked.connect(self.close)

        buttons = QHBoxLayout()
        buttons.addStretch(1)
        buttons.addWidget(copy_button)
        buttons.addWidget(close_button)

        layout = QVBoxLayout(self)
        layout.addLayout(range_row)
        layout.addWidget(self.output, 1)
        layout.addLayout(buttons)

        self.generate()

    def generate(self) -> None:
        start = datetime.fromtimestamp(self.start_edit.dateTime().toSecsSinceEpoch())
        end = datetime.fromtimestamp(self.end_edit.dateTime().toSecsSinceEpoch())
        if end < start:
            start, end = end, start
            self.start_edit.setDateTime(_qdate(start))
            self.end_edit.setDateTime(_qdate(end))
        self.output.setPlainText(history_increment(start, end))

    def closeEvent(self, event) -> None:
        self.closed.emit()
        super().closeEvent(event)

    def _copy(self) -> None:
        QApplication.clipboard().setText(self.output.toPlainText())