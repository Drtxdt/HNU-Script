from dataclasses import asdict
from PySide6.QtCore import Qt, Signal, QTimer
from PySide6.QtGui import QTextCursor, QColor
from PySide6.QtWidgets import (QWidget, QVBoxLayout, QHBoxLayout, QPlainTextEdit, QTableWidget,
    QTableWidgetItem, QPushButton, QComboBox, QCheckBox, QSplitter, QTabWidget, QTextEdit, QHeaderView)
from ..core import compile_lexer, tokenize, minimize, RuleSpec
from ..core.model import CompileError
from ..presets import LEXER_PRESETS
from .jobs import Jobs
from .graph import GraphView
from .timeline import Timeline
from .common import label, fill_table, dfa_table
from .layout import machine_layout


class LexerPage(QWidget):
    changed = Signal()
    machine_ready = Signal(object)

    def __init__(self, parent=None):
        super().__init__(parent)
        self.compilation = self.run = self.minimized = None
        self.loading = False
        box = QVBoxLayout(self)
        row = QHBoxLayout()
        self.presets = QComboBox()
        self.presets.addItems([p[0] for p in LEXER_PRESETS])
        self.build_button, self.cancel = QPushButton("构建规则自动机"), QPushButton("取消")
        self.use_min = QCheckBox("使用最小化 DFA")
        for widget in (self.presets, self.build_button, self.cancel, self.use_min):
            row.addWidget(widget)
        box.addLayout(row)
        self.note = label()
        box.addWidget(self.note)
        split = QSplitter()
        left = QWidget()
        edit = QVBoxLayout(left)
        edit.setContentsMargins(0, 0, 0, 0)
        edit.addWidget(label("公共定义（裸名称引用；字面文字加引号）"))
        self.definitions = QPlainTextEdit()
        self.definitions.setMaximumHeight(84)
        edit.addWidget(self.definitions)
        self.rules = QTableWidget(0, 5)
        self.rules.setMinimumHeight(125)
        self.rules.setHorizontalHeaderLabels(["启用", "规则名称", "类别", "表达式", "跳过"])
        self.rules.setColumnWidth(3, 250)
        for column, width in ((0,45), (1,85), (2,85), (4,45)):
            self.rules.setColumnWidth(column, width)
        self.rules.horizontalHeader().setSectionResizeMode(3, QHeaderView.Stretch)
        edit.addWidget(self.rules, 1)
        controls = QHBoxLayout()
        for text, callback in [("增加", self.add_rule), ("删除", self.remove_rule),
                               ("上移", lambda: self.move_rule(-1)), ("下移", lambda: self.move_rule(1))]:
            button = QPushButton(text)
            button.clicked.connect(callback)
            controls.addWidget(button)
        edit.addLayout(controls)
        self.input = QPlainTextEdit()
        self.input.setPlaceholderText("输入源文本；空白会原样参与扫描。")
        self.input.setMaximumHeight(100)
        edit.addWidget(self.input)
        runrow = QHBoxLayout()
        self.run_button = QPushButton("分析文本")
        self.recover, self.show_skipped = QCheckBox("遇错继续"), QCheckBox("显示跳过内容")
        for widget in (self.run_button, self.recover, self.show_skipped):
            runrow.addWidget(widget)
        edit.addLayout(runrow)
        split.addWidget(left)
        right = QWidget()
        views = QVBoxLayout(right)
        views.setContentsMargins(0, 0, 0, 0)
        self.view_tabs = QTabWidget()
        self.nfa_graph, self.graph = GraphView("联合 NFA · 规则终态"), GraphView("带类别 DFA")
        self.dtran, self.tokens = QTableWidget(), QTableWidget()
        for widget, title in [(self.graph, "DFA"), (self.nfa_graph, "联合 NFA"), (self.dtran, "DTran"), (self.tokens, "Token")]:
            self.view_tabs.addTab(widget, title)
        views.addWidget(self.view_tabs, 1)
        self.stage = QComboBox()
        self.stage.addItems(["DFA 构造", "扫描过程", "NFA 组合"])
        views.addWidget(self.stage)
        self.message = label()
        views.addWidget(self.message)
        self.timeline = Timeline()
        views.addWidget(self.timeline)
        split.addWidget(right)
        split.setSizes([470, 650])
        box.addWidget(split, 1)
        self.jobs = Jobs(self)
        self.jobs.result.connect(self.loaded)
        self.jobs.error.connect(self.failed)
        self.jobs.busy.connect(self.busy)
        self.build_button.clicked.connect(self.build)
        self.run_button.clicked.connect(self.analyze)
        self.cancel.clicked.connect(self.invalidate)
        self.definitions.textChanged.connect(self.invalidate)
        self.rules.itemChanged.connect(self.invalidate)
        self.input.textChanged.connect(self.invalidate_run)
        self.recover.toggled.connect(self.invalidate_run)
        self.show_skipped.toggled.connect(lambda: self.show_tokens())
        self.use_min.toggled.connect(self.variant_changed)
        self.stage.currentIndexChanged.connect(self.reset_timeline)
        self.timeline.changed.connect(self.show_step)
        self.presets.activated.connect(self.load_preset)
        self.view_tabs.currentChanged.connect(lambda: QTimer.singleShot(0, self.fit_graphs))
        self.load_preset(0)

    def busy(self, value):
        self.build_button.setEnabled(not value)
        self.run_button.setEnabled(not value and self.compilation is not None)

    def rule_specs(self):
        def text(row, col):
            item = self.rules.item(row, col)
            return item.text() if item else ""
        return tuple(RuleSpec(text(r, 1), text(r, 3), text(r, 2), self.rules.item(r, 4).checkState() == Qt.Checked,
                              self.rules.item(r, 0).checkState() == Qt.Checked) for r in range(self.rules.rowCount()))

    def set_rules(self, rules):
        self.loading = True
        self.rules.setRowCount(0)
        for rule in rules:
            self.add_rule(rule)
        self.loading = False
        self.invalidate()

    def add_rule(self, rule=None):
        if not isinstance(rule, RuleSpec):
            rule = RuleSpec(f"RULE_{self.rules.rowCount()+1}", "'a'")
        r = self.rules.rowCount()
        self.rules.blockSignals(True)
        self.rules.insertRow(r)
        for c, value in enumerate([rule.enabled, rule.name, rule.category, rule.pattern, rule.skip]):
            item = QTableWidgetItem()
            if c in (0, 4):
                item.setFlags(Qt.ItemIsEnabled | Qt.ItemIsSelectable | Qt.ItemIsUserCheckable)
                item.setCheckState(Qt.Checked if value else Qt.Unchecked)
            else:
                item.setText(str(value))
                item.setToolTip(str(value))
            self.rules.setItem(r, c, item)
        self.rules.blockSignals(False)
        self.invalidate()

    def remove_rule(self):
        if self.rules.currentRow() >= 0:
            self.rules.removeRow(self.rules.currentRow())
            self.invalidate()

    def move_rule(self, delta):
        row = self.rules.currentRow()
        target = row + delta
        if row < 0 or not 0 <= target < self.rules.rowCount():
            return
        rules = list(self.rule_specs())
        rules[row], rules[target] = rules[target], rules[row]
        self.set_rules(rules)
        self.rules.setCurrentCell(target, 1)

    def load_preset(self, index):
        self.presets.setCurrentIndex(index)
        _, definitions, rules, text, note = LEXER_PRESETS[index]
        self.definitions.setPlainText(definitions)
        self.set_rules(rules)
        self.input.setPlainText(text)
        self.note.setText(note)

    def fit_graphs(self):
        for graph in (self.graph, self.nfa_graph):
            if graph.isVisible():
                graph.fit()

    def invalidate(self, *_):
        if self.loading:
            return
        self.jobs.invalidate()
        self.compilation = self.minimized = self.run = None
        self.timeline.set_steps(0)
        self.graph.clear()
        self.nfa_graph.clear()
        self.dtran.setRowCount(0)
        self.tokens.setRowCount(0)
        self.run_button.setEnabled(False)
        self.message.setText("规则已改变，请重新构建。")
        self.changed.emit()

    def invalidate_run(self, *_):
        self.jobs.invalidate()
        self.run = None
        self.tokens.setRowCount(0)
        self.timeline.stop()
        if self.stage.currentIndex() == 1:
            self.timeline.set_steps(0)
        self.changed.emit()

    def build(self):
        definitions, rules = self.definitions.toPlainText(), self.rule_specs()
        self.invalidate()
        self.message.setText("正在构建带类别自动机…")
        def work(cancelled):
            result = compile_lexer(definitions, rules, cancelled=cancelled)
            minimized = minimize(result.dfa, cancelled=cancelled)
            layouts = {"nfa": machine_layout(result.nfa, cancelled), "dfa": machine_layout(result.dfa, cancelled),
                       "min": machine_layout(minimized.dfa, cancelled)}
            return "build", result, minimized, layouts
        self.jobs.start(work)

    def failed(self, exc):
        self.message.setText(str(exc))
        if isinstance(exc, CompileError) and exc.source_id.startswith("rule:"):
            row = int(exc.source_id.split(":")[1])
            self.rules.setCurrentCell(row, 3)
            self.rules.scrollToItem(self.rules.item(row, 3))

    def loaded(self, payload):
        if payload[0] == "build":
            _, self.compilation, self.minimized, self.layouts = payload
            self.nfa_graph.load(self.layouts["nfa"])
            self.variant_changed()
            self.run_button.setEnabled(True)
            self.machine_ready.emit(self.compilation.dfa)
            self.note.setText("构建完成。" + "；".join(self.compilation.diagnostics))
        else:
            self.run = payload[1]
            self.stage.setCurrentIndex(1)
            self.reset_timeline()
            self.timeline.seek(len(self.run.steps)-1)
            self.show_tokens()
            self.view_tabs.setCurrentWidget(self.tokens)

    @property
    def dfa(self):
        return self.minimized.dfa if self.use_min.isChecked() else self.compilation.dfa

    def variant_changed(self, *_):
        self.invalidate_run()
        if self.compilation:
            self.graph.load(self.layouts["min" if self.use_min.isChecked() else "dfa"])
            dfa_table(self.dtran, self.dfa)
            self.reset_timeline()

    def analyze(self):
        if not self.compilation:
            return
        text, recover, dfa = self.input.toPlainText(), self.recover.isChecked(), self.dfa
        if len(text) > 4096:
            self.message.setText("教学扫描最多支持 4096 个字符。")
            return
        self.run = None
        self.tokens.setRowCount(0)
        self.timeline.set_steps(0)
        self.jobs.start(lambda cancelled: ("run", tokenize(dfa, text, recover, cancelled=cancelled)))

    def reset_timeline(self, *_):
        if not self.compilation:
            return
        stage = self.stage.currentIndex()
        count = len(self.run.steps) if stage == 1 and self.run else (len(self.compilation.dfa_steps) if stage == 0 else len(self.compilation.nfa_steps) if stage == 2 else 0)
        if stage == 0 and self.use_min.isChecked():
            count = len(self.minimized.steps)
        self.timeline.set_steps(count)

    def show_tokens(self):
        if not self.run:
            return
        if self.stage.currentIndex() == 1:
            tokens = [s.token for s in self.run.steps[:self.timeline.slider.value()+1] if s.token is not None and (not s.token.skip or self.show_skipped.isChecked())]
        else:
            tokens = list(self.run.tokens) + (list(self.run.skipped) if self.show_skipped.isChecked() else [])
        tokens.sort(key=lambda t: t.start)
        fill_table(self.tokens, ["原文", "规则", "类别", "区间", "行:列", "动作"],
                   [(repr(t.text), t.rule, t.category, f"[{t.start},{t.end})", f"{t.line}:{t.column}", "跳过" if t.skip else "输出") for t in tokens])

    def show_step(self, index):
        if not self.compilation:
            return
        stage = self.stage.currentIndex()
        if stage == 1 and self.run:
            step = self.run.steps[index]
            edges = [step.source*len(self.dfa.alphabet)+step.group] if step.group >= 0 else []
            self.graph.show_subset(set(self.graph.nodes), set(self.graph.edges), [step.state], edges)
            text = self.input.toPlainText()
            selection = QTextEdit.ExtraSelection()
            cursor = self.input.textCursor()
            start = step.start if step.kind != "error" else step.position
            end = max(start+1, step.position) if step.kind == "error" else step.position
            cursor.setPosition(len(text[:start].encode("utf-16-le"))//2)
            cursor.setPosition(len(text[:end].encode("utf-16-le"))//2, QTextCursor.KeepAnchor)
            selection.cursor = cursor
            selection.format.setBackground(QColor("#fee2e2" if step.kind == "error" else "#dbeafe"))
            self.input.setExtraSelections([selection])
            self.show_tokens()
            extra = ""
            if index == len(self.run.steps)-1:
                extra = "\n" + "\n".join(e.message for e in self.run.errors)
            self.message.setText(step.message + f"\n扫描区间 [{step.start},{step.position})；最后接受 {step.last_accept}" + extra)
        elif stage == 0:
            if self.use_min.isChecked():
                self.message.setText(self.minimized.steps[index].message)
                self.graph.show_subset(set(self.graph.nodes), set(self.graph.edges))
                dfa_table(self.dtran, self.dfa)
                return
            step = self.compilation.dfa_steps[index]
            self.nfa_graph.show_subset(set(self.nfa_graph.nodes), set(self.nfa_graph.edges), step.nodes, step.edges)
            self.graph.show_step(step.node_count, step.committed, [step.source, step.target])
            dfa_table(self.dtran, self.dfa, step.committed)
            self.message.setText(step.message)
        elif stage == 2:
            step = self.compilation.nfa_steps[index]
            self.nfa_graph.show_step(step.node_count, step.edge_count, step.nodes, step.edges)
            self.message.setText(step.message)
