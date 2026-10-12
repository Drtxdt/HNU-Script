from PySide6.QtCore import Qt
from PySide6.QtWidgets import QLabel, QTableWidget, QTableWidgetItem, QHeaderView, QAbstractItemView
from ..core.model import charset_label


def label(text=""):
    widget = QLabel(text)
    widget.setWordWrap(True)
    widget.setTextFormat(Qt.PlainText)
    widget.setTextInteractionFlags(Qt.TextSelectableByMouse)
    return widget


def fill_table(table, headings, rows):
    table.setEditTriggers(QAbstractItemView.NoEditTriggers)
    table.setColumnCount(len(headings))
    table.setHorizontalHeaderLabels(headings)
    table.setRowCount(len(rows))
    for i, row in enumerate(rows):
        for j, value in enumerate(row):
            item = QTableWidgetItem(str(value))
            item.setToolTip(str(value))
            table.setItem(i, j, item)
    table.horizontalHeader().setSectionResizeMode(QHeaderView.ResizeToContents)


def dfa_table(table, dfa, committed=None):
    size = len(dfa.alphabet)
    rows = []
    for state in dfa.states:
        rows.append([f"S{state.id}", ",".join(r.name for r in state.candidates),
                     state.winner.output if state.winner else ("接受" if state.accepting else "—")] +
                    [f"S{target}" if committed is None or state.id*size+g < committed else ""
                     for g, target in enumerate(dfa.transitions[state.id])])
    fill_table(table, ["状态", "候选规则", "获胜类别"] + [charset_label(g) for g in dfa.alphabet], rows)


def show_machine(graph, layout, active=()):
    graph.load(layout)
    graph.show_subset(set(graph.nodes), set(graph.edges), active)
