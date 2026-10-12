"""Construction/simulation views. MainWindow supplies their controller actions."""
from PySide6.QtCore import Qt
from PySide6.QtWidgets import (QWidget, QVBoxLayout, QHBoxLayout, QSplitter, QTableWidget,
    QHeaderView, QAbstractItemView, QPlainTextEdit, QPushButton, QListWidget)
from .common import label
from .graph import GraphView
from .timeline import Timeline


def formula():
    widget = label("生成自动机后，点击下一步开始推导。")
    widget.setObjectName("formula")
    widget.setMinimumHeight(60)
    return widget


class NfaPage(QWidget):
    def __init__(self):
        super().__init__()
        box = QVBoxLayout(self)
        self.graph = GraphView("THOMPSON NFA")
        self.formula, self.timeline = formula(), Timeline()
        box.addWidget(self.graph, 1)
        box.addWidget(self.formula)
        box.addWidget(self.timeline)


class DfaPage(QWidget):
    def __init__(self):
        super().__init__()
        box = QVBoxLayout(self)
        split = QSplitter(Qt.Horizontal)
        self.nfa, self.dfa = GraphView("NFA · 当前集合 / 迁移"), GraphView("DFA · 已发现状态")
        split.addWidget(self.nfa)
        split.addWidget(self.dfa)
        self.table = QTableWidget()
        self.table.setEditTriggers(QAbstractItemView.NoEditTriggers)
        self.table.setSelectionMode(QAbstractItemView.SingleSelection)
        self.table.verticalHeader().hide()
        self.table.horizontalHeader().setSectionResizeMode(QHeaderView.ResizeToContents)
        self.table.setMinimumHeight(120)
        vertical = QSplitter(Qt.Vertical)
        vertical.addWidget(split)
        vertical.addWidget(self.table)
        vertical.setSizes([460, 210])
        self.formula, self.timeline = formula(), Timeline()
        box.addWidget(vertical, 1)
        box.addWidget(self.formula)
        box.addWidget(self.timeline)


class SimulationPage(QWidget):
    def __init__(self):
        super().__init__()
        box = QVBoxLayout(self)
        row = QHBoxLayout()
        self.input = QPlainTextEdit()
        self.input.setPlaceholderText("输入待匹配字符串；留空表示空串。空格、换行不会被删除。")
        self.input.setMaximumHeight(85)
        self.run = QPushButton("开始模拟")
        self.run.setObjectName("primary")
        self.run.setEnabled(False)
        row.addWidget(self.input, 1)
        row.addWidget(self.run)
        box.addLayout(row)
        # The original controller renders the tape with escaped rich text.
        self.tape = label("输入完整保留空白 · 最终必须停在接受状态")
        self.tape.setTextFormat(Qt.AutoText)
        self.tape.setMinimumHeight(35)
        box.addWidget(self.tape)
        self.graph = GraphView("DFA · 字符串模拟")
        self.log = QListWidget()
        split = QSplitter(Qt.Horizontal)
        split.addWidget(self.graph)
        split.addWidget(self.log)
        split.setSizes([650, 260])
        box.addWidget(split, 1)
        self.formula, self.timeline = formula(), Timeline()
        box.addWidget(self.formula)
        box.addWidget(self.timeline)
