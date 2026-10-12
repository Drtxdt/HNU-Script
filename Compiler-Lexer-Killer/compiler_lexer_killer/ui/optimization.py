from PySide6.QtCore import Signal
from PySide6.QtGui import QColor, QBrush
from PySide6.QtWidgets import QWidget, QVBoxLayout, QHBoxLayout, QPushButton, QComboBox, QSplitter, QTableWidget
from ..core import compile, compile_lexer, minimize, determinize
from ..core.model import NFA, Context, Limits, charset_label
from ..core.replay import Event, Replay, graph_replay
from .graph import GraphView
from .timeline import Timeline
from .jobs import Jobs
from .layout import machine_layout, history_layout
from .common import label, fill_table


class OptimizationPage(QWidget):
    result_ready = Signal(object)

    def __init__(self, provider, parent=None):
        super().__init__(parent)
        self.provider, self.data = provider, None
        box = QVBoxLayout(self)
        row = QHBoxLayout()
        self.source = QComboBox()
        self.source.addItems(["当前正则构造", "当前词法规则", "当前手绘自动机"])
        self.kind = QComboBox()
        self.kind.addItems(["DFA 最小化", "课件 NFA 简化"])
        self.build_button = QPushButton("计算优化与映射")
        self.cancel = QPushButton("取消")
        for widget in (self.source, self.kind, self.build_button, self.cancel):
            row.addWidget(widget)
        box.addLayout(row)
        self.note = label("课件简化构造是局部优化；DFA 最小化通过逐轮分组得到确定结果。")
        box.addWidget(self.note)
        split = QSplitter()
        self.before, self.after = GraphView("优化前"), GraphView("优化后")
        split.addWidget(self.before)
        split.addWidget(self.after)
        box.addWidget(split, 3)
        self.table = QTableWidget()
        box.addWidget(self.table, 1)
        self.message = label()
        box.addWidget(self.message)
        self.timeline = Timeline()
        box.addWidget(self.timeline)
        self.jobs = Jobs(self)
        self.jobs.result.connect(self.loaded)
        self.jobs.error.connect(lambda exc: self.message.setText(str(exc)))
        self.jobs.busy.connect(lambda value: self.build_button.setEnabled(not value))
        self.build_button.clicked.connect(self.build)
        self.cancel.clicked.connect(self.clear)
        self.source.currentIndexChanged.connect(self.clear)
        self.kind.currentIndexChanged.connect(self.switch)
        self.timeline.changed.connect(self.show_step)

    def clear(self, *_):
        self.jobs.invalidate()
        self.data = None
        self.timeline.set_steps(0)
        self.before.clear()
        self.after.clear()
        self.table.setRowCount(0)
        self.message.setText("结果已失效，请重新计算。")

    def build(self):
        try:
            payload = self.provider(self.source.currentIndex())
        except Exception as exc:
            self.message.setText(str(exc))
            return
        self.clear()
        self.message.setText("正在构造、最小化和布局…")
        def work(cancelled):
            raw, simple, steps = None, None, ()
            if payload[0] == "regex":
                _, source, mode, entry = payload
                raw = compile(source, mode, entry, cancelled=cancelled)
                simple = compile(source, mode, entry, construction="simplified", cancelled=cancelled)
                steps = simple.nfa_steps
            elif payload[0] == "lexer":
                _, definitions, rules = payload
                raw = compile_lexer(definitions, rules, cancelled=cancelled)
                simple = compile_lexer(definitions, rules, construction="simplified", cancelled=cancelled)
            else:
                from ..core.manual import machine_from_document
                raw = machine_from_document(payload[1])
            dfa = raw.dfa if hasattr(raw, "dfa") else (determinize(raw, Context(Limits(), cancelled))[0] if isinstance(raw, NFA) else raw)
            minimized = minimize(dfa, cancelled=cancelled)
            layouts = {"dfa": machine_layout(dfa, cancelled), "min": machine_layout(minimized.dfa, cancelled)}
            if simple:
                layouts["raw_nfa"] = machine_layout(raw.nfa, cancelled)
                layouts["simple"] = history_layout([s.graph for s in steps], cancelled) if steps else machine_layout(simple.nfa, cancelled)
                layouts["simple_final"] = machine_layout(simple.nfa, cancelled)
            return {"raw": raw, "simple": simple, "dfa": dfa, "min": minimized, "steps": steps, "layouts": layouts}
        self.jobs.start(work)

    def loaded(self, data):
        self.data = data
        self.result_ready.emit(data["min"].dfa)
        self.switch()

    def switch(self, *_):
        self.timeline.stop()
        if not self.data:
            return
        data = self.data
        if self.kind.currentIndex() == 0:
            self.before.load(data["layouts"]["dfa"])
            self.after.load(data["layouts"]["min"])
            self.replay = Replay([Event((("step", step),)) for step in data["min"].steps])
            self.timeline.set_steps(len(data["min"].steps))
            self.note.setText(f"DFA：{len(data['dfa'].states)} → {len(data['min'].dfa.states)} 状态；有效字母表外字符直接拒绝。")
        elif data["simple"]:
            self.before.load(data["layouts"]["raw_nfa"])
            self.after.load(data["layouts"]["simple"])
            self.replay = graph_replay(data["steps"])
            self.timeline.set_steps(max(1, len(data["steps"])))
            self.note.setText(f"NFA：{len(data['raw'].nfa.states)} → {len(data['simple'].nfa.states)} 状态；课件 126–134 页，不保证全局最少状态。")
        else:
            self.timeline.set_steps(0)
            self.message.setText("手绘图没有 AST；可进行 DFA 最小化，课件构造策略仅适用于正则或规则输入。")

    def show_step(self, index):
        if not self.data or not getattr(self, "replay", None):
            return
        data = self.data
        if self.kind.currentIndex() == 0:
            step = self.replay.at(index+1)["step"]
            self.message.setText(step.message)
            for i, part in enumerate(step.partitions):
                for state in part:
                    if state in self.before.nodes:
                        self.before.nodes[state].setBrush(QBrush(QColor.fromHsv((i*67)%360, 70, 250)))
            final = index == len(data["min"].steps)-1
            self.after.show_subset(set(self.after.nodes) if final else set(), set(self.after.edges) if final else set())
            fill_table(self.table, ["分组", "原 DFA 状态"], [(i, str(sorted(p))) for i, p in enumerate(step.partitions)])
        elif data["simple"]:
            if not data["steps"]:
                self.message.setText("各条规则独立简化后组合；类别与优先级保持不变。")
                return
            state = self.replay.at(index+1)
            step = state["step"]
            graph = step.graph
            nodes = {value for key, value in state.items() if key.startswith("node/")}
            indices = {value for key, value in state.items() if key.startswith("edge/")}
            self.after.show_subset(nodes, indices, step.nodes, accepts=graph.accepts)
            self.message.setText(step.message)
            fill_table(self.table, ["步骤", "当前状态数", "当前边数", "本步状态合并"],
                       [(index+1, len(graph.states), len(graph.edges), "; ".join(f"q{a} → q{b}" for a,b in step.merges) or "—")])
