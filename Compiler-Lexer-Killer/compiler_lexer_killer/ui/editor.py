import copy
import json
import math
from PySide6.QtCore import Qt, QPointF, QTimer, Signal
from PySide6.QtGui import QPen, QBrush, QColor, QPainterPath, QPolygonF, QUndoStack, QUndoCommand, QKeySequence, QShortcut
from PySide6.QtWidgets import (QWidget, QVBoxLayout, QHBoxLayout, QComboBox, QPushButton, QLineEdit,
    QGraphicsScene, QGraphicsEllipseItem, QGraphicsPathItem, QGraphicsPolygonItem, QGraphicsSimpleTextItem,
    QGraphicsItem, QGraphicsView, QSplitter, QTableWidget, QTableWidgetItem, QApplication, QHeaderView)
from ..core.manual import validate_document, machine_from_document, document_from_machine
from ..core import determinize
from ..core.model import NFA, Context, Limits
from .graph import Canvas
from .jobs import Jobs
from .layout import machine_layout
from .common import label


class EditCommand(QUndoCommand):
    def __init__(self, editor, before, after, text):
        super().__init__(text)
        self.editor, self.before, self.after = editor, copy.deepcopy(before), copy.deepcopy(after)

    def undo(self):
        self.editor.apply(self.before)

    def redo(self):
        self.editor.apply(self.after)


class EditNode(QGraphicsEllipseItem):
    def __init__(self, node, editor):
        super().__init__(-29, -29, 58, 58)
        self.number, self.editor = node["id"], editor
        self.setFlags(QGraphicsItem.ItemIsMovable | QGraphicsItem.ItemIsSelectable | QGraphicsItem.ItemSendsGeometryChanges)
        self.setBrush(QBrush(QColor("#ecfdf5" if node.get("accept") else "#ffffff")))
        self.setPen(QPen(QColor("#059669" if node.get("accept") else "#64748b"), 2))
        self.setZValue(3)
        text = QGraphicsSimpleTextItem(node.get("label") or f"q{node['id']}", self)
        text.setPos(-text.boundingRect().width()/2, -text.boundingRect().height()/2)
        text.setAcceptedMouseButtons(Qt.NoButton)
        if node.get("accept"):
            inner = QGraphicsEllipseItem(-24, -24, 48, 48, self)
            inner.setPen(QPen(QColor("#059669"), 1.5))
            inner.setAcceptedMouseButtons(Qt.NoButton)
        if node.get("start"):
            marker = QGraphicsSimpleTextItem("开始 →", self)
            marker.setPos(-marker.boundingRect().width()-32, -10)
            marker.setAcceptedMouseButtons(Qt.NoButton)
        if node.get("category"):
            category = QGraphicsSimpleTextItem(node["category"], self)
            category.setPos(-category.boundingRect().width()/2, 33)
            category.setAcceptedMouseButtons(Qt.NoButton)
        self.setPos(node.get("x", 0), node.get("y", 0))

    def itemChange(self, change, value):
        if change == QGraphicsItem.ItemPositionHasChanged and hasattr(self.editor, "edge_items"):
            self.editor.update_edges()
        return super().itemChange(change, value)

    def mousePressEvent(self, event):
        if self.editor.tool.currentIndex() == 1 and event.button() == Qt.LeftButton:
            self.editor.connect_node(self.number)
            event.accept()
            return
        super().mousePressEvent(event)

    def mouseReleaseEvent(self, event):
        super().mouseReleaseEvent(event)
        if self.editor.tool.currentIndex() == 0:
            QTimer.singleShot(0, self.editor.commit_positions)


class EditEdge(QGraphicsPathItem):
    def __init__(self, edge, index, editor):
        super().__init__()
        self.edge, self.index, self.editor = edge, index, editor
        self.setFlags(QGraphicsItem.ItemIsSelectable)
        self.setPen(QPen(QColor("#64748b"), 2))
        self.arrow = QGraphicsPolygonItem(self)
        self.arrow.setBrush(QBrush(QColor("#64748b")))
        self.arrow.setPen(QPen(Qt.NoPen))
        self.arrow.setAcceptedMouseButtons(Qt.NoButton)
        self.text = QGraphicsSimpleTextItem(edge["label"] if len(edge["label"]) < 22 else edge["label"][:19]+"…", self)
        self.text.setToolTip(edge["label"])
        self.text.setBrush(QBrush(QColor("#1e40af")))
        self.text.setAcceptedMouseButtons(Qt.NoButton)

    def itemChange(self, change, value):
        if change == QGraphicsItem.ItemSelectedHasChanged:
            self.setPen(QPen(QColor("#2563eb" if value else "#64748b"), 3 if value else 2))
        return super().itemChange(change, value)

    def update_path(self):
        a = self.editor.node_items[self.edge["source"]].pos()
        b = self.editor.node_items[self.edge["target"]].pos()
        same = [i for i, e in enumerate(self.editor.document["edges"]) if {e["source"], e["target"]} == {self.edge["source"], self.edge["target"]}]
        rank = same.index(self.index)
        path = QPainterPath()
        if a == b:
            radius = 65 + rank*25
            start, end = a+QPointF(-18,-23), a+QPointF(18,-23)
            path.moveTo(start)
            path.cubicTo(a+QPointF(-radius,-radius-35), a+QPointF(radius,-radius-35), end)
            center = a+QPointF(0, -radius-15)
        else:
            dx, dy = b.x()-a.x(), b.y()-a.y()
            length = max(1, math.hypot(dx, dy))
            unit = QPointF(dx/length, dy/length)
            start, end = a+unit*30, b-unit*31
            offset = (rank-(len(same)-1)/2)*45
            if self.edge["source"] > self.edge["target"]:
                offset = -offset
            center = (a+b)/2 + QPointF(-dy/length, dx/length)*offset
            path.moveTo(start)
            path.quadTo(center, end)
        self.setPath(path)
        tip, previous = path.pointAtPercent(1), path.pointAtPercent(.96)
        dx, dy = tip.x()-previous.x(), tip.y()-previous.y()
        length = max(1, math.hypot(dx,dy))
        dx, dy = dx/length, dy/length
        self.arrow.setPolygon(QPolygonF([tip, tip-QPointF(dx*11+dy*5,dy*11-dx*5), tip-QPointF(dx*11-dy*5,dy*11+dx*5)]))
        self.text.setPos(center.x()-self.text.boundingRect().width()/2, center.y()-20)


class EditScene(QGraphicsScene):
    def __init__(self, editor):
        super().__init__(editor)
        self.editor = editor

    def mouseDoubleClickEvent(self, event):
        if not self.itemAt(event.scenePos(), self.editor.view.transform()):
            self.editor.add_node(event.scenePos())
            event.accept()
        else:
            super().mouseDoubleClickEvent(event)


class EditorPage(QWidget):
    changed = Signal()
    machine_ready = Signal(object, object)
    construction_ready = Signal(object)

    def __init__(self, parent=None):
        super().__init__(parent)
        self.document = {"kind": "nfa", "nodes": [], "edges": []}
        self.node_items, self.edge_items, self.pending_source = {}, [], None
        self.loading = False
        self.undo = QUndoStack(self)
        box = QVBoxLayout(self)
        row = QHBoxLayout()
        self.kind = QComboBox()
        self.kind.addItems(["NFA", "DFA"])
        self.tool = QComboBox()
        self.tool.addItems(["选择／拖动", "连线：依次点击两个状态", "平移画布"])
        self.condition = QLineEdit("a")
        self.condition.setPlaceholderText("边条件：a、[a-z]、ε")
        for widget in (self.kind, self.tool, self.condition):
            row.addWidget(widget)
        for text, callback in [("新增状态", self.add_node), ("删除选中", self.delete_selected),
                               ("清空", self.clear_drawing), ("适应窗口", self.fit)]:
            button = QPushButton(text)
            button.clicked.connect(callback)
            row.addWidget(button)
        box.addLayout(row)
        controls = QHBoxLayout()
        for text, callback in [("撤销", self.undo.undo), ("重做", self.undo.redo), ("复制", self.copy_selected),
                               ("粘贴", self.paste), ("自动布局", self.auto_layout), ("验证并用于模拟／转换", self.build)]:
            button = QPushButton(text)
            button.clicked.connect(callback)
            controls.addWidget(button)
        box.addLayout(controls)
        box.addWidget(label("双击空白处创建状态；连线模式连续点击同一状态可画自环。表格可编辑名称、初态、终态、类别和边条件。"))
        split = QSplitter()
        self.scene = EditScene(self)
        self.view = Canvas(self.scene)
        self.view.setDragMode(QGraphicsView.RubberBandDrag)
        from PySide6.QtGui import QPainter
        self.view.setRenderHint(QPainter.Antialiasing)
        split.addWidget(self.view)
        tables = QWidget()
        right = QVBoxLayout(tables)
        self.nodes_table, self.edges_table = QTableWidget(), QTableWidget()
        self.nodes_table.setColumnCount(5)
        self.nodes_table.setHorizontalHeaderLabels(["ID", "名称", "初态", "终态", "类别"])
        self.edges_table.setColumnCount(3)
        self.edges_table.setHorizontalHeaderLabels(["起点 ID", "终点 ID", "条件"])
        self.nodes_table.horizontalHeader().setSectionResizeMode(QHeaderView.Stretch)
        self.edges_table.horizontalHeader().setSectionResizeMode(QHeaderView.Stretch)
        right.addWidget(self.nodes_table)
        right.addWidget(self.edges_table)
        split.addWidget(tables)
        split.setSizes([780, 390])
        box.addWidget(split, 1)
        self.message = label("新建 NFA 草稿。至少需要一个状态，并指定唯一初态。")
        box.addWidget(self.message)
        self.jobs = Jobs(self)
        self.jobs.result.connect(self.loaded)
        self.jobs.error.connect(lambda exc: self.message.setText(str(exc)))
        self.nodes_table.itemChanged.connect(self.edit_node_cell)
        self.edges_table.itemChanged.connect(self.edit_edge_cell)
        self.kind.currentIndexChanged.connect(self.change_kind)
        self.tool.currentIndexChanged.connect(self.change_tool)
        self.scene.selectionChanged.connect(self.select_table)
        for key, callback in [("Delete", self.delete_selected), ("Ctrl+Z", self.undo.undo), ("Ctrl+Y", self.undo.redo),
                               ("Ctrl+C", self.copy_selected), ("Ctrl+V", self.paste)]:
            shortcut = QShortcut(QKeySequence(key), self)
            shortcut.setContext(Qt.WidgetWithChildrenShortcut)
            shortcut.activated.connect(callback)

    def mutate(self, document, title):
        if document == self.document:
            return
        try:
            validate_document(document)
        except Exception as exc:
            self.message.setText(str(exc))
            self.refresh()
            return
        self.undo.push(EditCommand(self, self.document, document, title))

    def apply(self, document):
        self.jobs.invalidate()
        selected = {item.number for item in self.scene.selectedItems() if isinstance(item, EditNode)}
        self.document = copy.deepcopy(document)
        self.pending_source = None
        self.refresh()
        for number in selected:
            if number in self.node_items:
                self.node_items[number].setSelected(True)
        self.changed.emit()

    def set_document(self, document, undoable=True):
        validate_document(document)
        if undoable:
            self.mutate(document, "导入自动机")
        else:
            self.undo.clear()
            self.apply(document)
        self.fit()

    def refresh(self):
        self.loading = True
        self.node_items, self.edge_items = {}, []
        self.scene.clear()
        self.kind.setCurrentIndex(0 if self.document["kind"] == "nfa" else 1)
        for node in self.document["nodes"]:
            item = EditNode(node, self)
            self.node_items[node["id"]] = item
            self.scene.addItem(item)
        for i, edge in enumerate(self.document["edges"]):
            item = EditEdge(edge, i, self)
            self.edge_items.append(item)
            self.scene.addItem(item)
        self.update_edges()
        self.nodes_table.setRowCount(len(self.document["nodes"]))
        for row, node in enumerate(self.document["nodes"]):
            for col, key in enumerate(("id", "label", "start", "accept", "category")):
                item = QTableWidgetItem(str(node.get(key, "")))
                if key == "id":
                    item.setFlags(Qt.ItemIsEnabled | Qt.ItemIsSelectable)
                elif key in ("start", "accept"):
                    item.setText("")
                    item.setFlags(Qt.ItemIsEnabled | Qt.ItemIsSelectable | Qt.ItemIsUserCheckable)
                    item.setCheckState(Qt.Checked if node.get(key) else Qt.Unchecked)
                self.nodes_table.setItem(row, col, item)
        self.edges_table.setRowCount(len(self.document["edges"]))
        for row, edge in enumerate(self.document["edges"]):
            for col, key in enumerate(("source", "target", "label")):
                self.edges_table.setItem(row, col, QTableWidgetItem(str(edge[key])))
        self.loading = False

    def update_edges(self):
        for edge in self.edge_items:
            edge.update_path()
        self.scene.setSceneRect(self.scene.itemsBoundingRect().adjusted(-100,-100,100,100))

    def fit(self):
        self.view.fitInView(self.scene.sceneRect(), Qt.KeepAspectRatio)
        if self.view.transform().m11() > 1.5:
            self.view.resetTransform()

    def add_node(self, position=None):
        if not isinstance(position, QPointF):
            position = self.view.mapToScene(self.view.viewport().rect().center()) + QPointF(len(self.document["nodes"])*12, 0)
        document = copy.deepcopy(self.document)
        number = max((n["id"] for n in document["nodes"]), default=-1)+1
        document["nodes"].append({"id": number, "label": f"q{number}", "start": not document["nodes"],
                                  "accept": False, "category": "", "x": position.x(), "y": position.y()})
        self.mutate(document, "新增状态")

    def connect_node(self, number):
        if self.pending_source is None:
            self.pending_source = number
            self.message.setText(f"已选择起点 q{number}，请点击终点（可点击自身）。")
            return
        document = copy.deepcopy(self.document)
        document["edges"].append({"source": self.pending_source, "target": number, "label": self.condition.text()})
        self.mutate(document, "添加迁移")
        self.pending_source = None

    def commit_positions(self):
        document = copy.deepcopy(self.document)
        for node in document["nodes"]:
            if node["id"] in self.node_items:
                point = self.node_items[node["id"]].pos()
                node.update(x=point.x(), y=point.y())
        self.mutate(document, "移动状态")

    def delete_selected(self):
        nodes = {item.number for item in self.scene.selectedItems() if isinstance(item, EditNode)}
        edges = {item.index for item in self.scene.selectedItems() if isinstance(item, EditEdge)}
        document = copy.deepcopy(self.document)
        document["nodes"] = [n for n in document["nodes"] if n["id"] not in nodes]
        document["edges"] = [e for i,e in enumerate(document["edges"]) if i not in edges and e["source"] not in nodes and e["target"] not in nodes]
        self.mutate(document, "删除所选状态与迁移")

    def clear_drawing(self):
        self.mutate({"kind": self.document["kind"], "nodes": [], "edges": []}, "清空画布")

    def copy_selected(self):
        ids = {item.number for item in self.scene.selectedItems() if isinstance(item, EditNode)}
        if not ids:
            return
        data = {"kind": self.document["kind"], "nodes": [n for n in self.document["nodes"] if n["id"] in ids],
                "edges": [e for e in self.document["edges"] if e["source"] in ids and e["target"] in ids]}
        QApplication.clipboard().setText(json.dumps(data, ensure_ascii=False))

    def paste(self):
        try:
            text = QApplication.clipboard().text()
            if len(text) > 1_000_000:
                raise ValueError("剪贴板内容过大")
            fragment = validate_document(json.loads(text))
            document = copy.deepcopy(self.document)
            offset = max((n["id"] for n in document["nodes"]), default=-1)+1
            mapping = {n["id"]: offset+i for i,n in enumerate(fragment["nodes"])}
            for n in fragment["nodes"]:
                n = dict(n, id=mapping[n["id"]], x=n.get("x",0)+45, y=n.get("y",0)+45, start=False)
                document["nodes"].append(n)
            document["edges"].extend(dict(e, source=mapping[e["source"]], target=mapping[e["target"]]) for e in fragment["edges"])
            self.mutate(document, "粘贴自动机片段")
        except Exception as exc:
            self.message.setText(f"无法粘贴：{exc}")

    def edit_node_cell(self, item):
        if self.loading:
            return
        document = copy.deepcopy(self.document)
        node = document["nodes"][item.row()]
        key = ("id", "label", "start", "accept", "category")[item.column()]
        if key in ("start", "accept"):
            value = item.checkState() == Qt.Checked
            if key == "start" and value:
                for n in document["nodes"]:
                    n["start"] = False
            node[key] = value
        else:
            node[key] = item.text()
        self.mutate(document, "修改状态属性")

    def edit_edge_cell(self, item):
        if self.loading:
            return
        document = copy.deepcopy(self.document)
        key = ("source", "target", "label")[item.column()]
        try:
            document["edges"][item.row()][key] = item.text() if key == "label" else int(item.text())
            self.mutate(document, "修改迁移")
        except ValueError:
            self.message.setText("端点 ID 必须是整数")
            self.refresh()

    def select_table(self):
        if self.loading:
            return
        selected = self.scene.selectedItems()
        if len(selected) == 1 and isinstance(selected[0], EditNode):
            row = next(i for i,n in enumerate(self.document["nodes"]) if n["id"] == selected[0].number)
            self.nodes_table.setCurrentCell(row, 1)

    def change_kind(self, index):
        if not self.loading:
            document = copy.deepcopy(self.document)
            document["kind"] = "nfa" if index == 0 else "dfa"
            self.mutate(document, "切换自动机类型")

    def change_tool(self, index):
        self.pending_source = None
        self.view.setInteractive(index != 2)
        self.view.setDragMode(QGraphicsView.ScrollHandDrag if index == 2 else QGraphicsView.RubberBandDrag)

    def auto_layout(self):
        document = copy.deepcopy(self.document)
        def work(cancelled):
            # Layout preserves draft IDs and does not require a valid DFA.
            from .layout import compute_data_layout
            validate_document(document)
            if not document["nodes"]:
                raise ValueError("请先创建状态")
            nodes = [(n["id"], f"q{n['id']}", n.get("accept", False), n.get("label", "")) for n in document["nodes"]]
            edges = [(e["source"], e["target"], e["label"]) for e in document["edges"]]
            start = next((n["id"] for n in document["nodes"] if n.get("start")), nodes[0][0])
            return "layout", compute_data_layout(nodes, edges, start, cancelled)
        self.jobs.start(work)

    def build(self):
        document = copy.deepcopy(self.document)
        def work(cancelled):
            machine = machine_from_document(document)
            construction = None
            if isinstance(machine, NFA):
                dfa, steps = determinize(machine, Context(Limits(), cancelled))
                construction = (machine, dfa, steps, machine_layout(machine, cancelled))
            else:
                dfa = machine
            return "machine", dfa, machine_layout(dfa, cancelled), construction
        self.jobs.start(work)

    def loaded(self, payload):
        if payload[0] == "layout":
            document = copy.deepcopy(self.document)
            for node in document["nodes"]:
                x, y, _, _ = payload[1][3][node["id"]]
                node.update(x=x, y=y)
            self.mutate(document, "自动布局")
            self.fit()
        else:
            self.message.setText("验证成功；已接入字符串模拟。可在优化页选择当前手绘自动机。")
            if payload[3]:
                self.construction_ready.emit((*payload[3], payload[2]))
            self.machine_ready.emit(payload[1], payload[2])
