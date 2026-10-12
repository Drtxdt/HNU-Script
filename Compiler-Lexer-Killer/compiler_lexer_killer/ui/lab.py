import copy
import json
import os
from dataclasses import asdict
from pathlib import Path
from PySide6.QtCore import Qt, QTimer, QStandardPaths
from PySide6.QtGui import QAction, QKeySequence
from PySide6.QtWidgets import (QComboBox, QFileDialog, QMessageBox, QDialog, QVBoxLayout, QTextBrowser)
from ..core import simulate, check_equivalence, CompileError
from ..core.manual import document_from_machine, machine_from_document
from ..core.model import char_label
from ..project import load_project, save_project
from ..exporting import write_data, write_report
from .app import MainWindow
from .optimization import OptimizationPage
from .lexer_page import LexerPage
from .editor import EditorPage
from .exercises_page import ExercisesPage
from .jobs import Jobs
from .graph import GraphView
from .layout import machine_layout
from .export import export_scene
from .. import __version__
from .common import fill_table, label
from ..core.model import charset_label, set_label


class LabWindow(MainWindow):
    def __init__(self):
        self.loading_project = True
        self.document_dirty = False
        self.project_path = None
        self.machines = {}
        self.sim_dfa = None
        self.conversion = None
        super().__init__()
        self.setWindowTitle("Compiler Lexer Killer 2.0 · 词法分析实验室")
        self.setMinimumSize(850, 560)
        self.tabs.setUsesScrollButtons(True)
        self.tape.setTextFormat(Qt.PlainText)
        self.conversion_label = label("来源：当前正则／命名定义")
        self.tabs.widget(1).layout().insertWidget(0, self.conversion_label)
        self.sim_kind = QComboBox()
        for title, key in [("当前正则 DFA", "regular"), ("优化页最小化 DFA", "minimized"),
                           ("手绘 DFA／手绘 NFA 转换结果", "drawing"), ("词法规则 DFA（完整匹配）", "lexer")]:
            self.sim_kind.addItem(title, key)
        self.tabs.widget(2).layout().insertWidget(0, self.sim_kind)
        self.optimization = OptimizationPage(self.optimization_source)
        self.lexer_page = LexerPage()
        self.drawing = EditorPage()
        self.exercises = ExercisesPage(lambda: copy.deepcopy(self.drawing.document))
        for page, title in [(self.optimization, "04   优化比较"), (self.lexer_page, "05   词法分析"),
                            (self.drawing, "06   自由画图"), (self.exercises, "07   练习判题")]:
            self.tabs.addTab(page, title)
        self.compare_jobs = Jobs(self)
        self.compare_jobs.result.connect(self.compared)
        self.compare_jobs.error.connect(lambda exc: self.statusBar().showMessage(str(exc)))
        self.optimization.result_ready.connect(lambda dfa: self.install_machine("minimized", dfa, self.optimization.data["layouts"]["min"]))
        self.lexer_page.machine_ready.connect(lambda dfa: self.install_machine("lexer", dfa, self.lexer_page.layouts["dfa"]))
        self.drawing.machine_ready.connect(lambda dfa, layout: self.install_machine("drawing", dfa, layout, True))
        self.drawing.construction_ready.connect(self.manual_conversion)
        self.drawing.changed.connect(self.drawing_changed)
        self.lexer_page.changed.connect(self.lexer_changed)
        self.exercises.changed.connect(self.mark_dirty)
        self.exercises.open_drawing.connect(lambda: self.tabs.setCurrentWidget(self.drawing))
        self.exercises.restore_drawing.connect(self.drawing.set_document)
        self.sim_kind.currentIndexChanged.connect(self.choose_simulator)
        for widget in (self.editor, self.input):
            widget.textChanged.connect(self.mark_dirty)
        self.mode.currentIndexChanged.connect(self.mark_dirty)
        self.entry.currentIndexChanged.connect(self.mark_dirty)
        self.make_menus()
        root = os.environ.get("CLK_DATA_DIR") or QStandardPaths.writableLocation(QStandardPaths.AppLocalDataLocation)
        self.recovery_path = Path(root) / "recovery.clkproj"
        self.autosave = QTimer(self)
        self.autosave.setInterval(30_000)
        self.autosave.timeout.connect(self.save_recovery)
        self.autosave.start()
        self.loading_project = False
        self.document_dirty = False
        if not os.environ.get("CLK_TESTING") and "--smoke-test" not in __import__("sys").argv:
            QTimer.singleShot(0, self.offer_recovery)

    def make_menus(self):
        menu = self.menuBar().addMenu("文件")
        for title, shortcut, callback in [("新建工程", "Ctrl+N", self.new_project), ("打开工程…", "Ctrl+O", self.open_project),
                ("保存工程", "Ctrl+S", self.save), ("工程另存为…", "Ctrl+Shift+S", lambda: self.save(True)),
                ("导出当前步骤图…", "", lambda: self.export_graph(False)), ("导出完整图…", "", lambda: self.export_graph(True)),
                ("导出转换表／Token 数据…", "", self.export_data), ("导出实验报告…", "", self.export_report)]:
            action = QAction(title, self)
            if shortcut:
                action.setShortcut(QKeySequence(shortcut))
            action.triggered.connect(lambda checked=False, cb=callback: cb())
            menu.addAction(action)
        menu = self.menuBar().addMenu("画布")
        for title, key in [("复制当前正则 NFA", "nfa"), ("复制当前正则 DFA", "regular"),
                           ("复制最小化 DFA", "minimized"), ("复制词法 NFA", "lexer_nfa"), ("复制词法 DFA", "lexer")]:
            action = QAction(title, self)
            action.triggered.connect(lambda checked=False, k=key: self.copy_machine(k))
            menu.addAction(action)
        compare = QAction("与当前正则比较语言", self)
        compare.triggered.connect(self.compare_drawing)
        menu.addAction(compare)
        cancel = QAction("取消后台任务", self)
        cancel.triggered.connect(self.cancel_all)
        self.menuBar().addAction(cancel)

    def resizeEvent(self, event):
        super().resizeEvent(event)
        if hasattr(self, "legend"):
            compact = self.height() < 760
            self.brand_subtitle.setVisible(not compact)
            self.legend.setVisible(not compact)
            self.brand_title.setStyleSheet("font-size: 20px; font-weight: 700" if compact else "")
            margin = 10 if compact else 22
            self.centralWidget().layout().setContentsMargins(margin, 8 if compact else 18, margin, 8 if compact else 12)

    @property
    def job_groups(self):
        return tuple(page.jobs for page in (self.optimization, self.lexer_page, self.drawing, self.exercises)) + (self.compare_jobs,)

    def mark_dirty(self, *_):
        if not self.loading_project:
            self.document_dirty = True
            self.setWindowTitle("Compiler Lexer Killer 2.0 · " + (self.project_path.name if self.project_path else "未命名工程") + " *")

    def invalidate(self, *_):
        self.conversion = None
        super().invalidate()
        self.machines.pop("regular", None)
        if hasattr(self, "optimization") and self.optimization.source.currentIndex() == 0:
            self.optimization.clear()
            self.machines.pop("minimized", None)
        if hasattr(self, "compare_jobs"):
            self.compare_jobs.invalidate()
        if hasattr(self, "sim_kind"):
            self.choose_simulator()

    def drawing_changed(self):
        self.machines.pop("drawing", None)
        if self.conversion is not None and self.conversion is not self.result:
            self.conversion = None
            self.dfa_timeline.set_steps(0)
            self.table.setRowCount(0)
            self.dfa_graph.clear()
            self.subset_nfa.clear()
            self.dfa_formula.setText("手绘输入已改变，请重新验证和转换。")
        self.compare_jobs.invalidate()
        self.exercises.jobs.invalidate()
        if self.optimization.source.currentIndex() == 2:
            self.optimization.clear()
            self.machines.pop("minimized", None)
        if self.sim_kind.currentData() in ("drawing", "minimized"):
            self.choose_simulator()
        self.mark_dirty()

    def lexer_changed(self):
        if self.lexer_page.compilation is None:
            self.machines.pop("lexer", None)
            if self.optimization.source.currentIndex() == 1:
                self.optimization.clear()
                self.machines.pop("minimized", None)
            if self.sim_kind.currentData() in ("lexer", "minimized"):
                self.choose_simulator()
        self.mark_dirty()

    def built(self, generation, result, layouts):
        if generation != self.generation or self._closing:
            return
        self.conversion = result
        self.conversion_label.setText("来源：当前正则／命名定义")
        self.conversion_layout = layouts["dfa"]
        self.regular_layouts = layouts
        super().built(generation, result, layouts)
        if self.result is result:
            self.install_machine("regular", result.dfa, layouts["dfa"])

    def manual_conversion(self, payload):
        from types import SimpleNamespace
        nfa, dfa, steps, nfa_layout, dfa_layout = payload
        self.conversion = SimpleNamespace(nfa=nfa, dfa=dfa, dfa_steps=steps)
        self.conversion_label.setText("来源：当前手绘 NFA；修改画布后请重新验证和转换。")
        self.conversion_layout = dfa_layout
        self.subset_nfa.load(nfa_layout)
        self.dfa_graph.load(dfa_layout)
        self.populate_table()
        self.dfa_timeline.set_steps(len(steps))

    def populate_table(self):
        result = self.conversion or self.result
        if not result:
            return
        dfa = result.dfa
        fill_table(self.table, ["状态", "NFA 集合", "类别"]+[charset_label(g) for g in dfa.alphabet],
                   [(f"S{s.id}" + (" ✓" if s.accepting else ""), set_label(s.nfa_states),
                     s.winner.output if s.winner else "—", *["" for _ in dfa.alphabet]) for s in dfa.states])

    def show_dfa(self, index):
        result = self.conversion or self.result
        if not result:
            return
        step, dfa = result.dfa_steps[index], result.dfa
        self.subset_nfa.show_subset(set(self.subset_nfa.nodes), set(self.subset_nfa.edges), step.nodes, step.edges)
        self.dfa_graph.show_step(step.node_count, step.committed, [step.source, step.target],
                                 [step.source*len(dfa.alphabet)+step.group] if step.kind == "transition" else [])
        for s, row in enumerate(dfa.transitions):
            self.table.setRowHidden(s, s >= step.node_count)
            for g, t in enumerate(row):
                item = self.table.item(s,g+3)
                if item:
                    item.setText(f"S{t}" if s*len(dfa.alphabet)+g < step.committed else "")
        self.dfa_formula.setText(step.message)

    def select_dfa(self, number):
        result = self.conversion or self.result
        if result and 0 <= number < len(result.dfa.states):
            state = result.dfa.states[number]
            self.subset_nfa.show_subset(set(self.subset_nfa.nodes), set(self.subset_nfa.edges), state.nfa_states)
            self.statusBar().showMessage(f"S{number} = {set_label(state.nfa_states)}")

    def install_machine(self, key, dfa, layout, select=False):
        self.machines[key] = (dfa, layout)
        if select:
            self.sim_kind.setCurrentIndex(self.sim_kind.findData(key))
            self.tabs.setCurrentIndex(2)
        if self.sim_kind.currentData() == key:
            self.choose_simulator()

    def choose_simulator(self, *_):
        self.invalidate_simulation()
        entry = self.machines.get(self.sim_kind.currentData())
        self.sim_dfa = entry[0] if entry else None
        self.run_button.setEnabled(entry is not None)
        if entry:
            self.sim_graph.load(entry[1])
        else:
            self.sim_graph.clear()
            self.sim_formula.setText("所选来源尚未生成，或输入已改变。")
        if hasattr(self, "optimization"):
            self.tab_changed(self.tabs.currentIndex())

    def run_simulation(self):
        if not self.sim_dfa:
            return
        text = self.input.toPlainText()
        if len(text) > 4096:
            self.sim_formula.setText("教学模拟最多 4096 个字符。")
            return
        self.simulation = simulate(self.sim_dfa, text)
        self.sim_log.clear()
        self.sim_log.addItems([s.message for s in self.simulation.steps])
        self.sim_timeline.set_steps(len(self.simulation.steps))

    def show_simulation(self, index):
        if not self.simulation or not self.sim_dfa:
            return
        step = self.simulation.steps[index]
        edge = [step.source*len(self.sim_dfa.alphabet)+step.group] if step.group >= 0 else []
        self.sim_graph.show_subset(set(self.sim_graph.nodes), set(self.sim_graph.edges), [step.target], edge)
        self.sim_formula.setText(step.message)
        self.sim_log.setCurrentRow(index)
        text = self.input.toPlainText()
        position = step.position
        shown = " ".join(("【"+char_label(c)+"】") if i == position-1 else char_label(c)
                         for i,c in enumerate(text) if max(0,position-20) <= i < position+30)
        self.tape.setText(f"已读 {position}/{len(text)}：{shown or 'ε（空串）'}")
        if index == len(self.simulation.steps)-1 and self.simulation.failure_position is not None:
            failure = self.simulation.failure_position
            self.sim_formula.setText(step.message + ("\n输入结束仍未接受" if failure == len(text) else f"\n首次失败：第 {failure+1} 个字符"))

    def tab_changed(self, index):
        super().tab_changed(index)
        if hasattr(self, "optimization"):
            for page in (self.optimization, self.lexer_page):
                page.timeline.stop()
            show_input = index == 0 or (index == 1 and self.conversion is self.result) or (index == 2 and self.sim_kind.currentData() == "regular")
            self.splitter.widget(0).setVisible(show_input)
            self.presets.setVisible(show_input)
            QTimer.singleShot(0, self.fit_extra_graphs)

    def fit_extra_graphs(self):
        for graph in (self.optimization.before, self.optimization.after, self.lexer_page.graph,
                      self.lexer_page.nfa_graph, self.exercises.graph):
            if graph.isVisible():
                graph.fit()

    def optimization_source(self, index):
        if index == 0:
            return "regex", self.editor.toPlainText(), self.mode.currentData(), self.entry.currentText() or None
        if index == 1:
            return "lexer", self.lexer_page.definitions.toPlainText(), self.lexer_page.rule_specs()
        return "drawing", copy.deepcopy(self.drawing.document)

    def copy_machine(self, key):
        try:
            if key == "nfa":
                machine = self.result.nfa
            elif key == "lexer_nfa":
                machine = self.lexer_page.compilation.nfa
            else:
                machine = self.machines[key][0]
            self.drawing.set_document(document_from_machine(machine))
            self.tabs.setCurrentWidget(self.drawing)
            self.drawing.auto_layout()
        except (AttributeError, KeyError):
            self.statusBar().showMessage("请先生成对应自动机。")

    def compare_drawing(self):
        if not self.result:
            self.statusBar().showMessage("请先生成目标正则自动机。")
            return
        document, target = copy.deepcopy(self.drawing.document), self.result.dfa
        self.compare_jobs.start(lambda cancelled: check_equivalence(machine_from_document(document), target, cancelled=cancelled))

    def compared(self, result):
        message = result.message + (f"；最短反例 {result.counterexample!r}" if result.counterexample is not None else "")
        self.drawing.message.setText(message)
        self.statusBar().showMessage(message)

    def snapshot(self):
        return {"format_version": 1, "app_version": __version__, "source": self.editor.toPlainText(), "mode": self.mode.currentData(),
                "entry": self.entry.currentText(), "input": self.input.toPlainText(), "definitions": self.lexer_page.definitions.toPlainText(),
                "rules": [asdict(r) for r in self.lexer_page.rule_specs()], "lexer_input": self.lexer_page.input.toPlainText(),
                "recover": self.lexer_page.recover.isChecked(), "drawing": copy.deepcopy(self.drawing.document),
                "exercise": self.exercises.state(), "mistakes": copy.deepcopy(self.exercises.mistakes),
                "view": {"tab": self.tabs.currentIndex(), "sim_source": self.sim_kind.currentData()}}

    def apply_project(self, data):
        from ..core import RuleSpec
        self.cancel_all()
        self.loading_project = True
        try:
            self.machines.clear()
            self.mode.setCurrentIndex(self.mode.findData(data.get("mode", "expression")))
            self.editor.setPlainText(data.get("source", ""))
            if data.get("entry"):
                self.entry.setCurrentText(data["entry"])
            self.input.setPlainText(data.get("input", ""))
            self.lexer_page.definitions.setPlainText(data.get("definitions", ""))
            self.lexer_page.set_rules([RuleSpec(**r) for r in data.get("rules", [])])
            self.lexer_page.input.setPlainText(data.get("lexer_input", ""))
            self.lexer_page.recover.setChecked(bool(data.get("recover", False)))
            self.drawing.set_document(data.get("drawing", {"kind":"nfa", "nodes":[], "edges":[]}), undoable=False)
            self.exercises.restore(data.get("exercise", {}))
            self.exercises.mistakes = data.get("mistakes", [])
            self.exercises.refresh_history()
            view = data.get("view", {})
            self.tabs.setCurrentIndex(max(0, min(int(view.get("tab", 0)), self.tabs.count()-1)))
            self.sim_kind.setCurrentIndex(max(0,self.sim_kind.findData(view.get("sim_source", "regular"))))
            self.example_note.setText("已恢复工程；派生算法结果需要重新生成。")
            self.choose_simulator()
        finally:
            self.loading_project = False
        self.document_dirty = False

    def confirm_discard(self):
        if not self.document_dirty:
            return True
        answer = QMessageBox.question(self, "未保存的工程", "当前工程有修改，是否保存？",
                                      QMessageBox.Save | QMessageBox.Discard | QMessageBox.Cancel, QMessageBox.Save)
        return self.save() if answer == QMessageBox.Save else answer == QMessageBox.Discard

    def new_project(self):
        if self.confirm_discard():
            self.apply_project({"format_version":1, "source":"(a|b)*abb", "input":"abababb"})
            self.project_path = None
            self.setWindowTitle("Compiler Lexer Killer 2.0 · 未命名工程")

    def open_project(self):
        if not self.confirm_discard():
            return
        name, _ = QFileDialog.getOpenFileName(self, "打开工程", "", "词法实验工程 (*.clkproj)")
        if name:
            try:
                data = load_project(name)
                self.apply_project(data)
                self.project_path = Path(name)
                self.setWindowTitle("Compiler Lexer Killer 2.0 · " + self.project_path.name)
            except Exception as exc:
                QMessageBox.warning(self, "打开失败", str(exc))

    def save(self, save_as=False):
        path = self.project_path
        if save_as or path is None:
            name, _ = QFileDialog.getSaveFileName(self, "保存工程", str(path or "实验.clkproj"), "词法实验工程 (*.clkproj)")
            if not name:
                return False
            path = Path(name)
            if path.suffix.lower() != ".clkproj":
                path = path.with_suffix(".clkproj")
        try:
            save_project(path, self.snapshot())
            self.project_path = path
            self.document_dirty = False
            self.setWindowTitle("Compiler Lexer Killer 2.0 · " + path.name)
            self.statusBar().showMessage("工程已保存；算法结果在打开后重新生成。")
            return True
        except Exception as exc:
            QMessageBox.warning(self, "保存失败", str(exc))
            return False

    def save_recovery(self):
        if self.document_dirty:
            try:
                self.recovery_path.parent.mkdir(parents=True, exist_ok=True)
                save_project(self.recovery_path, self.snapshot())
            except (OSError, CompileError, ValueError) as exc:
                self.statusBar().showMessage(f"自动恢复文件未写入：{exc}")

    def offer_recovery(self):
        if self.recovery_path.exists():
            if QMessageBox.question(self, "恢复工程", "发现上次未正常关闭的工程，是否恢复？") == QMessageBox.Yes:
                try:
                    self.apply_project(load_project(self.recovery_path))
                    self.mark_dirty()
                except Exception as exc:
                    QMessageBox.warning(self, "恢复失败", str(exc))

    def active_graph(self, full=False):
        index = self.tabs.currentIndex()
        if index == 5:
            return self.drawing.scene, None
        graph = self.nfa_graph if index == 0 else self.dfa_graph if index == 1 else self.sim_graph
        layout = None
        if index == 3:
            graph = self.optimization.after
            data = self.optimization.data
            if data:
                machine = data["min"].dfa if self.optimization.kind.currentIndex() == 0 else data["simple"].nfa if data["simple"] else None
                if machine:
                    layout = data["layouts"]["min" if self.optimization.kind.currentIndex() == 0 else "simple_final"]
        elif index == 4:
            graph = self.lexer_page.nfa_graph if self.lexer_page.view_tabs.currentWidget() is self.lexer_page.nfa_graph else self.lexer_page.graph
            if self.lexer_page.compilation:
                layout = self.lexer_page.layouts["nfa" if graph is self.lexer_page.nfa_graph else "min" if self.lexer_page.use_min.isChecked() else "dfa"]
        elif index == 6:
            graph = self.exercises.graph
            layout = self.exercises.reference_layout
        elif index == 0 and self.result:
            layout = self.regular_layouts["nfa"]
        elif index == 1 and self.conversion:
            layout = self.conversion_layout
        elif index == 2:
            entry = self.machines.get(self.sim_kind.currentData())
            layout = entry[1] if entry else None
        if not graph.nodes:
            raise ValueError("请先生成当前页面的自动机")
        if full and layout:
            temporary = GraphView("导出快照")
            temporary.load(layout)
            return temporary.scene, temporary
        return graph.scene, None

    def export_graph(self, full=False):
        name, _ = QFileDialog.getSaveFileName(self, "导出自动机", "自动机.svg", "SVG 图形 (*.svg);;PNG 图片 (*.png)")
        if name:
            try:
                scene, owner = self.active_graph(full)
                export_scene(scene, name)
                if owner:
                    owner.deleteLater()
                self.statusBar().showMessage("已导出："+name)
            except Exception as exc:
                QMessageBox.warning(self, "导出失败", str(exc))

    def active_data(self):
        if self.tabs.currentIndex() == 6:
            return None, None, (), self.exercises.prompt.text()
        if self.tabs.currentIndex() == 1 and self.conversion:
            return self.conversion.dfa, None, self.conversion.dfa_steps, "子集构造：当前页面的 NFA → DFA。"
        if self.tabs.currentIndex() == 5:
            entry = self.machines.get("drawing")
            return entry[0] if entry else None, None, (), "手绘自动机；图形按保存的坐标导出。"
        if self.tabs.currentIndex() == 4 and self.lexer_page.compilation:
            return self.lexer_page.dfa, self.lexer_page.run, self.lexer_page.compilation.dfa_steps, self.lexer_page.note.text()
        if self.tabs.currentIndex() == 3 and self.optimization.data:
            data = self.optimization.data
            if self.optimization.kind.currentIndex() == 1 and data["simple"]:
                return data["simple"].dfa, None, data["steps"] or data["simple"].nfa_steps, self.optimization.note.text()
            return data["min"].dfa, None, data["min"].steps, self.optimization.note.text()
        if self.tabs.currentIndex() == 2 and self.sim_dfa:
            return self.sim_dfa, None, (), self.sim_formula.text()
        if self.result:
            return self.result.dfa, None, self.result.nfa_steps+self.result.dfa_steps, self.example_note.text()
        return None, None, (), ""

    def export_data(self):
        name, _ = QFileDialog.getSaveFileName(self, "导出转换表或 Token", "实验数据.json", "JSON (*.json);;CSV (*.csv)")
        if name:
            try:
                dfa, run, _, _ = self.active_data()
                if dfa is None:
                    raise ValueError("请先生成结果")
                write_data(name, dfa, run)
                self.statusBar().showMessage("数据已导出。")
            except Exception as exc:
                QMessageBox.warning(self, "导出失败", str(exc))

    def export_report(self):
        name, _ = QFileDialog.getSaveFileName(self, "导出实验报告", "实验报告.md", "Markdown (*.md)")
        if name:
            try:
                path = Path(name)
                scene, owner = self.active_graph(True)
                export_scene(scene, path.with_suffix(".svg"))
                if owner:
                    owner.deleteLater()
                dfa, run, steps, note = self.active_data()
                index = self.tabs.currentIndex()
                context, strategy, conclusion = "regular", "标准 Thompson 构造与子集构造", ""
                if index == 4:
                    context = "lexer"
                    strategy = "联合 NFA、子集构造、最长匹配及规则优先级" + ("；DFA 最小化" if self.lexer_page.use_min.isChecked() else "")
                elif index == 6:
                    context, strategy, conclusion = "exercise", "练习参考自动机", self.exercises.feedback.text()
                elif index == 5:
                    context, strategy = "drawing", "手绘自动机"
                elif index == 3:
                    context = ("regular", "lexer", "drawing")[self.optimization.source.currentIndex()]
                    strategy = "课件边界简化构造" if self.optimization.kind.currentIndex() == 1 else "DFA 逐轮分组最小化"
                    conclusion = self.optimization.note.text()
                elif index == 2:
                    key = self.sim_kind.currentData()
                    context = "drawing" if key == "drawing" else "lexer" if key == "lexer" else "regular"
                    if key == "minimized":
                        context = ("regular", "lexer", "drawing")[self.optimization.source.currentIndex()]
                        strategy = "最小化 DFA 完整字符串模拟"
                    if self.simulation:
                        conclusion = ("接受" if self.simulation.accepted else "拒绝") + f"输入 {self.input.toPlainText()!r}；最终状态 S{self.simulation.final_state}。"
                elif index == 1 and self.conversion is not self.result:
                    context, strategy = "drawing", "手绘 NFA 的子集构造"
                write_report(path, self.snapshot(), dfa, run, path.with_suffix(".svg").name, steps, note,
                             context=context, strategy=strategy, conclusion=conclusion)
                self.statusBar().showMessage("报告和同目录 SVG 已导出。")
            except Exception as exc:
                QMessageBox.warning(self, "导出失败", str(exc))

    def cancel_all(self):
        self.cancel_build()
        for jobs in self.job_groups:
            jobs.invalidate()
        for timeline in self.timelines + (self.optimization.timeline, self.lexer_page.timeline):
            timeline.stop()

    def help(self):
        dialog = QDialog(self)
        dialog.setWindowTitle("2.0 学习路线与语法")
        dialog.resize(800, 630)
        box = QVBoxLayout(dialog)
        browser = QTextBrowser()
        browser.setMarkdown("""# 词法分析实验室 2.0

1. 在构造页输入正则或命名定义，查看 AST、NFA、子集构造与 DTran。
2. 在优化页选择来源，查看课件简化构造或 DFA 分组最小化。
3. 在模拟页选择已生成的自动机，逐字符验证输入。
4. 在词法分析页编辑公共定义和有序规则，先构建，再分析文本。
5. 在画图页双击创建状态，连线模式点击两个端点；通过表格修改初态、终态与类别。
6. 在练习页生成题目，填写 JSON 或提交画布。语言判题不比较状态编号，错误时给出最短反例。

## 输入语法
单表达式模式普通字符是字面量。命名定义／词法规则中，裸名称引用公共定义，字面量加单或双引号；规则名称不能互相引用。支持 ε、|、隐式连接和 ·、*、+、?、括号、字符类、范围与 ASCII 取反。优先级：重复 > 连接 > 并。

转义使用反斜杠：\\n、\\r、\\t、\\xHH。句点是字面字符。引号和字符类外空白忽略；待扫描文本空白保留，须显式定义跳过规则。编辑器换行统一为 LF，API 与 CLI 文件输入保留原始换行。

## 词法分析
最长匹配优先，同长度取表中最靠前的启用规则。匹配失败或 EOF 时退回最后接受位置。任何启用规则都不能接受空串。默认遇错停止；继续模式每次记录错误并跳过一个字符。

## 图形与文件
滚轮缩放；生成图拖动画布；手绘页选择模式框选，空白处可使用滚动条平移。文件菜单保存 .clkproj、导出 SVG/PNG、报告和数据。画布菜单复制生成图或比较语言。工程包含输入、坐标与练习记录，算法结果重新生成。

课件简化是局部优化，不保证 NFA 全局最少。最小化保留获胜规则与动作。ASCII 域外字符不匹配；未列出的 ASCII 字符直接拒绝。默认限制为 512 个 NFA 状态、256 个 DFA 状态、20,000 条步骤；超限不展示截断结果。
""")
        box.addWidget(browser)
        dialog.exec()

    def closeEvent(self, event):
        if not self.confirm_discard():
            event.ignore()
            return
        self.autosave.stop()
        self._closing = True
        self.generation += 1
        for jobs in self.job_groups:
            jobs.shutdown()
        for worker in self.workers:
            worker.cancel_event.set()
            worker.wait()
        try:
            if self.recovery_path.exists():
                self.recovery_path.unlink()
        except OSError:
            pass
        event.accept()
