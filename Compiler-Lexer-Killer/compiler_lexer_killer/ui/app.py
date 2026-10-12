from __future__ import annotations

import html
import json
from pathlib import Path
import sys
import threading

from PySide6.QtCore import Qt, QThread, Signal, QTimer
from PySide6.QtGui import QColor, QTextCursor, QTextCharFormat, QFont
from PySide6.QtWidgets import (QApplication, QMainWindow, QWidget, QVBoxLayout, QHBoxLayout,
    QLabel, QPushButton, QComboBox, QPlainTextEdit, QTabWidget, QSplitter, QTreeWidget,
    QTreeWidgetItem, QTableWidget, QTableWidgetItem, QHeaderView, QListWidget,
    QTextEdit, QDialog, QTextBrowser, QAbstractItemView)

from ..core import compile, simulate, CompileError, Cancelled
from ..core.model import charset_label, set_label, char_label
from .graph import GraphView
from .layout import compute_layout
from .timeline import Timeline

PACKAGE = Path(__file__).resolve().parents[1]


class BuildWorker(QThread):
    ready = Signal(int, object, object)
    failed = Signal(int, object)

    def __init__(self, generation, source, mode, entry):
        super().__init__()
        self.generation, self.source, self.mode, self.entry = generation, source, mode, entry
        self.cancel_event = threading.Event()

    def run(self):
        try:
            result = compile(self.source, self.mode, self.entry, cancelled=self.cancel_event.is_set)
            layouts = {kind: compute_layout(result, kind, self.cancel_event.is_set) for kind in ("nfa", "dfa")}
            if not self.cancel_event.is_set():
                self.ready.emit(self.generation, result, layouts)
        except Cancelled:
            pass
        except Exception as exc:
            self.failed.emit(self.generation, exc)



class MainWindow(QMainWindow):
    def __init__(self):
        super().__init__()
        self.setWindowTitle("Compiler Lexer Killer · 词法分析实验室")
        self.resize(1420, 900)
        self.setMinimumSize(1040, 700)
        self.result = None
        self.simulation = None
        self.generation = 0
        self.workers = []
        self._closing = False
        self.examples = json.loads((PACKAGE / "presets/examples.json").read_text(encoding="utf-8"))

        central = QWidget()
        central.setObjectName("workspace")
        self.setCentralWidget(central)
        outer = QVBoxLayout(central)
        outer.setContentsMargins(22, 18, 22, 12)
        header = QHBoxLayout()
        titles = QVBoxLayout()
        title = QLabel("词法分析实验室")
        self.brand_title = title
        title.setObjectName("title")
        titles.addWidget(title)
        subtitle = QLabel("COMPILER LEXER KILLER    /    把每一次状态迁移看清楚")
        self.brand_subtitle = subtitle
        subtitle.setObjectName("subtitle")
        titles.addWidget(subtitle)
        header.addLayout(titles)
        header.addStretch()
        self.presets = QComboBox()
        self.presets.setMinimumWidth(235)
        self.presets.addItems([e["name"] for e in self.examples])
        header.addWidget(self.presets)
        help_button = QPushButton("语法与帮助")
        help_button.clicked.connect(self.help)
        header.addWidget(help_button)
        outer.addLayout(header)

        self.splitter = QSplitter(Qt.Horizontal)
        sidebar = QWidget()
        side = QVBoxLayout(sidebar)
        side.setContentsMargins(0, 12, 12, 0)
        self.mode = QComboBox()
        self.mode.addItem("单表达式", "expression")
        self.mode.addItem("课件式命名定义", "definitions")
        side.addWidget(QLabel("输入方式"))
        side.addWidget(self.mode)
        self.editor = QPlainTextEdit()
        self.editor.setPlaceholderText("(a|b)*abb")
        self.editor.setMinimumHeight(170)
        side.addWidget(self.editor, 3)
        self.entry = QComboBox()
        side.addWidget(QLabel("入口定义（单表达式模式无需选择）"))
        side.addWidget(self.entry)
        buttons = QHBoxLayout()
        self.generate = QPushButton("生成自动机")
        self.generate.setObjectName("primary")
        self.cancel = QPushButton("取消")
        self.cancel.setEnabled(False)
        buttons.addWidget(self.generate, 1)
        buttons.addWidget(self.cancel)
        side.addLayout(buttons)
        self.feedback = QLabel("就绪")
        self.feedback.setWordWrap(True)
        self.feedback.setTextFormat(Qt.PlainText)
        side.addWidget(self.feedback)
        self.example_note = QLabel()
        self.example_note.setWordWrap(True)
        self.example_note.setObjectName("subtitle")
        side.addWidget(self.example_note)
        self.ast_tree = QTreeWidget()
        self.ast_tree.setHeaderLabel("AST · 点击节点查看对应构造步骤")
        side.addWidget(self.ast_tree, 4)
        self.splitter.addWidget(sidebar)

        self.tabs = QTabWidget()
        from .construction_pages import NfaPage, DfaPage, SimulationPage
        nfa_page, dfa_page, sim_page = NfaPage(), DfaPage(), SimulationPage()
        self.nfa_graph, self.nfa_formula, self.nfa_timeline = nfa_page.graph, nfa_page.formula, nfa_page.timeline
        self.subset_nfa, self.dfa_graph, self.table = dfa_page.nfa, dfa_page.dfa, dfa_page.table
        self.dfa_formula, self.dfa_timeline = dfa_page.formula, dfa_page.timeline
        self.input, self.run_button, self.tape = sim_page.input, sim_page.run, sim_page.tape
        self.sim_graph, self.sim_log = sim_page.graph, sim_page.log
        self.sim_formula, self.sim_timeline = sim_page.formula, sim_page.timeline
        self.tabs.addTab(nfa_page, "01   正则 → NFA")
        self.tabs.addTab(dfa_page, "02   NFA → DFA")
        self.tabs.addTab(sim_page, "03   字符串模拟")
        self.splitter.addWidget(self.tabs)
        self.splitter.setSizes([310, 1060])
        outer.addWidget(self.splitter, 1)
        legend = QLabel("开始 → 初态     双圈 / 绿色：接受状态     蓝色：当前步骤     ε：不消耗字符     ∅：空状态集合")
        self.legend = legend
        legend.setObjectName("subtitle")
        outer.addWidget(legend)
        self.statusBar().showMessage("离线运行 · ASCII 0–127 · 状态编号本身没有数学意义，不同构造可得到等价自动机")

        self.editor.textChanged.connect(self.source_changed)
        self.mode.currentIndexChanged.connect(self.source_changed)
        self.entry.currentTextChanged.connect(self.invalidate)
        self.generate.clicked.connect(self.build)
        self.cancel.clicked.connect(self.cancel_build)
        self.presets.activated.connect(self.load_preset)
        self.nfa_timeline.changed.connect(self.show_nfa)
        self.dfa_timeline.changed.connect(self.show_dfa)
        self.sim_timeline.changed.connect(self.show_simulation)
        self.run_button.clicked.connect(self.run_simulation)
        self.input.textChanged.connect(self.invalidate_simulation)
        self.ast_tree.itemClicked.connect(self.ast_clicked)
        self.table.cellClicked.connect(lambda row, col: self.select_dfa(row))
        self.dfa_graph.state_clicked.connect(self.select_dfa)
        self.sim_graph.state_clicked.connect(self.select_dfa)
        for graph in (self.nfa_graph, self.subset_nfa):
            graph.state_clicked.connect(lambda number: self.statusBar().showMessage(f"NFA 状态 q{number}；点击 AST 节点可定位它所属的构造片段"))
        self.tabs.currentChanged.connect(self.tab_changed)
        self.load_preset(0)

    @property
    def timelines(self):
        return (self.nfa_timeline, self.dfa_timeline, self.sim_timeline)

    def tab_changed(self, index):
        for timeline in self.timelines:
            timeline.stop()
        QTimer.singleShot(0, self.fit_graphs)

    def fit_graphs(self):
        for graph in (self.nfa_graph, self.subset_nfa, self.dfa_graph, self.sim_graph):
            graph.fit()

    def source_changed(self):
        chosen = self.entry.currentText()
        self.entry.blockSignals(True)
        self.entry.clear()
        if self.mode.currentData() == "definitions":
            names = []
            for line in self.editor.toPlainText().splitlines():
                normalized = line.replace("->", "→", 1)
                if "→" in normalized:
                    name = normalized.split("→", 1)[0].strip()
                    if name and name not in names:
                        names.append(name)
            self.entry.addItems(names)
            if names:
                self.entry.setCurrentIndex(names.index(chosen) if chosen in names else len(names)-1)
        self.entry.setEnabled(self.mode.currentData() == "definitions")
        self.entry.blockSignals(False)
        self.invalidate()

    def invalidate(self, *_):
        self.generation += 1
        for worker in self.workers:
            worker.cancel_event.set()
        self.result = None
        self.simulation = None
        for timeline in self.timelines:
            timeline.set_steps(0)
        for graph in (self.nfa_graph, self.subset_nfa, self.dfa_graph, self.sim_graph):
            graph.clear()
        self.ast_tree.clear()
        self.table.setRowCount(0)
        self.sim_log.clear()
        self.tape.setText("输入已改变，请重新生成自动机。")
        for label in (self.nfa_formula, self.dfa_formula, self.sim_formula):
            label.setText("输入已改变，请重新生成自动机。")
        self.run_button.setEnabled(False)
        self.generate.setEnabled(True)
        self.cancel.setEnabled(False)
        self.feedback.setText("待生成 · 输入改变后旧结果已失效")
        self.feedback.setStyleSheet("color: #64748b")
        self.editor.setExtraSelections([])

    def load_preset(self, index):
        example = self.examples[index]
        self.mode.setCurrentIndex(self.mode.findData(example["mode"]))
        self.editor.setPlainText(example["source"])
        self.input.setPlainText(example["input"])
        self.example_note.setText(example["note"])

    def build(self):
        self.invalidate()
        generation = self.generation
        worker = BuildWorker(generation, self.editor.toPlainText(), self.mode.currentData(), self.entry.currentText() or None)
        worker.ready.connect(self.built)
        worker.failed.connect(self.failed)
        worker.finished.connect(lambda w=worker: self.worker_finished(w))
        self.workers.append(worker)
        self.generate.setEnabled(False)
        self.cancel.setEnabled(True)
        self.feedback.setText("正在构造自动机和布局…")
        worker.start()

    def cancel_build(self):
        self.invalidate()
        self.feedback.setText("已取消构建，可修改输入后重新生成。")

    def worker_finished(self, worker):
        if worker in self.workers:
            self.workers.remove(worker)
        worker.deleteLater()
        if self._closing and not self.workers:
            self.close()

    def built(self, generation, result, layouts):
        if generation != self.generation or self._closing:
            return
        self.result = result
        self.nfa_graph.load(layouts["nfa"])
        self.subset_nfa.load(layouts["nfa"])
        self.dfa_graph.load(layouts["dfa"])
        self.sim_graph.load(layouts["dfa"])
        self.invalidate_simulation()
        self.populate_ast()
        self.populate_table()
        self.nfa_timeline.set_steps(len(result.nfa_steps))
        self.dfa_timeline.set_steps(len(result.dfa_steps))
        self.generate.setEnabled(True)
        self.cancel.setEnabled(False)
        self.run_button.setEnabled(True)
        self.feedback.setText(f"生成成功 · NFA {len(result.nfa.states)} 状态 / DFA {len(result.dfa.states)} 状态\n从第一步开始回放，拖动进度条可查看完整结果。")
        self.feedback.setStyleSheet("color: #047857")
        QTimer.singleShot(0, self.fit_graphs)

    def failed(self, generation, error):
        if generation != self.generation or self._closing:
            return
        self.feedback.setText(str(error))
        self.feedback.setStyleSheet("color: #b91c1c")
        self.generate.setEnabled(True)
        self.cancel.setEnabled(False)
        if isinstance(error, CompileError):
            self.highlight_source(error.span, True)

    def highlight_source(self, span, error=False):
        selection = QTextEdit.ExtraSelection()
        cursor = self.editor.textCursor()
        # QTextCursor offsets count UTF-16 code units, core offsets count code points.
        source = self.editor.toPlainText()
        begin = len(source[:span.start].encode("utf-16-le")) // 2
        end = len(source[:span.end].encode("utf-16-le")) // 2
        cursor.setPosition(begin)
        cursor.setPosition(max(begin, end), QTextCursor.KeepAnchor)
        selection.cursor = cursor
        selection.format.setBackground(QColor("#fee2e2" if error else "#dbeafe"))
        if begin == end:
            selection.format.setProperty(QTextCharFormat.FullWidthSelection, True)
        self.editor.setExtraSelections([selection])
        if error:
            self.editor.setTextCursor(cursor)
            self.editor.ensureCursorVisible()

    def populate_ast(self):
        self.ast_tree.clear()
        labels = {"concat":"连接", "union":"并 |", "star":"闭包 *", "plus":"正闭包 +", "optional":"可选 ?", "epsilon":"空串 ε", "group":"分组", "reference":"引用"}
        stack = [(self.result.ast, None)]
        occurrence = 0
        while stack:
            node, parent = stack.pop()
            label = charset_label(node.chars) if node.kind == "chars" else labels[node.kind]
            if node.name:
                label += " " + node.name
            item = QTreeWidgetItem([label])
            item.setData(0, Qt.UserRole, node.span)
            item.setData(0, Qt.UserRole + 1, occurrence)
            occurrence += 1
            if parent is None:
                self.ast_tree.addTopLevelItem(item)
            else:
                parent.addChild(item)
            item.setToolTip(0, f"第 {node.span.line} 行，第 {node.span.column} 列")
            stack.extend((child, item) for child in reversed(node.children))
        self.ast_tree.expandToDepth(3)

    def ast_clicked(self, item, _):
        if not self.result:
            return
        span = item.data(0, Qt.UserRole)
        occurrence = item.data(0, Qt.UserRole + 1)
        indices = [i for i, step in enumerate(self.result.nfa_steps) if step.ast_occurrence == occurrence]
        if indices:
            self.tabs.setCurrentIndex(0)
            self.nfa_timeline.seek(indices[-1])
            self.highlight_source(span)

    def populate_table(self):
        dfa = self.result.dfa
        self.table.setColumnCount(2 + len(dfa.alphabet))
        self.table.setRowCount(len(dfa.states))
        self.table.setHorizontalHeaderLabels(["DFA 状态", "NFA 状态集合"] + [charset_label(c) for c in dfa.alphabet])
        for index, chars in enumerate(dfa.alphabet):
            self.table.horizontalHeaderItem(index+2).setToolTip("字符组：" + ", ".join(char_label(c) for c in sorted(chars)))
        for state in dfa.states:
            self.table.setItem(state.id, 0, QTableWidgetItem(f"S{state.id}" + ("  ✓" if state.accepting else "")))
            self.table.setItem(state.id, 1, QTableWidgetItem(set_label(state.nfa_states)))
            for j in range(len(dfa.alphabet)):
                self.table.setItem(state.id, j+2, QTableWidgetItem())

    def show_nfa(self, index):
        if not self.result:
            return
        step = self.result.nfa_steps[index]
        self.nfa_graph.show_step(step.node_count, step.edge_count, step.nodes, step.edges)
        endpoints = f"\n当前片段：q{step.fragment[0]} → q{step.fragment[1]}" if step.fragment else ""
        self.nfa_formula.setText(step.message + endpoints)
        if step.span:
            self.highlight_source(step.span)

    def show_dfa(self, index):
        if not self.result:
            return
        step = self.result.dfa_steps[index]
        nfa, dfa = self.result.nfa, self.result.dfa
        self.subset_nfa.show_step(len(nfa.states), len(nfa.edges), step.nodes, step.edges)
        selected = {s for s in (step.source, step.target) if s >= 0}
        group_count = len(dfa.alphabet)
        highlighted = [step.source * group_count + step.group] if step.kind == "transition" else []
        self.dfa_graph.show_step(step.node_count, step.committed, selected, highlighted)
        for state in dfa.states:
            self.table.setRowHidden(state.id, state.id >= step.node_count)
            for j, target in enumerate(dfa.transitions[state.id]):
                cell = self.table.item(state.id, j+2)
                cell.setText(f"S{target}" if state.id*group_count+j < step.committed else "")
                cell.setBackground(QColor("#dbeafe" if (state.id, j) == (step.source, step.group) else "#ffffff"))
        self.dfa_formula.setText(step.message)

    def select_dfa(self, number):
        if not self.result or number >= len(self.result.dfa.states):
            return
        state = self.result.dfa.states[number]
        nfa = self.result.nfa
        self.subset_nfa.show_step(len(nfa.states), len(nfa.edges), state.nfa_states)
        self.statusBar().showMessage(f"S{number} = {set_label(state.nfa_states)} · " + ("接受状态" if state.accepting else "非接受状态"))

    def invalidate_simulation(self):
        self.simulation = None
        self.sim_timeline.set_steps(0)
        self.sim_log.clear()
        self.sim_formula.setText("点击“开始模拟”，按原样读取输入（包括空白）。")
        self.tape.setText("输入完整保留空白 · 空输入表示空串")
        if self.result:
            self.sim_graph.show_step(len(self.result.dfa.states), sum(map(len, self.result.dfa.transitions)))

    def run_simulation(self):
        if not self.result:
            return
        text = self.input.toPlainText()
        if len(text) > 4096:
            self.sim_formula.setText("教学模拟最多支持 4096 个字符，请缩短输入。")
            return
        self.simulation = simulate(self.result.dfa, text)
        self.sim_log.clear()
        self.sim_log.addItems([step.message for step in self.simulation.steps])
        self.sim_timeline.set_steps(len(self.simulation.steps))

    def show_simulation(self, index):
        if not self.simulation or not self.result:
            return
        step = self.simulation.steps[index]
        dfa = self.result.dfa
        edge = [step.source*len(dfa.alphabet)+step.group] if step.group >= 0 else []
        self.sim_graph.show_step(len(dfa.states), sum(map(len, dfa.transitions)), [step.target], edge)
        self.sim_formula.setText(step.message)
        self.sim_log.setCurrentRow(index)
        self.sim_log.scrollToItem(self.sim_log.item(index))
        text = self.input.toPlainText()
        if index == len(self.simulation.steps)-1 and self.simulation.failure_position is not None:
            failed = self.simulation.failure_position
            where = "输入结束时仍未接受" if failed == len(text) else f"第 {failed+1} 个字符"
            self.sim_formula.setText(step.message + "\n首次失败位置：" + where)
        position = max(step.position-1, 0)
        start, end = max(position-25, 0), min(position+40, len(text))
        parts = []
        for i in range(start, end):
            char = html.escape(char_label(text[i]))
            parts.append(f'<b style="color:#1d4ed8;text-decoration:underline">[{char}]</b>' if i == position and step.position else char)
        self.tape.setText(f"已读 {step.position} / {len(text)}　" + ("…" if start else "") + " ".join(parts) + ("…" if end < len(text) else "") + ("ε（空串）" if not text else ""))

    def help(self):
        dialog = QDialog(self)
        dialog.setWindowTitle("语法与学习路线")
        dialog.resize(720, 570)
        layout = QVBoxLayout(dialog)
        browser = QTextBrowser()
        browser.setHtml("""<h2>从表达式到自动机</h2>
<p>输入后点击生成，再用底部控制条逐步回放。拖动画布平移，滚轮缩放，点击状态查看它对应的 NFA 集合。</p>
<h3>两种输入方式</h3><p>单表达式：<code>(a|b)*abb</code>。命名定义中裸名称代表引用，文字加引号：<code>keyword → 'if'</code>。每行一个定义，只能引用前面已有名称，入口默认为最后一项。</p>
<h3>支持的语法</h3><p>ε、并 |、连接（相邻或 ·）、*、+、?、括号、[a-z]、[^a]，以及单／双引号字面文字。优先级：重复 &gt; 连接 &gt; 并。句点 . 是字面字符。</p>
<p>字符范围是 ASCII 0–127。使用反斜杠转义：\\n、\\r、\\t、\\xHH 和特殊符号。课件 /n 等写法需转换。引号和字符类外的空白忽略；模拟输入中的空白完整保留。</p>
<p>暂不支持命名定义的前向或循环引用、花括号计数、通配点、锚点、\\d、回溯引用、断言和惰性量词。</p>
<h3>如何阅读结果</h3><p>ε 边不读取字符。move 只经过指定字符边；ε-closure 包含起始集合自身及沿 ε 边可达的状态。DFA 状态包含 NFA 接受状态时也是接受状态。取空集合的状态为陷阱状态。</p>
<p>Thompson 各片段在构造中用蓝色标示入口和出口，最终接受状态用绿色双圈。状态编号没有数学意义；本工具不会为了复制课件编号而合并状态。</p>
<p>最多 512 个 NFA 状态、256 个 DFA 状态、20,000 条构造步骤。超限会报错，不输出截断结果。多规则分词、优先级分类和最小化属于后续版本。</p>""")
        layout.addWidget(browser)
        dialog.exec()

    def closeEvent(self, event):
        if self.workers:
            self._closing = True
            self.generation += 1
            for worker in self.workers:
                worker.cancel_event.set()
            self.hide()
            event.ignore()
        else:
            event.accept()


def main():
    app = QApplication(sys.argv)
    app.setApplicationName("Compiler Lexer Killer")
    app.setStyle("Fusion")
    app.setFont(QFont("Microsoft YaHei UI", 10))
    app.setStyleSheet((PACKAGE / "ui/styles.qss").read_text(encoding="utf-8"))
    from .lab import LabWindow
    window = LabWindow()
    window.show()
    if "--smoke-test" in sys.argv:
        from .smoke import attach_smoke
        attach_smoke(app, window)
    sys.exit(app.exec())
