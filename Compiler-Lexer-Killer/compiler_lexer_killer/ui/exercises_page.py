import json
from PySide6.QtCore import Signal
from PySide6.QtWidgets import (QWidget, QVBoxLayout, QHBoxLayout, QComboBox, QSpinBox,
    QPushButton, QPlainTextEdit, QSplitter, QListWidget)
from ..core import compile
from ..core.exercises import make_question, grade, reference, KINDS
from ..core.model import charset_label, set_label
from .graph import GraphView
from .jobs import Jobs
from .layout import machine_layout
from .common import label


class ExercisesPage(QWidget):
    changed = Signal()
    open_drawing = Signal()
    restore_drawing = Signal(object)

    def __init__(self, drawing_provider, parent=None):
        super().__init__(parent)
        self.drawing_provider, self.question, self.mistakes = drawing_provider, None, []
        self.reference_layout = None
        self.restoring = False
        box = QVBoxLayout(self)
        row = QHBoxLayout()
        self.kind, self.difficulty = QComboBox(), QComboBox()
        for key, text in KINDS.items():
            self.kind.addItem(text, key)
        self.difficulty.addItems(["基础", "进阶"])
        self.seed = QSpinBox()
        self.seed.setRange(0, 2_000_000_000)
        self.seed.setValue(1)
        self.generate, self.submit = QPushButton("生成题目"), QPushButton("提交答案／当前画布")
        for widget in (self.kind, self.difficulty, label("种子"), self.seed, self.generate, self.submit):
            row.addWidget(widget)
        box.addLayout(row)
        self.prompt = label("选择题型和种子，生成可复现的练习。画图题在画图页作答后提交。")
        box.addWidget(self.prompt)
        split = QSplitter()
        self.graph = GraphView("参考自动机")
        split.addWidget(self.graph)
        right = QWidget()
        form = QVBoxLayout(right)
        self.answer = QPlainTextEdit()
        self.answer.setPlaceholderText("按题目要求填写 JSON；画图题直接提交当前画布。")
        form.addWidget(self.answer, 2)
        buttons = QHBoxLayout()
        for text, callback in [("提示", self.hint), ("查看解析", self.solution), ("重新作答", self.retry),
                               ("打开画图", self.open_drawing.emit)]:
            button = QPushButton(text)
            button.clicked.connect(callback)
            buttons.addWidget(button)
        form.addLayout(buttons)
        self.feedback = label()
        form.addWidget(self.feedback)
        form.addWidget(label("本地错题 · 双击重新载入"))
        self.history = QListWidget()
        form.addWidget(self.history, 1)
        split.addWidget(right)
        split.setSizes([650, 450])
        box.addWidget(split, 1)
        self.jobs = Jobs(self)
        self.jobs.result.connect(self.loaded)
        self.jobs.error.connect(lambda exc: self.feedback.setText(str(exc)))
        self.generate.clicked.connect(self.make)
        self.submit.clicked.connect(self.check)
        self.answer.textChanged.connect(self.answer_changed)
        self.history.itemDoubleClicked.connect(self.restore_mistake)

    def answer_changed(self):
        self.jobs.invalidate()
        self.feedback.setText("答案已修改，尚未判题。")
        self.changed.emit()

    def make(self):
        self.restoring = False
        self.jobs.invalidate()
        self.question = None
        seed, difficulty, kind = self.seed.value(), "basic" if self.difficulty.currentIndex() == 0 else "advanced", self.kind.currentData()
        self.answer.clear()
        self.feedback.setText("生成题目与参考数据…")
        def work(cancelled):
            question = make_question(seed, difficulty, kind, cancelled=cancelled)
            machine = reference(question, cancelled=cancelled)
            return "question", question, machine_layout(machine, cancelled)
        self.jobs.start(work)

    def loaded(self, payload):
        if payload[0] == "question":
            _, self.question, self.reference_layout = payload
            prefix = "词法分类" if self.question["kind"] in ("tokens", "categories") else f"正则：{self.question['source']}"
            self.prompt.setText(f"{prefix}　／　种子 {self.question['seed']}\n{self.question['prompt']}")
            if self.question["kind"] not in ("automaton", "minimal", "categories", "tokens"):
                self.graph.load(self.reference_layout)
            else:
                self.graph.clear()
            self.feedback.setText("题目已就绪。")
            if not self.restoring:
                self.changed.emit()
            self.restoring = False
        else:
            result = payload[1]
            self.feedback.setText(("✓ " if result.correct else "未完成：" if result.correct is None else "需修正：") + result.message +
                                  (f"\n最短反例：{result.counterexample!r}" if result.counterexample is not None else ""))
            if result.correct is False and self.question:
                record = {"question": self.question, "answer": self.answer.toPlainText()}
                if self.question["kind"] in ("automaton", "minimal", "categories"):
                    record["drawing"] = self.drawing_provider()
                self.mistakes.append(record)
                self.mistakes = self.mistakes[-100:]
                self.refresh_history()
                self.changed.emit()

    def check(self):
        if not self.question:
            return
        try:
            answer = self.drawing_provider() if self.question["kind"] in ("automaton", "minimal", "categories") else json.loads(self.answer.toPlainText())
        except (ValueError, TypeError) as exc:
            self.feedback.setText(f"JSON 格式错误：{exc}")
            return
        question = dict(self.question)
        self.jobs.start(lambda cancelled: ("grade", grade(question, answer, cancelled=cancelled)))

    def hint(self):
        if self.question:
            self.feedback.setText(self.question["hint"])

    def solution(self):
        if not self.question:
            return
        expected = self.question.get("expected")
        self.feedback.setText("参考答案：\n" + json.dumps(expected, ensure_ascii=False) if expected is not None else
                              "构造并不是唯一答案。生成目标表达式的自动机，比较其语言；编号、布局可以不同。类别题按 reserved/id 判断。")
        if self.reference_layout:
            self.graph.load(self.reference_layout)

    def retry(self):
        self.answer.clear()
        self.feedback.setText("重新作答；画布可在画图页清空或修改。")

    def refresh_history(self):
        self.history.clear()
        self.history.addItems([f"{KINDS.get(r['question']['kind'], '练习')} · 种子 {r['question']['seed']}" for r in self.mistakes])

    def restore_mistake(self, item):
        record = self.mistakes[self.history.row(item)]
        if "drawing" in record:
            self.restore_drawing.emit(record["drawing"])
        self.restore({"question": record["question"], "answer": record["answer"]})

    def state(self):
        return {"question": self.question, "answer": self.answer.toPlainText()}

    def restore(self, state):
        question = state.get("question")
        self.jobs.invalidate()
        self.question = None
        self.graph.clear()
        self.reference_layout = None
        self.answer.setPlainText(state.get("answer", ""))
        if isinstance(question, dict) and question.get("kind") in KINDS:
            # Recreate expected answers; never trust cached grading results from files.
            self.question = make_question(question.get("seed", 1), question.get("difficulty", "basic"), question["kind"])
            self.kind.setCurrentIndex(self.kind.findData(self.question["kind"]))
            self.seed.setValue(self.question["seed"])
            self.difficulty.setCurrentIndex(self.question["difficulty"] == "advanced")
            self.prompt.setText(f"正则：{self.question['source']}\n{self.question['prompt']}")
            self.feedback.setText("已恢复题目与答案。点击生成可重新查看参考图。")
            self.restoring = True
            restored = dict(self.question)
            self.jobs.start(lambda cancelled: ("question", restored, machine_layout(reference(restored, cancelled=cancelled), cancelled)))
